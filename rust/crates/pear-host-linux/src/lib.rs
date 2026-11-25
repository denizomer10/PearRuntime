//! Clean-room: Linux host backend implementing `pear_host::HostPlatform`
//! using public POSIX/X11/Wayland surface descriptors and XDG conventions.
//!
//! Provides window/surface management (`NativeSurface::Wayland` / `Xlib`),
//! screen metrics (`PEAR_HOST_SCREEN`), pasteboard, URL opening (`xdg-open`),
//! and run loop control without any Apple framework dependencies.

use pear_host::{
    events, DocumentRequest, GuestObject, GuestSelector, HostPlatform, LayerRef, Lifecycle,
    NativeSurface, PasteboardItem, Rect, ScreenInfo, Size, SurfaceInfo, WindowHandle,
};
use std::collections::HashMap;
use std::ffi::c_void;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Clone)]
struct LinuxWindow {
    _id: u64,
    _owner: usize,
    frame: Rect,
    root_layer: usize,
    visible: bool,
    x11_display: usize,
    x11_window: u64,
}

#[cfg(target_os = "linux")]
mod x11_ffi {
    use std::ffi::{c_char, c_int, c_ulong, c_void};

    pub struct X11Lib {
        pub display: *mut c_void,
        pub x_create_simple_window: unsafe extern "C" fn(
            *mut c_void,
            c_ulong,
            c_int,
            c_int,
            u32,
            u32,
            u32,
            c_ulong,
            c_ulong,
        ) -> c_ulong,
        pub x_root_window: unsafe extern "C" fn(*mut c_void, c_int) -> c_ulong,
        pub x_default_screen: unsafe extern "C" fn(*mut c_void) -> c_int,
        pub x_map_window: unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int,
        pub x_store_name: unsafe extern "C" fn(*mut c_void, c_ulong, *const c_char) -> c_int,
        pub x_set_window_background: unsafe extern "C" fn(*mut c_void, c_ulong, c_ulong) -> c_int,
        pub x_clear_window: unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int,
        pub x_flush: unsafe extern "C" fn(*mut c_void) -> c_int,
        pub x_pending: unsafe extern "C" fn(*mut c_void) -> c_int,
        pub x_next_event: unsafe extern "C" fn(*mut c_void, *mut u8) -> c_int,
        pub x_select_input: unsafe extern "C" fn(*mut c_void, c_ulong, i64) -> c_int,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct XButtonEvent {
        pub ev_type: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut c_void,
        pub window: c_ulong,
        pub root: c_ulong,
        pub subwindow: c_ulong,
        pub time: c_ulong,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub state: u32,
        pub button: u32,
        pub same_screen: c_int,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct XMotionEvent {
        pub ev_type: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut c_void,
        pub window: c_ulong,
        pub root: c_ulong,
        pub subwindow: c_ulong,
        pub time: c_ulong,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub state: u32,
        pub is_hint: u8,
        pub same_screen: c_int,
    }

    unsafe impl Send for X11Lib {}
    unsafe impl Sync for X11Lib {}

    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    pub fn try_open() -> Option<X11Lib> {
        if std::env::var_os("DISPLAY").is_none() {
            return None;
        }
        unsafe {
            let h = dlopen(c"libX11.so.6".as_ptr(), 1);
            if h.is_null() {
                return None;
            }
            let x_open: unsafe extern "C" fn(*const c_char) -> *mut c_void =
                std::mem::transmute(dlsym(h, c"XOpenDisplay".as_ptr()));
            let display = x_open(std::ptr::null());
            if display.is_null() {
                return None;
            }
            Some(X11Lib {
                display,
                x_create_simple_window: std::mem::transmute(dlsym(h, c"XCreateSimpleWindow".as_ptr())),
                x_root_window: std::mem::transmute(dlsym(h, c"XRootWindow".as_ptr())),
                x_default_screen: std::mem::transmute(dlsym(h, c"XDefaultScreen".as_ptr())),
                x_map_window: std::mem::transmute(dlsym(h, c"XMapWindow".as_ptr())),
                x_store_name: std::mem::transmute(dlsym(h, c"XStoreName".as_ptr())),
                x_set_window_background: std::mem::transmute(dlsym(h, c"XSetWindowBackground".as_ptr())),
                x_clear_window: std::mem::transmute(dlsym(h, c"XClearWindow".as_ptr())),
                x_flush: std::mem::transmute(dlsym(h, c"XFlush".as_ptr())),
                x_pending: std::mem::transmute(dlsym(h, c"XPending".as_ptr())),
                x_next_event: std::mem::transmute(dlsym(h, c"XNextEvent".as_ptr())),
                x_select_input: std::mem::transmute(dlsym(h, c"XSelectInput".as_ptr())),
            })
        }
    }
}

#[derive(Default)]
struct LinuxState {
    windows: HashMap<u64, LinuxWindow>,
    layer_to_window: HashMap<usize, u64>,
    pasteboards: HashMap<String, Vec<PasteboardItem>>,
    idle_disabled: bool,
    screen: Option<ScreenInfo>,
}

pub struct LinuxHost {
    next_id: AtomicU64,
    change_count: AtomicIsize,
    running: AtomicBool,
    use_wayland: bool,
    state: Mutex<LinuxState>,
}

impl Default for LinuxHost {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxHost {
    pub fn new() -> Self {
        let use_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        Self {
            next_id: AtomicU64::new(1),
            change_count: AtomicIsize::new(0),
            running: AtomicBool::new(false),
            use_wayland,
            state: Mutex::new(LinuxState::default()),
        }
    }

