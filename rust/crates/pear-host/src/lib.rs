//! The boundary between PearRuntime's portable compatibility layer and the
//! operating system it runs on.
//!
//! The iOS-facing frameworks (UIKit, OpenGLES, ...) ask the host only for
//! what is genuinely host-specific: windows and the layer they show, screen
//! facts, frame timing, the main loop, alerts, opening URLs and power
//! management. Each OS implements [`HostPlatform`] once (macOS: AppKit, in
//! pear-uikit's `host_macos`; Linux and Windows later), and the portable code
//! reaches it only through [`host()`]. Nothing here names an Apple type.
//!
//! Input flows the other way: a backend delivers pointer, key, text and
//! lifecycle events to the portable layer's `PearUIKitDeliver*` entry points.
//!
//! See docs/independence-plan.md (§4).

use std::ffi::c_void;
use std::sync::OnceLock;

/// A size in points (or pixels where stated).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// A rectangle in points, origin top-left.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// What the app sees as its screen: the logical size UIScreen reports, the
/// backing scale, and the highest refresh rate.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ScreenInfo {
    pub points: Size,
    pub scale: f64,
    pub max_fps: u32,
}

/// A host window, owned by the backend (NSWindow, wl_surface, HWND...).
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WindowHandle(pub *mut c_void);
unsafe impl Send for WindowHandle {}
unsafe impl Sync for WindowHandle {}

impl WindowHandle {
    pub const NONE: WindowHandle = WindowHandle(std::ptr::null_mut());
    pub fn is_none(self) -> bool {
        self.0.is_null()
    }
}

/// The root layer of a UIKit window: what the host window displays and what
/// graphics backends render into (a CALayer tree today).
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LayerRef(pub *mut c_void);
unsafe impl Send for LayerRef {}

/// An object of the iOS app's Objective-C world (a UIWindow, a display-link
/// target), passed through the host opaquely.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GuestObject(pub *mut c_void);
unsafe impl Send for GuestObject {}

/// A selector of the guest's Objective-C runtime.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GuestSelector(pub *mut c_void);

/// The services the portable runtime asks of the host OS.
pub trait HostPlatform: Send + Sync {
    /// Once, before the first window: connect to the window system and read
    /// the screen.
    fn initialize(&self);

    fn screen(&self) -> ScreenInfo;

    /// A host window for the UIKit window `owner`, sized to `frame` (points).
    /// Its events are delivered for `owner`.
    fn create_window(&self, owner: GuestObject, frame: Rect) -> WindowHandle;

    /// Shows `layer` (the UIKit window's root layer) as the window's content.
    fn set_root_layer(&self, window: WindowHandle, layer: LayerRef);

    fn show_window(&self, window: WindowHandle, key: bool);

    /// A display link (CADisplayLink as iOS apps use it) that sends
    /// `selector` to `target` once per host frame.
    fn display_link(&self, target: GuestObject, selector: GuestSelector) -> GuestObject;

    /// Opens `url` with the host's default handler.
    fn open_url(&self, url: &str) -> bool;

    /// Keeps the display awake while `disabled` (UIApplication.idleTimerDisabled).
    fn set_idle_timer_disabled(&self, disabled: bool);

    /// A modal alert; `reply` receives the chosen button index (-1 for none),
    /// on the main thread.
    fn present_alert(&self, title: &str, message: &str, buttons: &[String], reply: Box<dyn FnOnce(isize) + Send>);

    /// Runs the main loop; `did_finish_launching` runs once the host is ready.
    /// Returns only on hosts whose loop ends (tests); a real host terminates
    /// the process from its loop.
    fn run(&self, did_finish_launching: Box<dyn FnOnce()>);
}

static HOST: OnceLock<Box<dyn HostPlatform>> = OnceLock::new();

/// Installs the process's host. The first installation wins; returns false
/// for any later one.
pub fn install(host: Box<dyn HostPlatform>) -> bool {
    HOST.set(host).is_ok()
}

/// The installed host. Panics when none is installed: the portable layer
/// must not run before its image initializer installed one.
pub fn host() -> &'static dyn HostPlatform {
    HOST.get().map(|h| h.as_ref()).expect("pear-host: no HostPlatform installed")
}

/// Whether a host is installed.
pub fn installed() -> bool {
    HOST.get().is_some()
}

/// A host without a window system: fixed screen, windows are counted handles,
/// alerts answer -1, the main loop runs the launch callback and returns.
/// For tests and headless runs on any OS.
pub struct NullHost {
    pub screen: ScreenInfo,
    windows: std::sync::atomic::AtomicUsize,
}

impl NullHost {
    pub fn new(screen: ScreenInfo) -> Self {
        NullHost { screen, windows: std::sync::atomic::AtomicUsize::new(0) }
    }
    pub fn windows_created(&self) -> usize {
        self.windows.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for NullHost {
    fn default() -> Self {
        NullHost::new(ScreenInfo { points: Size { width: 1280.0, height: 720.0 }, scale: 1.0, max_fps: 60 })
    }
}

impl HostPlatform for NullHost {
    fn initialize(&self) {}
    fn screen(&self) -> ScreenInfo {
        self.screen
    }
    fn create_window(&self, _owner: GuestObject, _frame: Rect) -> WindowHandle {
        let n = self.windows.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        WindowHandle(n as *mut c_void)
    }
    fn set_root_layer(&self, _window: WindowHandle, _layer: LayerRef) {}
    fn show_window(&self, _window: WindowHandle, _key: bool) {}
    fn display_link(&self, _target: GuestObject, _selector: GuestSelector) -> GuestObject {
        GuestObject(std::ptr::null_mut())
    }
    fn open_url(&self, _url: &str) -> bool {
        false
    }
    fn set_idle_timer_disabled(&self, _disabled: bool) {}
    fn present_alert(&self, _title: &str, _message: &str, _buttons: &[String], reply: Box<dyn FnOnce(isize) + Send>) {
        reply(-1);
    }
    fn run(&self, did_finish_launching: Box<dyn FnOnce()>) {
        did_finish_launching();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn null_host_serves_the_portable_layer() {
        let host = NullHost::default();
        host.initialize();
        assert_eq!(host.screen().points, Size { width: 1280.0, height: 720.0 });
        let a = host.create_window(GuestObject(std::ptr::null_mut()), Rect::default());
        let b = host.create_window(GuestObject(std::ptr::null_mut()), Rect::default());
        assert!(!a.is_none() && a != b && host.windows_created() == 2);
        assert!(!host.open_url("https://example.com"));
        let answer = Arc::new(AtomicIsize::new(99));
        let seen = answer.clone();
        host.present_alert("t", "m", &["OK".into()], Box::new(move |i| seen.store(i, Ordering::SeqCst)));
        assert_eq!(answer.load(Ordering::SeqCst), -1);
        let launched = Arc::new(AtomicBool::new(false));
        let flag = launched.clone();
        host.run(Box::new(move || flag.store(true, Ordering::SeqCst)));
        assert!(launched.load(Ordering::SeqCst));
    }

    #[test]
    fn first_installed_host_wins() {
        assert!(install(Box::new(NullHost::default())));
        assert!(!install(Box::new(NullHost::default())));
        assert!(installed());
        assert_eq!(host().screen().max_fps, 60);
    }
}
