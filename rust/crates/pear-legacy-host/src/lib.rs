//! Clean-room: `pear-host` adapter for PearLegacyEngine (32-bit runtime),
//! bridging `HostPlatform` windows, EGL/Vulkan surfaces, and `GuestEvents`
//! input queues so `legacy-engine` can retire its SDL2 dependency per platform.

use pear_host::{
    EditCommand, GuestEvents, GuestObject, KeyEvent, LayerRef, Lifecycle, Rect,
    ScreenInfo, Size, SurfaceInfo, TouchPhase, WindowHandle,
};
use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CStr};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq)]
pub enum LegacyEvent {
    Pointer {
        phase: TouchPhase,
        x: f64,
        y: f64,
        timestamp: f64,
    },
    Key {
        hid_usage: u16,
        down: bool,
        modifiers: u64,
        timestamp: f64,
    },
    Text(String),
    Edit(EditCommand),
    Resized {
        width: f64,
        height: f64,
        scale: f64,
    },
    Lifecycle(Lifecycle),
}

struct LegacyEventSink {
    queue: Mutex<VecDeque<LegacyEvent>>,
}

impl LegacyEventSink {
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }
}

impl GuestEvents for LegacyEventSink {
    fn pointer(&self, _window: GuestObject, phase: TouchPhase, x: f64, y: f64, timestamp: f64) {
        self.queue.lock().unwrap().push_back(LegacyEvent::Pointer {
            phase,
            x,
            y,
            timestamp,
        });
    }

    fn key(&self, event: KeyEvent) {
        self.queue.lock().unwrap().push_back(LegacyEvent::Key {
            hid_usage: event.hid_usage,
            down: event.down,
            modifiers: event.modifiers,
            timestamp: event.timestamp,
        });
    }

    fn text(&self, text: &str) {
        self.queue.lock().unwrap().push_back(LegacyEvent::Text(text.to_string()));
    }

    fn edit_command(&self, command: EditCommand) {
        self.queue.lock().unwrap().push_back(LegacyEvent::Edit(command));
    }

    fn text_input_active(&self) -> bool {
        false
    }

    fn window_resized(&self, _window: GuestObject, points: Size, scale: f64) {
        self.queue.lock().unwrap().push_back(LegacyEvent::Resized {
            width: points.width,
            height: points.height,
            scale,
        });
    }

    fn window_became_key(&self, _window: GuestObject) {}

    fn lifecycle(&self, event: Lifecycle) {
        self.queue.lock().unwrap().push_back(LegacyEvent::Lifecycle(event));
    }
}

fn event_sink() -> &'static LegacyEventSink {
    static SINK: OnceLock<LegacyEventSink> = OnceLock::new();
    SINK.get_or_init(LegacyEventSink::new)
}

struct SinkForwarder;

impl GuestEvents for SinkForwarder {
    fn pointer(&self, w: GuestObject, phase: TouchPhase, x: f64, y: f64, ts: f64) {
        event_sink().pointer(w, phase, x, y, ts);
    }
    fn key(&self, event: KeyEvent) {
        event_sink().key(event);
    }
    fn text(&self, text: &str) {
        event_sink().text(text);
    }
    fn edit_command(&self, command: EditCommand) {
        event_sink().edit_command(command);
    }
    fn text_input_active(&self) -> bool {
        event_sink().text_input_active()
    }
    fn window_resized(&self, w: GuestObject, points: Size, scale: f64) {
        event_sink().window_resized(w, points, scale);
    }
    fn window_became_key(&self, w: GuestObject) {
        event_sink().window_became_key(w);
    }
    fn lifecycle(&self, event: Lifecycle) {
        event_sink().lifecycle(event);
    }
}

/// Initializes `pear-host` (using `LinuxHost` on Linux or `NullHost` fallback)
/// and installs the `GuestEvents` sink for `PearLegacyEngine`.
pub fn init_legacy_host() {
    if !pear_host::installed() {
        #[cfg(target_os = "linux")]
        {
            pear_host_linux::LinuxHost::install();
        }
        #[cfg(not(target_os = "linux"))]
        {
            pear_host::install(Box::new(pear_host::NullHost::default()));
        }
    }
    let _ = pear_host::install_events(Box::new(SinkForwarder));
    pear_host::host().initialize();
}

pub struct LegacyHostWindow {
    pub handle: WindowHandle,
    pub layer: LayerRef,
}

impl LegacyHostWindow {
    pub fn create(width: f64, height: f64) -> Self {
        init_legacy_host();
        let handle = pear_host::host().create_window(
            GuestObject(std::ptr::null_mut()),
            Rect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
        );
        let layer = LayerRef(handle.0);
        pear_host::host().set_root_layer(handle, layer);
        pear_host::host().show_window(handle, true);
        Self { handle, layer }
    }

    pub fn surface(&self) -> Option<SurfaceInfo> {
        pear_host::host().surface(self.layer)
    }

    pub fn screen(&self) -> ScreenInfo {
        pear_host::host().screen()
    }

    pub fn poll_event(&self) -> Option<LegacyEvent> {
        event_sink().queue.lock().unwrap().pop_front()
    }

    pub fn open_url(&self, url: &str) -> bool {
        pear_host::host().open_url(url)
    }
}

// ---- C-ABI entry points for PearLegacyEngine FFI ----------------------------

#[no_mangle]
pub extern "C" fn pear_legacy_host_init() {
    init_legacy_host();
}

#[no_mangle]
pub extern "C" fn pear_legacy_host_create_window(width: f64, height: f64) -> *mut c_void {
    let win = LegacyHostWindow::create(width, height);
    win.handle.0
}

#[no_mangle]
pub unsafe extern "C" fn pear_legacy_host_open_url(url: *const c_char) -> bool {
    if url.is_null() {
        return false;
    }
    let s = CStr::from_ptr(url).to_string_lossy();
    pear_host::host().open_url(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_host_window_receives_guest_events() {
        let win = LegacyHostWindow::create(480.0, 320.0);
        assert!(!win.handle.is_none());
        assert!(win.screen().points.width > 0.0);

        pear_host::events().pointer(
            GuestObject(std::ptr::null_mut()),
            TouchPhase::Began,
            120.0,
            80.0,
            1.5,
        );
        pear_host::events().lifecycle(Lifecycle::DidBecomeActive);

        assert_eq!(
            win.poll_event(),
            Some(LegacyEvent::Pointer {
                phase: TouchPhase::Began,
                x: 120.0,
                y: 80.0,
                timestamp: 1.5,
            })
        );
        assert_eq!(
            win.poll_event(),
            Some(LegacyEvent::Lifecycle(Lifecycle::DidBecomeActive))
        );
        assert_eq!(win.poll_event(), None);
    }
}