    pub fn install() -> &'static dyn HostPlatform {
        let _ = pear_host::install(Box::new(LinuxHost::new()));
        pear_host::host()
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    fn parse_screen_env() -> ScreenInfo {
        if let Ok(val) = std::env::var("PEAR_HOST_SCREEN") {
            let parts: Vec<&str> = val.split('x').collect();
            if parts.len() == 2 {
                if let (Ok(w), Ok(h)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    if w > 0.0 && h > 0.0 {
                        return ScreenInfo {
                            points: Size { width: w, height: h },
                            scale: 1.0,
                            max_fps: 60,
                        };
                    }
                }
            }
        }
        ScreenInfo {
            points: Size { width: 1280.0, height: 720.0 },
            scale: 1.0,
            max_fps: 60,
        }
    }
}

impl HostPlatform for LinuxHost {
    fn initialize(&self) {
        let info = Self::parse_screen_env();
        self.state.lock().unwrap().screen = Some(info);
    }

    fn screen(&self) -> ScreenInfo {
        self.state
            .lock()
            .unwrap()
            .screen
            .unwrap_or_else(Self::parse_screen_env)
    }

    fn create_window(&self, owner: GuestObject, frame: Rect) -> WindowHandle {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        #[allow(unused_mut)]
        let mut x11_display = 0usize;
        #[allow(unused_mut)]
        let mut x11_window = 0u64;
        #[cfg(target_os = "linux")]
        if !self.use_wayland {
            if let Some(x11) = x11_ffi::try_open() {
                unsafe {
                    x11_display = x11.display as usize;
                    let scr = (x11.x_default_screen)(x11.display);
                    let root = (x11.x_root_window)(x11.display, scr);
                    let w = (frame.width as u32).max(64);
                    let h = (frame.height as u32).max(64);
                    x11_window = (x11.x_create_simple_window)(
                        x11.display,
                        root,
                        frame.x as i32,
                        frame.y as i32,
                        w,
                        h,
                        2,
                        0x4FD1_C5,
                        0x1418_24,
                    ) as u64;
                    (x11.x_store_name)(x11.display, x11_window as _, c"PearRuntime (Linux ARM64)".as_ptr());
                    (x11.x_set_window_background)(x11.display, x11_window as _, 0x1418_24);
                    (x11.x_map_window)(x11.display, x11_window as _);
                    (x11.x_clear_window)(x11.display, x11_window as _);
                    (x11.x_flush)(x11.display);
                }
            }
        }
        let win = LinuxWindow {
            _id: id,
            _owner: owner.0 as usize,
            frame,
            root_layer: 0,
            visible: false,
            x11_display,
            x11_window,
        };
        self.state.lock().unwrap().windows.insert(id, win);
        WindowHandle(id as usize as *mut c_void)
    }

    fn set_root_layer(&self, window: WindowHandle, layer: LayerRef) {
        let id = window.0 as usize as u64;
        let mut st = self.state.lock().unwrap();
        if let Some(w) = st.windows.get_mut(&id) {
            w.root_layer = layer.0 as usize;
        }
        if !layer.0.is_null() {
            st.layer_to_window.insert(layer.0 as usize, id);
        }
    }

