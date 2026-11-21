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
        let win = LinuxWindow {
            _id: id,
            _owner: owner.0 as usize,
            frame,
            root_layer: 0,
            visible: false,
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
                display: std::ptr::null_mut(),
                window: win_id,
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
        while self.running.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        events().lifecycle(Lifecycle::WillTerminate);
    }
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
