//! PearRuntime's macOS host: pear-host's [`HostPlatform`] on AppKit (windows,
//! the main loop, alerts, NSWorkspace), QuartzCore (layer hosting, display
//! links) and IOKit (power assertions), and the AppKit side of input: an
//! NSView and window/application delegates that deliver pointer, key, text
//! and lifecycle events to the UIKit layer's `PearUIKit*` entry points.
//!
//! This is the only crate of the PearHost frameworks that names AppKit. A
//! Linux or Windows host implements the same trait (see pear-host and
//! docs/independence-plan.md §4). The `PearBackend*` functions stay exported
//! from UIKit.framework for the C ABI.

#![allow(non_snake_case, clippy::missing_safety_doc)]

use pear_host::{
    DocumentRequest, GuestObject, GuestSelector, HostPlatform, LayerRef, PasteboardItem, Rect, ScreenInfo, Size,
    WindowHandle, UTI_FILE_URL, UTI_TEXT, UTI_URL,
};
use pear_objc::{
    cls, is_kind_of, msg0, msg1, msg2, msg3, msg4, nsarray_count, nsarray_from_ids, nsarray_object_at,
    nsdata_from_bytes, nsdata_to_vec, nsnumber_i64, nsstring, nsstring_to_string,
    objc_getAssociatedObject, objc_release, objc_setAssociatedObject, replace_method as add_method,
    responds_to, sel, CGPoint, CGRect, CGSize, Class, Id, Imp, Sel, NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// UIScreen facts as the C ABI (`PearBackendScreenInfo`) returns them.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PearHostScreenInfo {
    pub point_size: CGSize,
    pub scale: f64,
    pub max_fps: isize,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CAFrameRateRange {
    minimum: f32,
    maximum: f32,
    preferred: f32,
}

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extraBytes: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn class_replaceMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> Imp;
    fn object_getClass(obj: Id) -> Class;
    fn class_getName(cls: Class) -> *const c_char;
    fn sel_getName(sel: Sel) -> *const c_char;
    fn IOPMAssertionCreateWithName(assertion_type: *const c_void, level: u32, name: *const c_void, assertion_id: *mut u32) -> i32;
    fn IOPMAssertionRelease(assertion_id: u32) -> i32;

    // PearHost UIKit (pear-core, pear-uikit): logging, missing-call reports,
    // and the event sink of the portable layer.
    fn PearHostLogMessage(tag: *const c_char, message: *const c_char);
    fn PearMissingReport(kind: c_int, owner: *const c_char, name: *const c_char, caller: *const c_void);
    fn PearUIKitTryProbeDisplayLinkSelector(target: Id, selector: Sel, is_setter: bool) -> bool;
    fn PearUIKitDeliverPointer(window: Id, phase: isize, point: CGPoint, timestamp: f64);
    fn PearUIKitDeliverKey(hid_usage: isize, chars: Id, chars_no_mods: Id, modifiers: isize, down: bool, timestamp: f64);
    fn PearUIKitDeliverText(text: Id);
    fn PearUIKitDeliverEditCommand(cmd: c_int);
    fn PearUIKitTextInputActive() -> bool;
    fn PearUIKitWindowDidResize(window: Id, point_size: CGSize, scale: f64);
    fn PearUIKitSetKeyWindow(window: Id);
    fn PearUIKitApplicationDidBecomeActive();
    fn PearUIKitApplicationWillResignActive();
    fn PearUIKitApplicationDidEnterBackground();
    fn PearUIKitApplicationWillEnterForeground();
    fn PearUIKitApplicationWillTerminate();
}

unsafe fn add_class_method(c: Class, selector: &str, imp: Imp, types: &str) {
    let meta = object_getClass(c as Id);
    add_method(meta, selector, imp, types);
}

fn assoc_get(obj: Id, key: &u8) -> Id {
    unsafe { objc_getAssociatedObject(obj, key as *const u8 as *const c_void) }
}

fn assoc_set(obj: Id, key: &u8, val: Id) {
    unsafe {
        objc_setAssociatedObject(obj, key as *const u8 as *const c_void, val, OBJC_ASSOCIATION_RETAIN);
    }
}

unsafe fn log_msg(tag: &str, msg: &str) {
    let t = CString::new(tag).unwrap_or_default();
    let m = CString::new(msg).unwrap_or_default();
    PearHostLogMessage(t.as_ptr(), m.as_ptr());
}

/// Registers the AppKit helper classes and installs [`MacHost`] as the
/// process's host. Called from UIKit.framework's image initializer.
pub fn install() {
    unsafe { register_classes() };
    pear_host::install(Box::new(MacHost));
}

// ---- AppKit backend ----------------------------------------------------------


static G_NATIVE_WINDOWS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static G_PRIMARY_VIEW: AtomicUsize = AtomicUsize::new(0);
static G_SCREEN: Mutex<PearHostScreenInfo> = Mutex::new(PearHostScreenInfo {
    point_size: CGSize { width: 0.0, height: 0.0 },
    scale: 2.0,
    max_fps: 60,
});
static G_IDLE_ASSERTION: AtomicUsize = AtomicUsize::new(0);
static G_APP_DELEGATE: AtomicUsize = AtomicUsize::new(0);
static G_DID_FINISH_CB: Mutex<Option<(usize, extern "C" fn(*mut c_void))>> = Mutex::new(None);