    fn show_window(&self, window: WindowHandle, key: bool) {
        let id = window.0 as usize as u64;
        let mut st = self.state.lock().unwrap();
        if let Some(w) = st.windows.get_mut(&id) {
            w.visible = true;
            let _ = key;
        }
    }

    fn surface(&self, layer: LayerRef) -> Option<SurfaceInfo> {
        let st = self.state.lock().unwrap();
        let win_id = *st.layer_to_window.get(&(layer.0 as usize))?;
        let win = st.windows.get(&win_id)?;
        let scale = st.screen.map_or(1.0, |s| s.scale);
        let width_px = (win.frame.width * scale).round().max(1.0);
        let height_px = (win.frame.height * scale).round().max(1.0);
        let surface = if self.use_wayland {
            NativeSurface::Wayland {
                display: std::ptr::null_mut(),
                surface: win_id as usize as *mut c_void,
                egl_window: win_id as usize as *mut c_void,
            }
        } else {
            NativeSurface::Xlib {
                display: win.x11_display as *mut c_void,
                window: if win.x11_window != 0 { win.x11_window } else { win_id },
            }
        };
        Some(SurfaceInfo {
            surface,
            pixels: Size { width: width_px, height: height_px },
            scale,
        })
    }

    fn display_link(&self, _target: GuestObject, _selector: GuestSelector) -> GuestObject {
        GuestObject(std::ptr::null_mut())
    }

    fn open_url(&self, url: &str) -> bool {
        Command::new("xdg-open").arg(url).spawn().is_ok()
    }

    fn set_idle_timer_disabled(&self, disabled: bool) {
        self.state.lock().unwrap().idle_disabled = disabled;
    }

    fn present_alert(
        &self,
        _title: &str,
        _message: &str,
        _buttons: &[String],
        reply: Box<dyn FnOnce(isize) + Send>,
    ) {
        reply(-1);
    }

