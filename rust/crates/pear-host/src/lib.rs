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
//! Input flows the other way: a host delivers pointer, key, text, window and
//! lifecycle events to the [`GuestEvents`] sink the iOS layer installed
//! ([`events()`]).
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

/// One item on a pasteboard, in the forms iOS apps exchange through
/// UIPasteboard and share sheets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasteboardItem {
    Text(String),
    /// An absolute URL string (web or file URL).
    Url(String),
    /// Bytes of a uniform type (e.g. "public.png", an app's own type).
    Data { uti: String, bytes: Vec<u8> },
}

/// Uniform type identifiers of the common pasteboard representations.
pub const UTI_TEXT: &str = "public.utf8-plain-text";
pub const UTI_URL: &str = "public.url";
pub const UTI_FILE_URL: &str = "public.file-url";

impl PasteboardItem {
    /// The uniform type this item is stored as.
    pub fn uti(&self) -> &str {
        match self {
            PasteboardItem::Text(_) => UTI_TEXT,
            PasteboardItem::Url(_) => UTI_URL,
            PasteboardItem::Data { uti, .. } => uti,
        }
    }
}

/// A document picker request (UIDocumentPickerViewController).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentRequest {
    /// Choose existing files; `directory` is a file URL to start in.
    Open { multiple: bool, directory: Option<String> },
    /// Choose where to save a document named `suggested_name`.
    Save { suggested_name: String, directory: Option<String> },
}

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

    /// The text and URL items on the pasteboard `name` (None: the general
    /// pasteboard); other types are read with [`HostPlatform::pasteboard_data`].
    fn pasteboard_read(&self, name: Option<&str>) -> Vec<PasteboardItem>;

    /// The uniform types present on the pasteboard.
    fn pasteboard_types(&self, name: Option<&str>) -> Vec<String>;

    /// The first item's bytes of uniform type `uti`.
    fn pasteboard_data(&self, name: Option<&str>, uti: &str) -> Option<Vec<u8>>;

    /// Replaces the pasteboard's contents with `items` (empty clears it).
    fn pasteboard_write(&self, name: Option<&str>, items: &[PasteboardItem]);

    /// Increments whenever the pasteboard's contents change.
    fn pasteboard_change_count(&self, name: Option<&str>) -> isize;

    /// Asks the user for documents; `reply` receives the chosen file URLs, or
    /// None when cancelled, on the main thread.
    fn pick_documents(&self, request: DocumentRequest, reply: Box<dyn FnOnce(Option<Vec<String>>) + Send>);

    /// Offers `items` to the host's sharing services (UIActivityViewController).
    fn share(&self, items: &[PasteboardItem]);

    /// A screen reader (VoiceOver on macOS) is running.
    fn screen_reader_running(&self) -> bool;

    /// Runs the main loop; `did_finish_launching` runs once the host is ready.
    /// Returns only on hosts whose loop ends (tests); a real host terminates
    /// the process from its loop.
    fn run(&self, did_finish_launching: Box<dyn FnOnce()>);
}

/// Phase of a touch, with UITouchPhase's values.
#[repr(isize)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Began = 0,
    Moved = 1,
    Ended = 3,
    Cancelled = 4,
}

/// Modifier bits of a key event, with UIKeyModifierFlags' values.
pub mod modifiers {
    pub const ALPHA_SHIFT: u64 = 1 << 16;
    pub const SHIFT: u64 = 1 << 17;
    pub const CONTROL: u64 = 1 << 18;
    pub const ALTERNATE: u64 = 1 << 19;
    pub const COMMAND: u64 = 1 << 20;
    pub const NUMERIC_PAD: u64 = 1 << 21;
}

/// A hardware key press or release.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    /// USB HID usage (keyboard page), as UIKey.keyCode.
    pub hid_usage: u16,
    /// The characters the key produces, with and without modifiers.
    pub characters: String,
    pub characters_ignoring_modifiers: String,
    /// [`modifiers`] bits.
    pub modifiers: u64,
    pub down: bool,
    /// Seconds since boot.
    pub timestamp: f64,
}

/// Text-editing commands a host's text system produces.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EditCommand {
    DeleteBackward = 0,
    Newline = 1,
    MoveLeft = 2,
    MoveRight = 3,
    Cancel = 4,
}