unsafe fn default_point_size() -> CGSize {
    if let Ok(val) = std::env::var("PEAR_HOST_SCREEN") {
        let parts: Vec<&str> = val.split('x').collect();
        if parts.len() == 2 {
            if let (Ok(w), Ok(h)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                if w > 0 && h > 0 {
                    return CGSize { width: w as f64, height: h as f64 };
                }
            }
        }
    }
    let main_scr: Id = msg0(cls("NSScreen"), "mainScreen");
    let vis: CGRect = if main_scr.is_null() {
        CGRect { origin: CGPoint::default(), size: CGSize { width: 1280.0, height: 800.0 } }
    } else {
        msg0(main_scr, "visibleFrame")
    };
    let mut width = (vis.size.width * 0.8).floor();
    let mut height = (width * 9.0 / 16.0).floor();
    if height > vis.size.height * 0.85 {
        height = (vis.size.height * 0.85).floor();
        width = (height * 16.0 / 9.0).floor();
    }
    CGSize { width, height }
}

#[no_mangle]
pub extern "C" fn PearBackendInitialize() {
    unsafe {
        let ns_app: Id = msg0(cls("NSApplication"), "sharedApplication");
        msg1::<isize, bool>(ns_app, "setActivationPolicy:", 0);
        G_NATIVE_WINDOWS.lock().unwrap().clear();
        let main_scr: Id = msg0(cls("NSScreen"), "mainScreen");
        let scale: f64 = if main_scr.is_null() { 2.0 } else { msg0(main_scr, "backingScaleFactor") };
        let max_fps: isize = if main_scr.is_null() { 60 } else { msg0(main_scr, "maximumFramesPerSecond") };
        let info = PearHostScreenInfo {
            point_size: default_point_size(),
            scale: if scale > 0.0 { scale } else { 2.0 },
            max_fps: if max_fps > 0 { max_fps } else { 60 },
        };
        *G_SCREEN.lock().unwrap() = info;
        PearBackendInstallQuartzCoreCompat();

        let obs: Id = msg0(cls("PearHostAppObserver"), "new");
        let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        for (notif, sel_name) in [
            ("NSApplicationDidBecomeActiveNotification", "onBecomeActive:"),
            ("NSApplicationWillResignActiveNotification", "onResignActive:"),
            ("NSApplicationDidHideNotification", "onDidHide:"),
            ("NSApplicationWillUnhideNotification", "onWillUnhide:"),
            ("NSApplicationWillTerminateNotification", "onWillTerminate:"),
        ] {
            msg4::<Id, Sel, Id, Id, ()>(nc, "addObserver:selector:name:object:", obs, sel(sel_name), nsstring(notif), NIL);
        }

        let w = info.point_size.width.round() as isize;
        let h = info.point_size.height.round() as isize;
        log_msg("host", &format!("AppKit backend: screen {w}x{h} pt @{:.1}x, {} Hz", info.scale, info.max_fps));
    }
}

#[no_mangle]
pub extern "C" fn PearBackendScreenInfo() -> PearHostScreenInfo {
    *G_SCREEN.lock().unwrap()
}

static HV_UIWIN_KEY: u8 = 0;
static WD_UIWIN_KEY: u8 = 0;
static W_DEL_KEY: u8 = 0;

#[no_mangle]
pub extern "C" fn PearBackendCreateWindow(ui_window: Id, frame: CGRect) -> *mut c_void {
    unsafe {
        let content = CGRect { origin: CGPoint::default(), size: frame.size };
        let w: Id = msg4(
            msg0(cls("NSWindow"), "alloc"),
            "initWithContentRect:styleMask:backing:defer:",
            content,
            (1usize << 0) | (1usize << 1) | (1usize << 2) | (1usize << 3), // titled|closable|miniaturizable|resizable
            2usize, // buffered
            false,
        );
        let info: Id = msg0(msg0(cls("NSBundle"), "mainBundle"), "infoDictionary");
        let disp: Id = msg1(info, "objectForKey:", nsstring("CFBundleDisplayName"));
        let title = if !disp.is_null() {
            disp
        } else {
            msg0(msg0(cls("NSProcessInfo"), "processInfo"), "processName")
        };
        msg1::<Id, ()>(w, "setTitle:", title);
        msg1::<bool, ()>(w, "setReleasedWhenClosed:", false);
        msg1::<bool, ()>(w, "setAcceptsMouseMovedEvents:", true);
        let cb: usize = msg0(w, "collectionBehavior");
        msg1::<usize, ()>(w, "setCollectionBehavior:", cb | (1usize << 7));
        msg1::<CGSize, ()>(w, "setContentAspectRatio:", content.size);

        let v: Id = msg1(msg0(cls("PearHostView"), "alloc"), "initWithFrame:", content);
        assoc_set(v, &HV_UIWIN_KEY, ui_window);
        msg1::<Id, ()>(w, "setContentView:", v);
        msg1::<Id, bool>(w, "makeFirstResponder:", v);

        let d: Id = msg0(cls("PearHostWindowDelegate"), "new");
        assoc_set(d, &WD_UIWIN_KEY, ui_window);
        msg1::<Id, ()>(w, "setDelegate:", d);
        assoc_set(w, &W_DEL_KEY, d);
        objc_release(d);

        msg0::<()>(w, "center");
        G_NATIVE_WINDOWS.lock().unwrap().push(w as usize);
        let _ = G_PRIMARY_VIEW.compare_exchange(0, v as usize, Ordering::AcqRel, Ordering::Acquire);
        objc_release(v);
        w as *mut c_void
    }
}