    fn pasteboard_read(&self, name: Option<&str>) -> Vec<PasteboardItem> {
        let key = name.unwrap_or("general").to_string();
        self.state
            .lock()
            .unwrap()
            .pasteboards
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    fn pasteboard_types(&self, name: Option<&str>) -> Vec<String> {
        let items = self.pasteboard_read(name);
        let mut out = Vec::new();
        for item in items {
            match item {
                PasteboardItem::Text(_) => out.push("public.utf8-plain-text".into()),
                PasteboardItem::Url(_) => out.push("public.url".into()),
                PasteboardItem::Data { uti, .. } => out.push(uti),
            }
        }
        out
    }

    fn pasteboard_data(&self, name: Option<&str>, uti: &str) -> Option<Vec<u8>> {
        for item in self.pasteboard_read(name) {
            if let PasteboardItem::Data { uti: u, bytes } = item {
                if u == uti {
                    return Some(bytes);
                }
            }
        }
        None
    }

    fn pasteboard_write(&self, name: Option<&str>, items: &[PasteboardItem]) {
        let key = name.unwrap_or("general").to_string();
        self.state.lock().unwrap().pasteboards.insert(key, items.to_vec());
        self.change_count.fetch_add(1, Ordering::Relaxed);
    }

    fn pasteboard_change_count(&self, _name: Option<&str>) -> isize {
        self.change_count.load(Ordering::Relaxed)
    }

    fn pick_documents(
        &self,
        _request: DocumentRequest,
        reply: Box<dyn FnOnce(Option<Vec<String>>) + Send>,
    ) {
        reply(None);
    }

    fn share(&self, _items: &[PasteboardItem]) {}

    fn screen_reader_running(&self) -> bool {
        false
    }

    fn run(&self, did_finish_launching: Box<dyn FnOnce()>) {
        self.running.store(true, Ordering::Release);
        did_finish_launching();
        events().lifecycle(Lifecycle::DidBecomeActive);
        #[cfg(target_os = "linux")]
        let x11 = if !self.use_wayland { x11_ffi::try_open() } else { None };
        #[cfg(target_os = "linux")]
        let mut selected_windows = std::collections::HashSet::<u64>::new();
        #[cfg(target_os = "linux")]
        let mut mouse_down = false;
        let mut ev_buf = [0u8; 192];
        while self.running.load(Ordering::Acquire) {
            #[cfg(target_os = "linux")]
            if let Some(ref x) = x11 {
                let pending_wins: Vec<(u64, usize)> = {
                    let g = self.state.lock().unwrap();
                    g.windows
                        .values()
                        .filter(|w| w.x11_window != 0 && !selected_windows.contains(&w.x11_window))
                        .map(|w| (w.x11_window, w._owner))
                        .collect()
                };
                if !pending_wins.is_empty() {
                    const INPUT_MASK: i64 = 0x0000_0004 | 0x0000_0008 | 0x0000_0040 | 0x0000_0100 | 0x0002_0000;
                    unsafe {
                        for (xw, _) in pending_wins {
                            (x.x_select_input)(x.display, xw as _, INPUT_MASK);
                            selected_windows.insert(xw);
                        }
                        (x.x_flush)(x.display);
                    }
                }
                let scale = self.screen().scale.max(1.0);
                let resolve_owner = |xw: u64| -> GuestObject {
                    let g = self.state.lock().unwrap();
                    let ptr = g
                        .windows
                        .values()
                        .find(|w| w.x11_window == xw)
                        .or_else(|| g.windows.values().next())
                        .map(|w| w._owner)
                        .unwrap_or(0);
                    GuestObject(ptr as *mut c_void)
                };
                unsafe {
                    while (x.x_pending)(x.display) > 0 {
                        (x.x_next_event)(x.display, ev_buf.as_mut_ptr());
                        let ev_type = *(ev_buf.as_ptr() as *const i32);
                        match ev_type {
                            4 => {
                                let bev = *(ev_buf.as_ptr() as *const x11_ffi::XButtonEvent);
                                if bev.button == 1 {
                                    mouse_down = true;
                                    let ts = (bev.time as f64) / 1000.0;
                                    events().pointer(
                                        resolve_owner(bev.window as u64),
                                        pear_host::TouchPhase::Began,
                                        (bev.x as f64) / scale,
                                        (bev.y as f64) / scale,
                                        ts,
                                    );
                                }
                            }
                            5 => {
                                let bev = *(ev_buf.as_ptr() as *const x11_ffi::XButtonEvent);
                                if bev.button == 1 {
                                    mouse_down = false;
                                    let ts = (bev.time as f64) / 1000.0;
                                    events().pointer(
                                        resolve_owner(bev.window as u64),
                                        pear_host::TouchPhase::Ended,
                                        (bev.x as f64) / scale,
                                        (bev.y as f64) / scale,
                                        ts,
                                    );
                                }
                            }
                            6 => {
                                let mev = *(ev_buf.as_ptr() as *const x11_ffi::XMotionEvent);
                                if mouse_down || (mev.state & 0x100) != 0 {
                                    let ts = (mev.time as f64) / 1000.0;
                                    events().pointer(
                                        resolve_owner(mev.window as u64),
                                        pear_host::TouchPhase::Moved,
                                        (mev.x as f64) / scale,
                                        (mev.y as f64) / scale,
                                        ts,
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    (x.x_flush)(x.display);
                }
            }
            let tick = FRAME_TICK.load(Ordering::Acquire);
            if tick != 0 {
                let cb: extern "C" fn() = unsafe { std::mem::transmute(tick) };
                cb();
            }
            let _ = &mut ev_buf;
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        events().lifecycle(Lifecycle::WillTerminate);
    }
}

static FRAME_TICK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Registers a callback invoked once per host run-loop frame (~60 Hz) on the main thread.
pub fn set_frame_tick(cb: extern "C" fn()) {
    FRAME_TICK.store(cb as usize, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_host_window_surface_and_pasteboard() {
        let host = LinuxHost::new();
        host.initialize();
        let scr = host.screen();
        assert!(scr.points.width > 0.0 && scr.points.height > 0.0);

        let w = host.create_window(
            GuestObject(std::ptr::null_mut()),
            Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 },
        );
        let layer = LayerRef(0x1234 as *mut c_void);
        host.set_root_layer(w, layer);
        host.show_window(w, true);

        let info = host.surface(layer).expect("surface for attached layer");
        assert_eq!(info.pixels.width, 800.0);
        assert_eq!(info.pixels.height, 600.0);
        assert!(!info.surface.egl_window().is_null());

        host.pasteboard_write(None, &[PasteboardItem::Text("selam".into())]);
        assert_eq!(host.pasteboard_read(None), vec![PasteboardItem::Text("selam".into())]);
        assert_eq!(host.pasteboard_change_count(None), 1);
    }
}