/// Application lifecycle transitions (UIApplicationDelegate's).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    DidBecomeActive,
    WillResignActive,
    DidEnterBackground,
    WillEnterForeground,
    WillTerminate,
}

/// Where a host delivers input and window/application events: the portable
/// iOS layer (UIKit implements it). Hosts call [`events()`], never UIKit.
pub trait GuestEvents: Send + Sync {
    /// A pointer/touch in `window` at (`x`, `y`) points, origin top-left.
    fn pointer(&self, window: GuestObject, phase: TouchPhase, x: f64, y: f64, timestamp: f64);
    fn key(&self, event: KeyEvent);
    /// Committed text while a text-input responder is active.
    fn text(&self, text: &str);
    fn edit_command(&self, command: EditCommand);
    /// A text-input responder is first responder: the host should route keys
    /// through its text system (`text`/`edit_command`) instead of `key`.
    fn text_input_active(&self) -> bool;
    fn window_resized(&self, window: GuestObject, points: Size, scale: f64);
    fn window_became_key(&self, window: GuestObject);
    fn lifecycle(&self, event: Lifecycle);
}

static EVENTS: OnceLock<Box<dyn GuestEvents>> = OnceLock::new();

/// Installs the process's event sink. The first installation wins.
pub fn install_events(sink: Box<dyn GuestEvents>) -> bool {
    EVENTS.set(sink).is_ok()
}

/// The installed event sink. Panics when none is installed: hosts deliver
/// events only after the iOS layer started.
pub fn events() -> &'static dyn GuestEvents {
    EVENTS.get().map(|e| e.as_ref()).expect("pear-host: no GuestEvents sink installed")
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
    /// In-memory pasteboards by name ("" is the general one), with change counts.
    pasteboards: std::sync::Mutex<std::collections::HashMap<String, (isize, Vec<PasteboardItem>)>>,
}