#[no_mangle]
pub extern "C" fn PearBackendSetRootLayer(handle: *mut c_void, layer: Id) {
    unsafe {
        if handle.is_null() { return; }
        let w = handle as Id;
        let v: Id = msg0(w, "contentView");
        if v.is_null() { return; }
        msg1::<Id, ()>(v, "setLayer:", layer);
        msg1::<bool, ()>(v, "setWantsLayer:", true);
        let scale: f64 = msg0(w, "backingScaleFactor");
        msg1::<f64, ()>(layer, "setContentsScale:", scale);
    }
}

#[no_mangle]
pub extern "C" fn PearBackendShowWindow(handle: *mut c_void, key: bool) {
    unsafe {
        if handle.is_null() { return; }
        let w = handle as Id;
        if key {
            msg1::<Id, ()>(w, "makeKeyAndOrderFront:", NIL);
        } else {
            msg1::<Id, ()>(w, "orderFront:", NIL);
        }
        let ns_app: Id = msg0(cls("NSApplication"), "sharedApplication");
        msg1::<bool, ()>(ns_app, "activateIgnoringOtherApps:", true);
    }
}

#[no_mangle]
pub extern "C" fn PearBackendDisplayLink(target: Id, selector: Sel) -> Id {
    unsafe {
        let pv = G_PRIMARY_VIEW.load(Ordering::Acquire) as Id;
        if !pv.is_null() {
            return msg2(pv, "displayLinkWithTarget:selector:", target, selector);
        }
        let scr: Id = msg0(cls("NSScreen"), "mainScreen");
        if scr.is_null() { NIL } else { msg2(scr, "displayLinkWithTarget:selector:", target, selector) }
    }
}

unsafe extern "C" fn dl_factory(_cls: Id, _c: Sel, target: Id, selector: Sel) -> Id {
    PearBackendDisplayLink(target, selector)
}
static DL_FI_KEY: u8 = 0;
static DL_PFPS_KEY: u8 = 0;
unsafe fn dl_apply_rate(link: Id, raw_fps: f64) {
    let max_fps = PearBackendScreenInfo().max_fps as f64;
    let fps = if raw_fps <= 0.0 || raw_fps > max_fps { max_fps } else { raw_fps } as f32;
    let r = CAFrameRateRange { minimum: fps, maximum: fps, preferred: fps };
    msg1::<CAFrameRateRange, ()>(link, "setPreferredFrameRateRange:", r);
}
unsafe extern "C" fn dl_fi(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &DL_FI_KEY);
    if n.is_null() { 1 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn dl_set_fi(s: Id, _c: Sel, raw: isize) {
    let iv = raw.max(1);
    assoc_set(s, &DL_FI_KEY, nsnumber_i64(iv as i64));
    dl_apply_rate(s, (PearBackendScreenInfo().max_fps as f64) / (iv as f64));
}
unsafe extern "C" fn dl_pfps(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &DL_PFPS_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn dl_set_pfps(s: Id, _c: Sel, fps: isize) {
    assoc_set(s, &DL_PFPS_KEY, nsnumber_i64(fps as i64));
    dl_apply_rate(s, fps as f64);
}

#[no_mangle]
pub extern "C" fn PearBackendInstallQuartzCoreCompat() {
    unsafe {
        let dl_cls = cls("CADisplayLink");
        if dl_cls.is_null() {
            let c1 = CString::new("QuartzCore").unwrap();
            let c2 = CString::new("CADisplayLink class absent (macOS < 14)").unwrap();
            PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
            return;
        }
        if !responds_to(dl_cls as Id, "displayLinkWithTarget:selector:") {
            add_class_method(dl_cls, "displayLinkWithTarget:selector:", dl_factory as Imp, "@@:@:");
            log_msg("host", "+[CADisplayLink displayLinkWithTarget:selector:] provided by the host");
        }
        let scr: Id = msg0(cls("NSScreen"), "mainScreen");
        if scr.is_null() { return; }
        let probe: Id = msg2(scr, "displayLinkWithTarget:selector:", msg0::<Id>(cls("NSNull"), "null"), sel("classForCoder"));
        if probe.is_null() { return; }
        let concrete = object_getClass(probe);
        let table: [(&str, Imp, &str, bool); 4] = [
            ("frameInterval", dl_fi as Imp, "q@:", false),
            ("setFrameInterval:", dl_set_fi as Imp, "v@:q", true),
            ("preferredFramesPerSecond", dl_pfps as Imp, "q@:", false),
            ("setPreferredFramesPerSecond:", dl_set_pfps as Imp, "v@:q", true),
        ];
        for (sname, imp, types, is_setter) in table {
            let s = sel(sname);
            let works = responds_to(probe, sname) && PearUIKitTryProbeDisplayLinkSelector(probe, s, is_setter);
            if !works {
                let ct = CString::new(types).unwrap();
                class_replaceMethod(concrete, s, imp, ct.as_ptr());
                let cn = CStr::from_ptr(class_getName(concrete)).to_string_lossy();
                log_msg("host", &format!("-[{cn} {sname}] provided by the host"));
            }
        }
        msg0::<()>(probe, "invalidate");
    }
}

#[no_mangle]
pub extern "C" fn PearBackendOpenURL(url: Id) -> bool {
    unsafe {
        if url.is_null() { return false; }
        let ws: Id = msg0(cls("NSWorkspace"), "sharedWorkspace");
        msg1(ws, "openURL:", url)
    }
}

#[no_mangle]
pub extern "C" fn PearBackendSetIdleTimerDisabled(disabled: bool) {
    unsafe {
        let cur = G_IDLE_ASSERTION.load(Ordering::Acquire) as u32;
        if disabled && cur == 0 {
            let mut aid: u32 = 0;
            let ty = nsstring("PreventUserIdleDisplaySleep");
            let reason = nsstring("iOS app disabled the idle timer");
            IOPMAssertionCreateWithName(ty as *const c_void, 255, reason as *const c_void, &mut aid);
            G_IDLE_ASSERTION.store(aid as usize, Ordering::Release);
        } else if !disabled && cur != 0 {
            IOPMAssertionRelease(cur);
            G_IDLE_ASSERTION.store(0, Ordering::Release);
        }
    }
}

#[no_mangle]
pub extern "C" fn PearBackendRun(ctx: *mut c_void, did_finish_launching: extern "C" fn(*mut c_void)) {
    unsafe {
        *G_DID_FINISH_CB.lock().unwrap() = Some((ctx as usize, did_finish_launching));
        let del: Id = msg0(cls("PearHostAppDelegate"), "new");
        G_APP_DELEGATE.store(del as usize, Ordering::Release);
        let ns_app: Id = msg0(cls("NSApplication"), "sharedApplication");
        msg1::<Id, ()>(ns_app, "setDelegate:", del);

        let bar: Id = msg0(cls("NSMenu"), "new");
        let app_item: Id = msg0(cls("NSMenuItem"), "new");
        msg1::<Id, ()>(bar, "addItem:", app_item);
        let app_menu: Id = msg0(cls("NSMenu"), "new");
        msg3::<Id, Sel, Id, Id>(app_menu, "addItemWithTitle:action:keyEquivalent:", nsstring("Quit"), sel("terminate:"), nsstring("q"));
        msg1::<Id, ()>(app_item, "setSubmenu:", app_menu);
        msg1::<Id, ()>(ns_app, "setMainMenu:", bar);
        objc_release(app_menu);
        objc_release(app_item);
        objc_release(bar);

        msg0::<()>(ns_app, "run");
    }
}

struct AlertCtx {
    title: String,
    message: String,
    buttons: Vec<String>,
    ctx: usize,
    on_button: extern "C" fn(*mut c_void, isize),
}
extern "C" fn present_alert_main(raw: *mut c_void) {
    unsafe {
        let a = Box::from_raw(raw as *mut AlertCtx);
        let alert: Id = msg0(cls("NSAlert"), "new");
        msg1::<Id, ()>(alert, "setMessageText:", nsstring(if a.title.is_empty() { " " } else { &a.title }));
        msg1::<Id, ()>(alert, "setInformativeText:", nsstring(&a.message));
        if a.buttons.is_empty() {
            msg1::<Id, Id>(alert, "addButtonWithTitle:", nsstring("OK"));
        } else {
            for b in &a.buttons {
                msg1::<Id, Id>(alert, "addButtonWithTitle:", nsstring(b));
            }
        }
        let resp: isize = msg0(alert, "runModal");
        let idx = if a.buttons.is_empty() { -1 } else { resp - 1000 };
        (a.on_button)(a.ctx as *mut c_void, idx);
        objc_release(alert);
    }
}

#[no_mangle]
pub extern "C" fn PearBackendPresentAlert(
    title: *const c_char,
    message: *const c_char,
    buttons: *const *const c_char,
    button_count: usize,
    ctx: *mut c_void,
    on_button: extern "C" fn(*mut c_void, isize),
) {
    unsafe {
        let t = if title.is_null() { String::new() } else { CStr::from_ptr(title).to_string_lossy().into_owned() };
        let m = if message.is_null() { String::new() } else { CStr::from_ptr(message).to_string_lossy().into_owned() };
        let mut btns = Vec::with_capacity(button_count);
        if !buttons.is_null() {
            for i in 0..button_count {
                let p = *buttons.add(i);
                if !p.is_null() {
                    btns.push(CStr::from_ptr(p).to_string_lossy().into_owned());
                }
            }
        }
        let boxed = Box::into_raw(Box::new(AlertCtx { title: t, message: m, buttons: btns, ctx: ctx as usize, on_button }));
        dispatch_async_f(&_dispatch_main_q as *const c_void, boxed as *mut c_void, present_alert_main);
    }
}

#[no_mangle]
pub extern "C" fn PearHIDUsageForMacKeyCode(code: u16) -> isize {
    match code {
        0x00 => 0x04, 0x0B => 0x05, 0x08 => 0x06, 0x02 => 0x07, 0x0E => 0x08, 0x03 => 0x09, 0x05 => 0x0A,
        0x04 => 0x0B, 0x22 => 0x0C, 0x26 => 0x0D, 0x28 => 0x0E, 0x25 => 0x0F, 0x2E => 0x10, 0x2D => 0x11,
        0x1F => 0x12, 0x23 => 0x13, 0x0C => 0x14, 0x0F => 0x15, 0x01 => 0x16, 0x11 => 0x17, 0x20 => 0x18,
        0x09 => 0x19, 0x0D => 0x1A, 0x07 => 0x1B, 0x10 => 0x1C, 0x06 => 0x1D,
        0x12 => 0x1E, 0x13 => 0x1F, 0x14 => 0x20, 0x15 => 0x21, 0x17 => 0x22, 0x16 => 0x23, 0x1A => 0x24,
        0x1C => 0x25, 0x19 => 0x26, 0x1D => 0x27,
        0x24 => 0x28, 0x35 => 0x29, 0x33 => 0x2A, 0x30 => 0x2B, 0x31 => 0x2C, 0x1B => 0x2D, 0x18 => 0x2E,
        0x21 => 0x2F, 0x1E => 0x30, 0x2A => 0x31, 0x29 => 0x33, 0x27 => 0x34, 0x32 => 0x35, 0x2B => 0x36,
        0x2F => 0x37, 0x2C => 0x38, 0x39 => 0x39, 0x0A => 0x64,
        0x7A => 0x3A, 0x78 => 0x3B, 0x63 => 0x3C, 0x76 => 0x3D, 0x60 => 0x3E, 0x61 => 0x3F, 0x62 => 0x40,
        0x64 => 0x41, 0x65 => 0x42, 0x6D => 0x43, 0x67 => 0x44, 0x6F => 0x45,
        0x73 => 0x4A, 0x74 => 0x4B, 0x75 => 0x4C, 0x77 => 0x4D, 0x79 => 0x4E,
        0x7C => 0x4F, 0x7B => 0x50, 0x7D => 0x51, 0x7E => 0x52,
        0x47 => 0x53, 0x4B => 0x54, 0x43 => 0x55, 0x4E => 0x56, 0x45 => 0x57, 0x4C => 0x58, 0x53 => 0x59,
        0x54 => 0x5A, 0x55 => 0x5B, 0x56 => 0x5C, 0x57 => 0x5D, 0x58 => 0x5E, 0x59 => 0x5F, 0x5B => 0x60,
        0x5C => 0x61, 0x52 => 0x62, 0x41 => 0x63, 0x51 => 0x67,
        0x3B => 0xE0, 0x38 => 0xE1, 0x3A => 0xE2, 0x37 => 0xE3, 0x3E => 0xE4, 0x3C => 0xE5, 0x3D => 0xE6,
        0x36 => 0xE7,
        _ => 0,
    }
}

// PearHostAppDelegate / PearHostAppObserver / PearHostWindowDelegate / PearHostView methods
unsafe extern "C" fn app_del_did_finish(_s: Id, _c: Sel, _n: Id) {
    if let Some((ctx, cb)) = G_DID_FINISH_CB.lock().unwrap().take() {
        cb(ctx as *mut c_void);
    }
}
unsafe extern "C" fn app_del_should_term(_s: Id, _c: Sel, _app: Id) -> bool { true }
unsafe extern "C" fn obs_active(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationDidBecomeActive(); }
unsafe extern "C" fn obs_resign(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationWillResignActive(); }
unsafe extern "C" fn obs_hide(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationDidEnterBackground(); }
unsafe extern "C" fn obs_unhide(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationWillEnterForeground(); }
unsafe extern "C" fn obs_term(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationWillTerminate(); }

unsafe extern "C" fn wd_did_resize(s: Id, _c: Sel, notif: Id) {
    let w: Id = msg0(notif, "object");
    let ui = assoc_get(s, &WD_UIWIN_KEY);
    if w.is_null() || ui.is_null() { return; }
    let cv: Id = msg0(w, "contentView");
    if cv.is_null() { return; }
    let b: CGRect = msg0(cv, "bounds");
    let scale: f64 = msg0(w, "backingScaleFactor");
    PearUIKitWindowDidResize(ui, b.size, scale);
}
unsafe extern "C" fn wd_became_key(s: Id, _c: Sel, _notif: Id) {
    PearUIKitSetKeyWindow(assoc_get(s, &WD_UIWIN_KEY));
}

unsafe extern "C" fn hv_true(_s: Id, _c: Sel) -> bool { true }
unsafe extern "C" fn hv_accepts_mouse(_s: Id, _c: Sel, _e: Id) -> bool { true }
unsafe fn hv_pointer(view: Id, event: Id, phase: isize) {
    let loc: CGPoint = msg0(event, "locationInWindow");
    let p: CGPoint = msg2(view, "convertPoint:fromView:", loc, NIL);
    let w = assoc_get(view, &HV_UIWIN_KEY);
    if w.is_null() { return; }
    let wb: CGRect = msg0(w, "bounds");
    let vb: CGRect = msg0(view, "bounds");
    let ui = CGPoint {
        x: p.x * wb.size.width / vb.size.width.max(1.0),
        y: p.y * wb.size.height / vb.size.height.max(1.0),
    };
    let ts: f64 = msg0(event, "timestamp");
    PearUIKitDeliverPointer(w, phase, ui, ts);
}
unsafe extern "C" fn hv_mouse_down(s: Id, _c: Sel, e: Id) { hv_pointer(s, e, 0); }
unsafe extern "C" fn hv_mouse_drag(s: Id, _c: Sel, e: Id) { hv_pointer(s, e, 1); }
unsafe extern "C" fn hv_mouse_up(s: Id, _c: Sel, e: Id) { hv_pointer(s, e, 3); }
unsafe fn hv_send_key(event: Id, down: bool) {
    let code: u16 = msg0(event, "keyCode");
    let usage = PearHIDUsageForMacKeyCode(code);
    if usage == 0 { return; }
    let flags: usize = msg0(event, "modifierFlags");
    let mods = (flags & 0xFFFF_0000) as isize;
    let chars: Id = msg0(event, "characters");
    let nomod: Id = msg0(event, "charactersIgnoringModifiers");
    let ts: f64 = msg0(event, "timestamp");
    PearUIKitDeliverKey(usage, chars, nomod, mods, down, ts);
}
unsafe extern "C" fn hv_key_down(s: Id, _c: Sel, e: Id) {
    if PearUIKitTextInputActive() {
        msg1::<Id, ()>(s, "interpretKeyEvents:", nsarray_from_ids(&[e]));
        return;
    }
    if msg0::<bool>(e, "isARepeat") { return; }
    hv_send_key(e, true);
}
unsafe extern "C" fn hv_key_up(_s: Id, _c: Sel, e: Id) {
    if PearUIKitTextInputActive() { return; }
    hv_send_key(e, false);
}
unsafe extern "C" fn hv_flags_changed(_s: Id, _c: Sel, e: Id) {
    let code: u16 = msg0(e, "keyCode");
    let usage = PearHIDUsageForMacKeyCode(code);
    let bit: usize = match usage {
        0xE1 | 0xE5 => 1 << 17,
        0xE0 | 0xE4 => 1 << 18,
        0xE2 | 0xE6 => 1 << 19,
        0xE3 | 0xE7 => 1 << 20,
        0x39 => 1 << 16,
        _ => return,
    };
    let flags: usize = msg0(e, "modifierFlags");
    let down = (flags & bit) != 0;
    let mods = (flags & 0xFFFF_0000) as isize;
    let ts: f64 = msg0(e, "timestamp");
    PearUIKitDeliverKey(usage, nsstring(""), nsstring(""), mods, down, ts);
}
unsafe extern "C" fn hv_insert_text(_s: Id, _c: Sel, obj: Id) {
    let str_id = if is_kind_of(obj, "NSAttributedString") { msg0(obj, "string") } else { obj };
    if is_kind_of(str_id, "NSString") && msg0::<usize>(str_id, "length") > 0 {
        PearUIKitDeliverText(str_id);
    }
}
unsafe extern "C" fn hv_do_cmd(_s: Id, _c: Sel, selector: Sel) {
    let name = CStr::from_ptr(sel_getName(selector)).to_string_lossy();
    match name.as_ref() {
        "deleteBackward:" => PearUIKitDeliverEditCommand(0),
        "insertNewline:" | "insertLineBreak:" => PearUIKitDeliverEditCommand(1),
        "moveLeft:" => PearUIKitDeliverEditCommand(2),
        "moveRight:" => PearUIKitDeliverEditCommand(3),
        "cancelOperation:" => PearUIKitDeliverEditCommand(4),
        "insertTab:" => PearUIKitDeliverText(nsstring("\t")),
        _ => {}
    }
}

// ---- AppKit helper classes ---------------------------------------------------

unsafe fn register_classes() {
    // AppKit helper classes
    if cls("PearHostAppDelegate").is_null() {
        let name = CString::new("PearHostAppDelegate").unwrap();
        let c = objc_allocateClassPair(cls("NSObject"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "applicationDidFinishLaunching:", app_del_did_finish as Imp, "v@:@");
            add_method(c, "applicationShouldTerminateAfterLastWindowClosed:", app_del_should_term as Imp, "B@:@");
            objc_registerClassPair(c);
        }
    }
    if cls("PearHostAppObserver").is_null() {
        let name = CString::new("PearHostAppObserver").unwrap();
        let c = objc_allocateClassPair(cls("NSObject"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "onBecomeActive:", obs_active as Imp, "v@:@");
            add_method(c, "onResignActive:", obs_resign as Imp, "v@:@");
            add_method(c, "onDidHide:", obs_hide as Imp, "v@:@");
            add_method(c, "onWillUnhide:", obs_unhide as Imp, "v@:@");
            add_method(c, "onWillTerminate:", obs_term as Imp, "v@:@");
            objc_registerClassPair(c);
        }
    }
    if cls("PearHostWindowDelegate").is_null() {
        let name = CString::new("PearHostWindowDelegate").unwrap();
        let c = objc_allocateClassPair(cls("NSObject"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "windowDidResize:", wd_did_resize as Imp, "v@:@");
            add_method(c, "windowDidChangeBackingProperties:", wd_did_resize as Imp, "v@:@");
            add_method(c, "windowDidBecomeKey:", wd_became_key as Imp, "v@:@");
            objc_registerClassPair(c);
        }
    }
    if cls("PearHostView").is_null() {
        let name = CString::new("PearHostView").unwrap();
        let c = objc_allocateClassPair(cls("NSView"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "isFlipped", hv_true as Imp, "B@:");
            add_method(c, "acceptsFirstResponder", hv_true as Imp, "B@:");
            add_method(c, "acceptsFirstMouse:", hv_accepts_mouse as Imp, "B@:@");
            add_method(c, "wantsUpdateLayer", hv_true as Imp, "B@:");
            add_method(c, "mouseDown:", hv_mouse_down as Imp, "v@:@");
            add_method(c, "mouseDragged:", hv_mouse_drag as Imp, "v@:@");
            add_method(c, "mouseUp:", hv_mouse_up as Imp, "v@:@");
            add_method(c, "keyDown:", hv_key_down as Imp, "v@:@");
            add_method(c, "keyUp:", hv_key_up as Imp, "v@:@");
            add_method(c, "flagsChanged:", hv_flags_changed as Imp, "v@:@");
            add_method(c, "insertText:", hv_insert_text as Imp, "v@:@");
            add_method(c, "doCommandBySelector:", hv_do_cmd as Imp, "v@::");
            objc_registerClassPair(c);
        }
    }
}

// ---- pear-host ---------------------------------------------------------------

/// NSPasteboard for a UIPasteboard name (None: the general pasteboard).
unsafe fn pasteboard(name: Option<&str>) -> Id {
    match name {
        None => msg0(cls("NSPasteboard"), "generalPasteboard"),
        Some(n) => msg1(cls("NSPasteboard"), "pasteboardWithName:", nsstring(n)),
    }
}

pub struct MacHost;

extern "C" fn alert_reply(ctx: *mut c_void, index: isize) {
    let reply = unsafe { Box::from_raw(ctx as *mut Box<dyn FnOnce(isize) + Send>) };
    reply(index);
}

extern "C" fn did_finish(ctx: *mut c_void) {
    let f = unsafe { Box::from_raw(ctx as *mut Box<dyn FnOnce()>) };
    f();
}

impl HostPlatform for MacHost {
    fn initialize(&self) {
        PearBackendInitialize();
    }

    fn screen(&self) -> ScreenInfo {
        let info = PearBackendScreenInfo();
        ScreenInfo {
            points: Size { width: info.point_size.width, height: info.point_size.height },
            scale: info.scale,
            max_fps: info.max_fps.max(0) as u32,
        }
    }

    fn create_window(&self, owner: GuestObject, frame: Rect) -> WindowHandle {
        let rect = CGRect {
            origin: CGPoint { x: frame.x, y: frame.y },
            size: CGSize { width: frame.width, height: frame.height },
        };
        WindowHandle(PearBackendCreateWindow(owner.0 as Id, rect))
    }

    fn set_root_layer(&self, window: WindowHandle, layer: LayerRef) {
        PearBackendSetRootLayer(window.0, layer.0 as Id);
    }

    fn show_window(&self, window: WindowHandle, key: bool) {
        PearBackendShowWindow(window.0, key);
    }

    fn display_link(&self, target: GuestObject, selector: GuestSelector) -> GuestObject {
        GuestObject(PearBackendDisplayLink(target.0 as Id, selector.0 as pear_objc::Sel))
    }

    fn open_url(&self, url: &str) -> bool {
        unsafe {
            let ns_url: Id = msg1(cls("NSURL"), "URLWithString:", nsstring(url));
            PearBackendOpenURL(ns_url)
        }
    }

    fn set_idle_timer_disabled(&self, disabled: bool) {
        PearBackendSetIdleTimerDisabled(disabled);
    }

    fn present_alert(&self, title: &str, message: &str, buttons: &[String], reply: Box<dyn FnOnce(isize) + Send>) {
        let title = CString::new(title.replace('\0', "")).unwrap_or_default();
        let message = CString::new(message.replace('\0', "")).unwrap_or_default();
        let owned: Vec<CString> = buttons.iter().map(|b| CString::new(b.replace('\0', "")).unwrap_or_default()).collect();
        let pointers: Vec<*const c_char> = owned.iter().map(|b| b.as_ptr()).collect();
        let ctx = Box::into_raw(Box::new(reply)) as *mut c_void;
        // The backend copies the strings before returning.
        PearBackendPresentAlert(title.as_ptr(), message.as_ptr(), pointers.as_ptr(), pointers.len(), ctx, alert_reply);
    }

    fn pasteboard_read(&self, name: Option<&str>) -> Vec<PasteboardItem> {
        unsafe {
            let items: Id = msg0(pasteboard(name), "pasteboardItems");
            let mut out = Vec::new();
            for i in 0..nsarray_count(items) {
                let item = nsarray_object_at(items, i);
                let read = |uti: &str| nsstring_to_string(msg1(item, "stringForType:", nsstring(uti)));
                let url = read(UTI_FILE_URL).or_else(|| read(UTI_URL));
                let text = read(UTI_TEXT);
                if let Some(u) = &url {
                    out.push(PasteboardItem::Url(u.clone()));
                }
                if let Some(s) = text {
                    if url.as_deref() != Some(s.as_str()) {
                        out.push(PasteboardItem::Text(s));
                    }
                }
            }
            out
        }
    }

    fn pasteboard_types(&self, name: Option<&str>) -> Vec<String> {
        unsafe {
            let types: Id = msg0(pasteboard(name), "types");
            (0..nsarray_count(types)).filter_map(|i| nsstring_to_string(nsarray_object_at(types, i))).collect()
        }
    }

    fn pasteboard_data(&self, name: Option<&str>, uti: &str) -> Option<Vec<u8>> {
        unsafe { nsdata_to_vec(msg1(pasteboard(name), "dataForType:", nsstring(uti))) }
    }

    fn pasteboard_write(&self, name: Option<&str>, items: &[PasteboardItem]) {
        unsafe {
            let pb = pasteboard(name);
            msg0::<isize>(pb, "clearContents");
            if items.is_empty() {
                return;
            }
            // NSString/NSURL write their standard representations; other
            // types go into one pasteboard item.
            let mut objects = Vec::new();
            let data_item: Id = msg0(cls("NSPasteboardItem"), "new");
            let mut has_data = false;
            for item in items {
                match item {
                    PasteboardItem::Text(s) => objects.push(nsstring(s)),
                    PasteboardItem::Url(u) => {
                        let url: Id = msg1(cls("NSURL"), "URLWithString:", nsstring(u));
                        if !url.is_null() {
                            objects.push(url);
                        }
                    }
                    PasteboardItem::Data { uti, bytes } => {
                        msg2::<Id, Id, bool>(data_item, "setData:forType:", nsdata_from_bytes(bytes), nsstring(uti));
                        has_data = true;
                    }
                }
            }
            if has_data {
                objects.push(data_item);
            }
            msg1::<Id, bool>(pb, "writeObjects:", nsarray_from_ids(&objects));
            objc_release(data_item);
        }
    }

    fn pasteboard_change_count(&self, name: Option<&str>) -> isize {
        unsafe { msg0(pasteboard(name), "changeCount") }
    }

    fn pick_documents(&self, request: DocumentRequest, reply: Box<dyn FnOnce(Option<Vec<String>>) + Send>) {
        unsafe {
            let url_string = |u: Id| nsstring_to_string(msg0(u, "absoluteString"));
            let directory = |d: &Option<String>| -> Id {
                d.as_deref().map(|s| msg1(cls("NSURL"), "URLWithString:", nsstring(s))).unwrap_or(NIL)
            };
            let chosen = match &request {
                DocumentRequest::Save { suggested_name, directory: dir } => {
                    let panel: Id = msg0(cls("NSSavePanel"), "savePanel");
                    let d = directory(dir);
                    if !d.is_null() {
                        msg1::<Id, ()>(panel, "setDirectoryURL:", d);
                    }
                    msg1::<Id, ()>(panel, "setNameFieldStringValue:", nsstring(suggested_name));
                    (msg0::<isize>(panel, "runModal") == 1)
                        .then(|| url_string(msg0(panel, "URL")).into_iter().collect::<Vec<_>>())
                }
                DocumentRequest::Open { multiple, directory: dir } => {
                    let panel: Id = msg0(cls("NSOpenPanel"), "openPanel");
                    msg1::<bool, ()>(panel, "setAllowsMultipleSelection:", *multiple);
                    let d = directory(dir);
                    if !d.is_null() {
                        msg1::<Id, ()>(panel, "setDirectoryURL:", d);
                    }
                    (msg0::<isize>(panel, "runModal") == 1).then(|| {
                        let urls: Id = msg0(panel, "URLs");
                        (0..nsarray_count(urls)).filter_map(|i| url_string(nsarray_object_at(urls, i))).collect()
                    })
                }
            };
            reply(chosen);
        }
    }

    fn share(&self, items: &[PasteboardItem]) {
        unsafe {
            let objects: Vec<Id> = items
                .iter()
                .filter_map(|item| match item {
                    PasteboardItem::Text(s) => Some(nsstring(s)),
                    PasteboardItem::Url(u) => {
                        let url: Id = msg1(cls("NSURL"), "URLWithString:", nsstring(u));
                        (!url.is_null()).then_some(url)
                    }
                    PasteboardItem::Data { .. } => None,
                })
                .collect();
            let picker: Id = msg1(msg0(cls("NSSharingServicePicker"), "alloc"), "initWithItems:", nsarray_from_ids(&objects));
            let ns_app: Id = msg0(cls("NSApplication"), "sharedApplication");
            let key: Id = msg0(ns_app, "keyWindow");
            let content: Id = if key.is_null() { NIL } else { msg0(key, "contentView") };
            if !content.is_null() {
                let b: CGRect = msg0(content, "bounds");
                let anchor = CGRect {
                    origin: CGPoint { x: b.origin.x + b.size.width * 0.5, y: b.origin.y + b.size.height * 0.5 },
                    size: CGSize { width: 1.0, height: 1.0 },
                };
                msg3::<CGRect, Id, usize, ()>(picker, "showRelativeToRect:ofView:preferredEdge:", anchor, content, 1);
            }
            objc_release(picker);
        }
    }

    fn screen_reader_running(&self) -> bool {
        unsafe {
            let workspace: Id = msg0(cls("NSWorkspace"), "sharedWorkspace");
            !workspace.is_null() && msg0::<bool>(workspace, "isVoiceOverEnabled")
        }
    }

    fn run(&self, did_finish_launching: Box<dyn FnOnce()>) {
        let ctx = Box::into_raw(Box::new(did_finish_launching)) as *mut c_void;
        PearBackendRun(ctx, did_finish);
    }
}