impl NullHost {
    pub fn new(screen: ScreenInfo) -> Self {
        NullHost { screen, windows: std::sync::atomic::AtomicUsize::new(0), pasteboards: Default::default() }
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
    fn pasteboard_read(&self, name: Option<&str>) -> Vec<PasteboardItem> {
        let boards = self.pasteboards.lock().unwrap();
        let items = boards.get(name.unwrap_or("")).map(|(_, items)| items.clone()).unwrap_or_default();
        items.into_iter().filter(|i| !matches!(i, PasteboardItem::Data { .. })).collect()
    }
    fn pasteboard_types(&self, name: Option<&str>) -> Vec<String> {
        let boards = self.pasteboards.lock().unwrap();
        let mut types: Vec<String> = Vec::new();
        for item in boards.get(name.unwrap_or("")).map(|(_, items)| items.as_slice()).unwrap_or_default() {
            if !types.iter().any(|t| t == item.uti()) {
                types.push(item.uti().to_string());
            }
        }
        types
    }
    fn pasteboard_data(&self, name: Option<&str>, uti: &str) -> Option<Vec<u8>> {
        let boards = self.pasteboards.lock().unwrap();
        boards.get(name.unwrap_or(""))?.1.iter().find(|i| i.uti() == uti).map(|i| match i {
            PasteboardItem::Text(s) | PasteboardItem::Url(s) => s.clone().into_bytes(),
            PasteboardItem::Data { bytes, .. } => bytes.clone(),
        })
    }
    fn pasteboard_write(&self, name: Option<&str>, items: &[PasteboardItem]) {
        let mut boards = self.pasteboards.lock().unwrap();
        let entry = boards.entry(name.unwrap_or("").to_string()).or_default();
        entry.0 += 1;
        entry.1 = items.to_vec();
    }
    fn pasteboard_change_count(&self, name: Option<&str>) -> isize {
        self.pasteboards.lock().unwrap().get(name.unwrap_or("")).map(|(count, _)| *count).unwrap_or(0)
    }
    fn pick_documents(&self, _request: DocumentRequest, reply: Box<dyn FnOnce(Option<Vec<String>>) + Send>) {
        reply(None);
    }
    fn share(&self, _items: &[PasteboardItem]) {}
    fn screen_reader_running(&self) -> bool {
        false
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
    fn null_host_pasteboards_and_pickers() {
        let host = NullHost::default();
        assert!(host.pasteboard_read(None).is_empty());
        assert_eq!(host.pasteboard_change_count(None), 0);
        let items = vec![PasteboardItem::Text("pear".into()), PasteboardItem::Url("https://example.com".into())];
        host.pasteboard_write(None, &items);
        assert_eq!(host.pasteboard_read(None), items);
        assert_eq!(host.pasteboard_change_count(None), 1);
        assert!(host.pasteboard_read(Some("game")).is_empty(), "named pasteboards are separate");
        assert_eq!(host.pasteboard_types(None), vec![UTI_TEXT.to_string(), UTI_URL.to_string()]);
        assert_eq!(host.pasteboard_data(None, UTI_TEXT), Some(b"pear".to_vec()));
        let save = PasteboardItem::Data { uti: "com.example.save".into(), bytes: vec![1, 2, 3] };
        host.pasteboard_write(Some("game"), &[save]);
        assert!(host.pasteboard_read(Some("game")).is_empty(), "data items are read by type");
        assert_eq!(host.pasteboard_data(Some("game"), "com.example.save"), Some(vec![1, 2, 3]));
        host.pasteboard_write(None, &[]);
        assert!(host.pasteboard_read(None).is_empty() && host.pasteboard_change_count(None) == 2);
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        host.pick_documents(
            DocumentRequest::Open { multiple: false, directory: None },
            Box::new(move |urls| flag.store(urls.is_none(), Ordering::SeqCst)),
        );
        assert!(cancelled.load(Ordering::SeqCst));
        assert!(!host.screen_reader_running());
    }

    /// A sink recording what a host delivers (into LOG: the sink is global).
    static LOG: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    #[derive(Default)]
    struct Recorder;
    impl GuestEvents for Recorder {
        fn pointer(&self, _w: GuestObject, phase: TouchPhase, x: f64, y: f64, _t: f64) {
            LOG.lock().unwrap().push(format!("pointer {phase:?} {x} {y}"));
        }
        fn key(&self, event: KeyEvent) {
            LOG.lock().unwrap().push(format!("key {} {}", event.hid_usage, event.down));
        }
        fn text(&self, text: &str) {
            LOG.lock().unwrap().push(format!("text {text}"));
        }
        fn edit_command(&self, command: EditCommand) {
            LOG.lock().unwrap().push(format!("edit {command:?}"));
        }
        fn text_input_active(&self) -> bool {
            false
        }
        fn window_resized(&self, _w: GuestObject, points: Size, scale: f64) {
            LOG.lock().unwrap().push(format!("resize {}x{}@{scale}", points.width, points.height));
        }
        fn window_became_key(&self, _w: GuestObject) {
            LOG.lock().unwrap().push("key window".into());
        }
        fn lifecycle(&self, event: Lifecycle) {
            LOG.lock().unwrap().push(format!("{event:?}"));
        }
    }

    #[test]
    fn events_reach_the_installed_sink() {
        assert_eq!(TouchPhase::Ended as isize, 3, "UITouchPhaseEnded");
        assert_eq!(EditCommand::Cancel as i32, 4);
        assert!(install_events(Box::new(Recorder::default())));
        assert!(!install_events(Box::new(Recorder::default())));
        let w = GuestObject(std::ptr::null_mut());
        events().pointer(w, TouchPhase::Began, 10.0, 20.0, 1.0);
        events().key(KeyEvent {
            hid_usage: 0x04,
            characters: "a".into(),
            characters_ignoring_modifiers: "a".into(),
            modifiers: modifiers::SHIFT,
            down: true,
            timestamp: 1.0,
        });
        events().text("pear");
        events().edit_command(EditCommand::DeleteBackward);
        events().window_resized(w, Size { width: 800.0, height: 600.0 }, 2.0);
        events().lifecycle(Lifecycle::WillTerminate);
        assert!(!events().text_input_active());
        assert_eq!(
            *LOG.lock().unwrap(),
            vec![
                "pointer Began 10 20",
                "key 4 true",
                "text pear",
                "edit DeleteBackward",
                "resize 800x600@2",
                "WillTerminate"
            ]
        );
    }

    #[test]
    fn first_installed_host_wins() {
        assert!(install(Box::new(NullHost::default())));
        assert!(!install(Box::new(NullHost::default())));
        assert!(installed());
        assert_eq!(host().screen().max_fps, 60);
    }
}
