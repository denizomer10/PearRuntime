//! How a touch gets into an iOS app running on arm64 macOS.
//!
//!   * UINSVirtualDigitizer (default, working) — macOS's own virtual-touch
//!     machinery (the one behind Touch Alternatives). Each binding drives a
//!     named virtual finger, so real multi-touch works and the system cursor is
//!     untouched.
//!   * CGEvent (opt-in via PEAR_TOUCH_CGEVENT) — post a real system mouse event
//!     at the game window's *screen* position. Only one pointer exists, and the
//!     post moves the real cursor (stashed/restored around a tap).
//!   * IOHIDEvent (experimental, opt-in via PEAR_TOUCH_IOHID) — build a
//!     digitizer event and hand it to UIKit's own event pipeline. Inert on
//!     macOS: touches arrive through the UIKitMac glue.
//!
//! The private symbols are reached dynamically (the SDK has no headers).

use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsstring, sel, CGPoint, CGRect, CGSize, Id, Imp, Sel};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const NIL: Id = std::ptr::null_mut();
type Ev = *mut c_void;

extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn mach_absolute_time() -> u64;
    fn CFRelease(cf: *const c_void);
    fn CFArrayGetCount(a: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(a: *const c_void, i: isize) -> *const c_void;
    fn CACurrentMediaTime() -> f64;
    fn NSStringFromCGRect(r: CGRect) -> Id;
    fn CGWindowListCopyWindowInfo(option: u32, relative: u32) -> *const c_void;
    static kCGWindowOwnerPID: Id;
    static kCGWindowBounds: Id;
    fn CGEventCreateMouseEvent(source: *const c_void, kind: u32, p: CGPoint, button: u32) -> *mut c_void;
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(e: *const c_void) -> CGPoint;
    fn CGEventPost(tap: u32, e: *const c_void);
    fn CGEventPostToPid(pid: i32, e: *const c_void);
    fn CGWarpMouseCursorPosition(p: CGPoint) -> i32;
    fn CGAssociateMouseAndMouseCursorPosition(connected: bool) -> i32;
    fn getpid() -> i32;
    fn usleep(us: u32) -> c_int;
    fn os_unfair_lock_lock(lock: *mut u32);
    fn os_unfair_lock_unlock(lock: *mut u32);
    fn class_getInstanceVariable(c: Id, name: *const c_char) -> *mut c_void;
    fn ivar_getOffset(ivar: *mut c_void) -> isize;
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

// ---------------------------------------------------------------------------
// The primary window

/// Scene-less legacy apps can have only the app delegate's UIWindow: keep the
/// context ID and coordinate dimensions tied to the same actual window.
#[export_name = "_Z17PearPrimaryWindowv"]
pub unsafe extern "C" fn primary_window() -> Id {
    let app: Id = msg0(cls("UIApplication"), "sharedApplication");
    let mut first_visible: Id = NIL;
    let usable = |w: Id| !msg0::<bool>(w, "isHidden") && !msg0::<Id>(w, "rootViewController").is_null();
    let scenes: Id = msg0(msg0::<Id>(app, "connectedScenes"), "allObjects");
    for i in 0..pear_objc::nsarray_count(scenes) {
        let scene = pear_objc::nsarray_object_at(scenes, i);
        if !pear_objc::is_kind_of(scene, "UIWindowScene") {
            continue;
        }
        let windows: Id = msg0(scene, "windows");
        for j in 0..pear_objc::nsarray_count(windows) {
            let w = pear_objc::nsarray_object_at(windows, j);
            // Catalyst can create a key host window without game content: it
            // must not displace the app's real rendering/input window.
            if usable(w) && msg0::<bool>(w, "isKeyWindow") {
                return w;
            }
            if usable(w) && first_visible.is_null() {
                first_visible = w;
            }
        }
    }
    if !first_visible.is_null() {
        return first_visible;
    }
    // Existing legacy windows only: an app delegate's lazy window getter can
    // construct UIKit while its traits initialize.
    let windows: Id = msg0(app, "windows");
    for i in 0..pear_objc::nsarray_count(windows) {
        let w = pear_objc::nsarray_object_at(windows, i);
        if !usable(w) {
            continue;
        }
        if msg0::<bool>(w, "isKeyWindow") {
            return w;
        }
        if first_visible.is_null() {
            first_visible = w;
        }
    }
    first_visible
}

// ---------------------------------------------------------------------------
// IOHIDEvent (experimental)

#[derive(Clone, Copy)]
struct Hid {
    create_finger: Option<unsafe extern "C" fn(*const c_void, u64, u32, u32, u32, i32, i32, i32, i32, i32, u8, u8, usize) -> Ev>,
    create_hand: Option<unsafe extern "C" fn(*const c_void, u64, u32, u32, u32, u32, u32, i32, i32, i32, i32, i32, u8, u8, usize) -> Ev>,
    create: Option<unsafe extern "C" fn(*const c_void, c_int, u64) -> Ev>,
    client_create: Option<unsafe extern "C" fn(*const c_void) -> *mut c_void>,
    client_dispatch: Option<unsafe extern "C" fn(*mut c_void, Ev)>,
    create_copy: Option<unsafe extern "C" fn(*const c_void, Ev) -> Ev>,
    append: Option<unsafe extern "C" fn(Ev, Ev)>,
    set_integer: Option<unsafe extern "C" fn(Ev, u32, isize)>,
    set_float: Option<unsafe extern "C" fn(Ev, u32, f64)>,
    set_sender: Option<unsafe extern "C" fn(Ev, u32)>,
    get_integer: Option<unsafe extern "C" fn(Ev, u32) -> isize>,
    get_float: Option<unsafe extern "C" fn(Ev, u32) -> f64>,
    get_children: Option<unsafe extern "C" fn(Ev) -> *const c_void>,
    get_timestamp: Option<unsafe extern "C" fn(Ev) -> u64>,
    get_type: Option<unsafe extern "C" fn(Ev) -> c_int>,
    get_sender: Option<unsafe extern "C" fn(Ev) -> u32>,
    ready: bool,
}
unsafe impl Send for Hid {}
unsafe impl Sync for Hid {}

fn ptr<T>(o: &Option<T>) -> *const c_void {
    match o {
        Some(f) => unsafe { std::mem::transmute_copy(f) },
        None => std::ptr::null(),
    }
}

fn hid() -> &'static Hid {
    static H: OnceLock<Hid> = OnceLock::new();
    H.get_or_init(|| unsafe {
        let iokit = rt::dlopen_path("/System/Library/Frameworks/IOKit.framework/IOKit", rt::RTLD_LAZY);
        macro_rules! f {
            ($n:literal) => {{
                let p = dlsym(iokit, $n.as_ptr());
                if p.is_null() { None } else { Some(std::mem::transmute(p)) }
            }};
        }
        let mut h = Hid {
            create_finger: f!(c"IOHIDEventCreateDigitizerFingerEvent"),
            create_hand: f!(c"IOHIDEventCreateDigitizerEvent"),
            create: f!(c"IOHIDEventCreate"),
            client_create: f!(c"IOHIDEventSystemClientCreate"),
            client_dispatch: f!(c"IOHIDEventSystemClientDispatchEvent"),
            create_copy: f!(c"IOHIDEventCreateCopy"),
            append: f!(c"IOHIDEventAppendEvent"),
            set_integer: f!(c"IOHIDEventSetIntegerValue"),
            set_float: f!(c"IOHIDEventSetFloatValue"),
            set_sender: f!(c"IOHIDEventSetSenderID"),
            get_integer: f!(c"IOHIDEventGetIntegerValue"),
            get_float: f!(c"IOHIDEventGetFloatValue"),
            get_children: f!(c"IOHIDEventGetChildren"),
            get_timestamp: f!(c"IOHIDEventGetTimeStamp"),
            get_type: f!(c"IOHIDEventGetType"),
            get_sender: f!(c"IOHIDEventGetSenderID"),
            ready: false,
        };
        h.ready = h.create_finger.is_some() && h.create_hand.is_some() && h.append.is_some() && h.set_integer.is_some();
        rt::log(&format!(
            "[PearRuntime][touch] IOHIDEvent ready={} (finger={:p} hand={:p} append={:p})",
            h.ready as i32,
            ptr(&h.create_finger),
            ptr(&h.create_hand),
            ptr(&h.append)
        ));
        h
    })
}

/// Prints an IOHIDEvent tree (type, sender, timestamp, digitizer fields) to
/// compare a real delivered event with a synthesized one.
unsafe fn dump_hid_tree(event: Ev, depth: usize) {
    if event.is_null() {
        return;
    }
    let h = hid();
    let pad = "  ".repeat(depth);
    rt::log(&format!(
        "[PearRuntime][hid] {pad}event type={} sender={} timestamp={}",
        h.get_type.map(|f| f(event)).unwrap_or(-1),
        h.get_sender.map(|f| f(event)).unwrap_or(0),
        h.get_timestamp.map(|f| f(event)).unwrap_or(0)
    ));
    for i in 0u32..0x30 {
        let field = (11u32 << 16) | i;
        let iv = h.get_integer.map(|f| f(event, field)).unwrap_or(0);
        let fv = h.get_float.map(|f| f(event, field)).unwrap_or(0.0);
        if iv != 0 || fv != 0.0 {
            rt::log(&format!("[PearRuntime][hid] {pad}  d[0x{i:02x}] int={iv} float={fv:.6}"));
        }
    }
    let children = h.get_children.map(|f| f(event)).unwrap_or(std::ptr::null());
    if !children.is_null() {
        for i in 0..CFArrayGetCount(children) {
            dump_hid_tree(CFArrayGetValueAtIndex(children, i) as Ev, depth + 1);
        }
    }
}

unsafe fn window_size(w: Id) -> CGSize {
    msg0::<CGRect>(w, "bounds").size
}

/// Synthesizes a touch at a normalized (0..1) point in the key window.
#[export_name = "_Z19PearSynthesizeTouchbdd"]
pub unsafe extern "C" fn synthesize_touch(down: bool, nx: f64, ny: f64) {
    let h = *hid();
    if !h.ready {
        return;
    }
    let app: Id = msg0(cls("UIApplication"), "sharedApplication");
    let enqueue = sel("_enqueueHIDEvent:");
    let has_selector: bool = msg1(app, "respondsToSelector:", enqueue);
    let key = primary_window();
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        let frame = if key.is_null() { "nil".to_string() } else { rt::ns_string(NSStringFromCGRect(msg0(key, "frame"))) };
        rt::log(&format!(
            "[PearRuntime][touch] synthesize down={} state={} keyWindow={frame} sel={}",
            down as i32,
            msg0::<isize>(app, "applicationState"),
            has_selector as i32
        ));
    }
    if !has_selector {
        return;
    }
    // Field layout and values from a *real* delivered digitizer event (type 11):
    //   root (sender 10572147)  d[4]=3 d[7]=2 d[8]=1 d[9]=touch d[0x18]=6 d[0x19]=1
    //     ├ finger              d[0]=x d[1]=y d[4]=2 d[5]=19 d[7]=6 d[8]=1 d[9]=touch
    //     │                     d[0x11..0x13]=1 d[0x14]=0.015 d[0x15]=5 d[0x1f]=2
    //     ├ (type 17, empty)
    //     └ (type 1,  empty)
    // Coordinates are in the window's UIKit point space.
    let base = 11u32 << 16;
    let now = mach_absolute_time();
    let pts = if key.is_null() { CGSize { width: 1000.0, height: 1000.0 } } else { window_size(key) };
    let (x, y) = (nx * pts.width, ny * pts.height);
    let (si, sf, append) = (h.set_integer.unwrap(), h.set_float, h.append.unwrap());
    let d = down as u8;
    let finger = (h.create_finger.unwrap())(std::ptr::null(), now, 0, 0, 6, 0, 0, 0, 0, 0, d, d, 0);
    if finger.is_null() {
        rt::log("[PearRuntime][touch] finger event nil");
        return;
    }
    for (field, v) in [(0x04, 2), (0x05, 19), (0x07, 6), (0x08, 1), (0x09, down as isize)] {
        si(finger, base | field, v);
    }
    if let Some(sf) = sf {
        sf(finger, base, x);
        sf(finger, base | 0x01, y);
    }
    for field in [0x11, 0x12, 0x13] {
        si(finger, base | field, 1);
    }
    if let Some(sf) = sf {
        sf(finger, base | 0x14, 0.015);
    }
    si(finger, base | 0x15, 5);
    si(finger, base | 0x1f, 2);

    let root = (h.create_hand.unwrap())(std::ptr::null(), now, 3, 0, 0, 2, 0, 0, 0, 0, 0, 0, d, d, 0);
    for (field, v) in [(0x04, 3), (0x07, 2), (0x08, 1), (0x09, down as isize), (0x18, 6), (0x19, 1)] {
        si(root, base | field, v);
    }
    append(root, finger);
    // The two trailing children a real event carries.
    if let Some(create) = h.create {
        for t in [17, 1] {
            let c = create(std::ptr::null(), t, now);
            if !c.is_null() {
                append(root, c);
            }
        }
    }
    if let Some(s) = h.set_sender {
        s(root, 10572147);
    }
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!(
            "[PearRuntime][touch] synthesize down={} nx={nx:.3} ny={ny:.3} -> ({x:.1},{y:.1}) of ({:.0},{:.0})",
            down as i32, pts.width, pts.height
        ));
    }
    if rt::env_set("PEAR_DUMP_HIDEVENT") {
        rt::log("[PearRuntime][hid] ---- synthetic toSend ----");
        dump_hid_tree(root, 0);
    }
    // PEAR_TOUCH_SINK: handle (_handleHIDEvent:), enqueue (_enqueueHIDEvent:),
    // direct (_handleHIDEventBypassingUIEvent:), dispatch (HID system client).
    let sink_name = env("PEAR_TOUCH_SINK");
    if sink_name.as_deref() == Some("dispatch") {
        static CLIENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
        if CLIENT.load(Ordering::Acquire).is_null() {
            if let Some(create) = h.client_create {
                CLIENT.store(create(std::ptr::null()), Ordering::Release);
            }
        }
        let client = CLIENT.load(Ordering::Acquire);
        match (client.is_null(), h.client_dispatch) {
            (false, Some(dispatch)) => {
                dispatch(client, root);
                rt::log(&format!("[PearRuntime][touch] dispatched via IOHIDEventSystemClient={client:p}"));
            }
            _ => rt::log(&format!(
                "[PearRuntime][touch] dispatch unavailable (create={:p} dispatch={:p})",
                ptr(&h.client_create),
                ptr(&h.client_dispatch)
            )),
        }
        CFRelease(root);
        return;
    }
    let mut sink = sel("_handleHIDEvent:");
    match sink_name.as_deref() {
        Some("enqueue") => sink = enqueue,
        Some("direct") => sink = sel("_handleHIDEventBypassingUIEvent:"),
        _ => {}
    }
    if !msg1::<Sel, bool>(app, "respondsToSelector:", sink) {
        sink = enqueue;
    }
    msg2::<Sel, Id, Id>(app, "performSelector:withObject:", sink, root as Id);
    CFRelease(root);
}

#[export_name = "_Z24PearSynthesizeTouchBeginv"]
pub extern "C" fn synthesize_touch_begin() {
    hid();
}

// ---------------------------------------------------------------------------
// Virtual digitizer (macOS Catalyst)
//
// The app's touches flow through the UIKitMac glue, which ships its own way to
// synthesize touches: UINSVirtualDigitizer / UINSVirtualFinger.
//   [digitizer addVirtualFingerAtLoc:forKey:]
//   [digitizer modifyVirtualFingerForKey:withBlock:^(f){ [f setTouching:YES]; }]
//   [digitizer _postHIDDigitizerCollectionToUIKitWithTimestamp:]

static DIGITIZER: AtomicUsize = AtomicUsize::new(0);

unsafe fn virtual_digitizer() -> Id {
    let existing = DIGITIZER.load(Ordering::Acquire);
    if existing != 0 {
        return existing as Id;
    }
    let c = rt::class_named("UINSVirtualDigitizer");
    if c.is_null() {
        rt::log("[PearRuntime][touch] UINSVirtualDigitizer not found");
        return NIL;
    }
    let digitizer: Id = msg0(msg0::<Id>(c, "alloc"), "init");
    if digitizer.is_null() {
        return NIL;
    }
    // Bind to the key window's context so the touch lands in the right scene;
    // without one UIKit's dispatcher crashes, so wait for a key window.
    let key = primary_window();
    if key.is_null() || !pear_objc::responds_to(key, "_contextId") {
        rt::log("[PearRuntime][touch] virtual digitizer deferred (no key window)");
        pear_objc::objc_release(digitizer);
        return NIL;
    }
    let context_id = msg0::<usize>(key, "_contextId") as u32;
    if context_id == 0 {
        rt::log("[PearRuntime][touch] virtual digitizer deferred (contextId 0)");
        pear_objc::objc_release(digitizer);
        return NIL;
    }
    if pear_objc::responds_to(digitizer, "setContextId:") {
        msg1::<u32, ()>(digitizer, "setContextId:", context_id);
    }
    rt::log(&format!("[PearRuntime][touch] virtual digitizer={digitizer:p} contextId=0x{context_id:x}"));
    DIGITIZER.store(digitizer as usize, Ordering::Release);
    digitizer
}

/// The digitizer's `_spinLock` (asserted held by the post).
unsafe fn digitizer_lock(digitizer: Id) -> *mut u32 {
    static OFFSET: OnceLock<Option<isize>> = OnceLock::new();
    let offset = *OFFSET.get_or_init(|| unsafe {
        let ivar = class_getInstanceVariable(rt::class_named("UINSVirtualDigitizer"), c"_spinLock".as_ptr());
        (!ivar.is_null()).then(|| ivar_getOffset(ivar))
    });
    match offset {
        Some(o) => (digitizer as *mut u8).offset(o) as *mut u32,
        None => std::ptr::null_mut(),
    }
}

/// Drives the virtual finger `key`: `down` presses it at the normalized window
/// point, `!down` releases it; then posts the collection to UIKit. Different
/// keys are independent fingers (real multi-touch).
#[export_name = "_Z27PearSynthesizeVirtualFingerbP8NSStringdd"]
pub unsafe extern "C" fn synthesize_virtual_finger(down: bool, key: Id, nx: f64, ny: f64) {
    let digitizer = virtual_digitizer();
    if digitizer.is_null() {
        return;
    }
    let window = primary_window();
    if window.is_null() {
        return;
    }
    let pts = window_size(window);
    if pts.width < 1.0 || pts.height < 1.0 {
        return;
    }
    let (x, y) = (nx * pts.width, ny * pts.height);
    if down && pear_objc::responds_to(digitizer, "addVirtualFingerAtLoc:forKey:") {
        msg2::<CGPoint, Id, ()>(digitizer, "addVirtualFingerAtLoc:forKey:", CGPoint { x, y }, key);
    }
    if pear_objc::responds_to(digitizer, "modifyVirtualFingerForKey:withBlock:") {
        let mutate = rt::block1(move |finger| unsafe {
            // Move first, so a lift reports the last position.
            if pear_objc::responds_to(finger, "setLocation:") {
                msg1::<CGPoint, ()>(finger, "setLocation:", CGPoint { x, y });
            }
            if pear_objc::responds_to(finger, "setTouching:") {
                msg1::<bool, ()>(finger, "setTouching:", down);
            }
        });
        msg2::<Id, Id, ()>(digitizer, "modifyVirtualFingerForKey:withBlock:", key, mutate);
        rt::release_block(mutate);
    }
    if pear_objc::responds_to(digitizer, "_postHIDDigitizerCollectionToUIKitWithTimestamp:") {
        let lock = digitizer_lock(digitizer);
        if !lock.is_null() {
            os_unfair_lock_lock(lock);
        }
        msg1::<u64, ()>(digitizer, "_postHIDDigitizerCollectionToUIKitWithTimestamp:", mach_absolute_time());
        if !lock.is_null() {
            os_unfair_lock_unlock(lock);
        }
    }
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!(
            "[PearRuntime][touch] virtual finger {} down={} nx={nx:.3} ny={ny:.3} -> ({x:.1},{y:.1}) of ({:.0},{:.0})",
            rt::description(key),
            down as i32,
            pts.width,
            pts.height
        ));
    }
}

// ---------------------------------------------------------------------------
// CGEvent path

/// The window's *screen* rectangle from the window server (UIKit's frame is
/// (0,0,W,H)): the largest on-screen window of this process.
unsafe fn window_screen_rect() -> CGRect {
    let mut best = CGRect::default();
    let pid = getpid() as isize;
    let info = CGWindowListCopyWindowInfo(1, 0);
    if info.is_null() {
        return best;
    }
    for i in 0..CFArrayGetCount(info) {
        let entry = CFArrayGetValueAtIndex(info, i) as Id;
        let owner: Id = msg1(entry, "objectForKey:", kCGWindowOwnerPID);
        let bounds: Id = msg1(entry, "objectForKey:", kCGWindowBounds);
        let owner_pid: isize = if owner.is_null() { 0 } else { msg0(owner, "integerValue") };
        if owner_pid != pid || bounds.is_null() {
            continue;
        }
        let num = |k: &str| -> f64 {
            let v: Id = msg1(bounds, "objectForKey:", nsstring(k));
            if v.is_null() { 0.0 } else { msg0(v, "doubleValue") }
        };
        let r = CGRect { origin: CGPoint { x: num("X"), y: num("Y") }, size: CGSize { width: num("Width"), height: num("Height") } };
        if r.size.width * r.size.height > best.size.width * best.size.height {
            best = r;
        }
    }
    CFRelease(info);
    best
}

/// Posts one mouse event at a normalized point in the game window. Where it is
/// posted matters (PEAR_POST_TARGET = hid (default) | self | annotated | session):
/// the global HID tap drops an untrusted poster's events, our own pid does not.
unsafe fn post_mouse(kind: u32, nx: f64, ny: f64) {
    let rect = window_screen_rect();
    if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
        rt::log("[PearRuntime][touch] cgevent: no window rect");
        return;
    }
    let p = CGPoint { x: rect.origin.x + nx * rect.size.width, y: rect.origin.y + ny * rect.size.height };
    let e = CGEventCreateMouseEvent(std::ptr::null(), kind, p, 0);
    let target = env("PEAR_POST_TARGET");
    if !e.is_null() {
        match target.as_deref() {
            Some("self") => CGEventPostToPid(getpid(), e),
            Some("annotated") => CGEventPost(2, e),
            Some("session") => CGEventPost(1, e),
            _ => CGEventPost(0, e),
        }
        CFRelease(e);
    }
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!(
            "[PearRuntime][touch] cgevent type={kind} at ({:.0}, {:.0}) rect={{{{{:.0},{:.0}}},{{{:.0},{:.0}}}}} target={}",
            p.x,
            p.y,
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
            target.as_deref().unwrap_or("hid")
        ));
    }
}

/// A held finger: down on press, up on release.
#[export_name = "_Z19PearSynthesizeMousebdd"]
pub unsafe extern "C" fn synthesize_mouse(down: bool, nx: f64, ny: f64) {
    post_mouse(if down { 1 } else { 2 }, nx, ny);
}

/// A finger already down moving to a new point (joystick drag).
#[export_name = "_Z18PearSynthesizeDragdd"]
pub unsafe extern "C" fn synthesize_drag(nx: f64, ny: f64) {
    post_mouse(6, nx, ny);
}

// A CGEvent tap moves the real pointer: stash/restore confine that jump to a gesture.
static STASHED: Mutex<Option<(f64, f64)>> = Mutex::new(None);

unsafe fn cursor_location() -> CGPoint {
    let e = CGEventCreate(std::ptr::null());
    if e.is_null() {
        return CGPoint::default();
    }
    let p = CGEventGetLocation(e);
    CFRelease(e);
    p
}

#[export_name = "_Z15PearStashCursorv"]
pub unsafe extern "C" fn stash_cursor() {
    let mut s = STASHED.lock().unwrap_or_else(|e| e.into_inner());
    if s.is_none() {
        let p = cursor_location();
        *s = Some((p.x, p.y));
    }
}

#[export_name = "_Z17PearUnstashCursorv"]
pub unsafe extern "C" fn unstash_cursor() {
    let mut s = STASHED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((x, y)) = s.take() {
        CGWarpMouseCursorPosition(CGPoint { x, y });
        CGAssociateMouseAndMouseCursorPosition(true);
    }
}

/// Keeps the hardware pointer inside the game window during mouse-look
/// (containment, not a pointer lock). Opt in with PEAR_MOUSE_CAPTURE=1; the
/// window rect is cached for 0.5 s (window list queries are not cheap).
#[export_name = "_Z25PearContainCursorInWindowv"]
pub unsafe extern "C" fn contain_cursor_in_window() {
    if !rt::env_set("PEAR_MOUSE_CAPTURE") {
        return;
    }
    static CACHE: Mutex<(CGRect, f64)> = Mutex::new((CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: 0.0, height: 0.0 } }, 0.0));
    let rect = {
        let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let now = CACurrentMediaTime();
        if now - c.1 > 0.5 {
            *c = (window_screen_rect(), now);
        }
        c.0
    };
    if rect.size.width < 2.0 || rect.size.height < 2.0 {
        return;
    }
    let p = cursor_location();
    let margin = 4.0;
    let inner = CGRect {
        origin: CGPoint { x: rect.origin.x + margin, y: rect.origin.y + margin },
        size: CGSize { width: rect.size.width - 2.0 * margin, height: rect.size.height - 2.0 * margin },
    };
    if inner.size.width <= 0.0 || inner.size.height <= 0.0 {
        return;
    }
    let (min_x, min_y) = (inner.origin.x, inner.origin.y);
    let (max_x, max_y) = (min_x + inner.size.width, min_y + inner.size.height);
    if p.x >= min_x && p.x < max_x && p.y >= min_y && p.y < max_y {
        return;
    }
    let clamped = CGPoint { x: p.x.max(min_x).min(max_x), y: p.y.max(min_y).min(max_y) };
    CGWarpMouseCursorPosition(clamped);
    // A game may have unassociated the pointer: re-associate so the warp sticks.
    CGAssociateMouseAndMouseCursorPosition(true);
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!(
            "[PearRuntime][capture] cursor ({:.0},{:.0}) clamped to ({:.0},{:.0}) window={{{{{:.0},{:.0}}},{{{:.0},{:.0}}}}}",
            p.x, p.y, clamped.x, clamped.y, rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
        ));
    }
}

#[export_name = "_Z24PearSynthesizeTapCGEventdd"]
pub unsafe extern "C" fn synthesize_tap_cgevent(nx: f64, ny: f64) {
    let restore = !rt::env_set("PEAR_NO_CURSOR_RESTORE");
    if restore {
        stash_cursor();
    }
    synthesize_mouse(true, nx, ny);
    usleep(40000);
    synthesize_mouse(false, nx, ny);
    if restore {
        unstash_cursor();
    }
}

// ---------------------------------------------------------------------------
// Self-test (PEAR_TEST_TAP) and delivery diagnostics (PEAR_DUMP_HIDEVENT)

static ORIGINAL_SEND_EVENT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn window_send_event(this: Id, cmd: Sel, event: Id) -> Id {
    if msg0::<isize>(event, "type") == 0 {
        // UIEventTypeTouches: log every touch that reaches the window.
        let touches: Id = msg0(msg0::<Id>(event, "allTouches"), "allObjects");
        for i in 0..pear_objc::nsarray_count(touches) {
            let touch = pear_objc::nsarray_object_at(touches, i);
            let p: CGPoint = msg1(touch, "locationInView:", this);
            let target: Id = msg0(touch, "view");
            let local: CGPoint = if target.is_null() { CGPoint::default() } else { msg1(touch, "locationInView:", target) };
            rt::log(&format!(
                "[PearRuntime][touch] DELIVERED phase={} window=({:.0},{:.0}) view={} local=({:.0},{:.0})",
                msg0::<isize>(touch, "phase"),
                p.x,
                p.y,
                if target.is_null() { "nil" } else { pear_objc::class_name(target) },
                local.x,
                local.y
            ));
        }
        // Dump the underlying HID event to compare with a synthesized one.
        let h = hid();
        if rt::env_set("PEAR_DUMP_HIDEVENT") && h.get_integer.is_some() && h.get_type.is_some() {
            if pear_objc::responds_to(event, "_hidEvent") {
                let real: Ev = msg0(event, "_hidEvent");
                if !real.is_null() {
                    rt::log("[PearRuntime][hid] ---- real delivered ----");
                    dump_hid_tree(real, 0);
                    // Replay the event UIKit just accepted: tells a sink problem
                    // from a construction problem.
                    if let (true, Some(copy_fn)) = (rt::env_set("PEAR_REPLAY"), h.create_copy) {
                        let copy = copy_fn(std::ptr::null(), real);
                        if !copy.is_null() {
                            let app: Id = msg0(cls("UIApplication"), "sharedApplication");
                            if pear_objc::responds_to(app, "_handleHIDEvent:") {
                                rt::log("[PearRuntime][hid] replaying captured event via _handleHIDEvent:");
                                msg1::<Id, ()>(app, "_handleHIDEvent:", copy as Id);
                            }
                            CFRelease(copy);
                        }
                    }
                } else {
                    rt::log("[PearRuntime][hid] real event but _hidEvent was NULL");
                }
            } else {
                let class = rt::object_getClass(event);
                rt::log(&format!(
                    "[PearRuntime][hid] event {} has no _hidEvent (classes: {})",
                    pear_objc::class_name(event),
                    if pear_objc::responds_to(class as Id, "_hidEvent") { "class-level" } else { "none" }
                ));
            }
        }
    }
    let original: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(ORIGINAL_SEND_EVENT.load(Ordering::Acquire));
    original(this, cmd, event)
}

fn test_tap() {
    unsafe {
        // Virtual digitizer is the default; CGEvent and IOHIDEvent are opt-in.
        if rt::env_set("PEAR_TOUCH_CGEVENT") {
            synthesize_tap_cgevent(0.5, 0.5);
        } else if rt::env_set("PEAR_TOUCH_IOHID") {
            synthesize_touch(true, 0.5, 0.5);
            rt::after_main(0.08, || unsafe { synthesize_touch(false, 0.5, 0.5) });
        } else {
            synthesize_virtual_finger(true, nsstring("pear-test"), 0.5, 0.5);
            rt::after_main(0.10, || unsafe { synthesize_virtual_finger(false, nsstring("pear-test"), 0.5, 0.5) });
        }
    }
}

pub fn init() {
    if !rt::module_enabled("input") {
        return;
    }
    let test = rt::env_set("PEAR_TEST_TAP");
    if !test && !rt::env_set("PEAR_DUMP_HIDEVENT") {
        return;
    }
    unsafe {
        pear_objc::autorelease_pool(|| {
            // The CGEvent path never resolves IOHID; the dump below needs it.
            hid();
            // Synthetic events are gated on the poster's Accessibility trust.
            let services = rt::dlopen_path("/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices", rt::RTLD_LAZY);
            let trusted_fn = if services.is_null() { std::ptr::null_mut() } else { dlsym(services, c"AXIsProcessTrusted".as_ptr()) };
            let trusted = !trusted_fn.is_null() && std::mem::transmute::<_, unsafe extern "C" fn() -> u8>(trusted_fn)() != 0;
            rt::log(&format!(
                "[PearRuntime][touch] AXIsProcessTrusted={} pid={}",
                if trusted_fn.is_null() { -1 } else { trusted as i32 },
                getpid()
            ));
            if !trusted && rt::env_set("PEAR_AX_PROMPT") && !services.is_null() {
                let with_options = dlsym(services, c"AXIsProcessTrustedWithOptions".as_ptr());
                if !with_options.is_null() {
                    let options: Id = msg2(cls("NSDictionary"), "dictionaryWithObject:forKey:", pear_objc::nsnumber_bool(true), nsstring("AXTrustedCheckOptionPrompt"));
                    std::mem::transmute::<_, unsafe extern "C" fn(Id) -> u8>(with_options)(options);
                    rt::log("[PearRuntime][touch] requested Accessibility prompt");
                }
            }
            // Is the post itself dropped (untrusted), or delivered elsewhere?
            if rt::env_set("PEAR_MOVE_PROBE") {
                let target = CGPoint { x: 200.0, y: 200.0 };
                let mv = CGEventCreateMouseEvent(std::ptr::null(), 5, target, 0);
                if !mv.is_null() {
                    CGEventPost(0, mv);
                    CFRelease(mv);
                }
                usleep(200000);
                let got = cursor_location();
                rt::log(&format!(
                    "[PearRuntime][touch] move probe: target=(200,200) cursor=({:.0},{:.0}) accepted={}",
                    got.x,
                    got.y,
                    (got.x == target.x && got.y == target.y) as i32
                ));
            }
            // Verify delivery: log any touch that reaches a window.
            let m = rt::class_getInstanceMethod(cls("UIWindow"), sel("sendEvent:"));
            if !m.is_null() {
                ORIGINAL_SEND_EVENT.store(rt::method_getImplementation(m) as usize, Ordering::Release);
                rt::method_setImplementation(m, window_send_event as Imp);
            }
        });
    }
    if !test {
        return;
    }
    if rt::env_set("PEAR_TEST_MULTITOUCH") {
        rt::after_main(8.0, || unsafe {
            rt::log("[PearRuntime][touch] TEST multi-touch: two fingers");
            synthesize_virtual_finger(true, nsstring("mt-a"), 0.30, 0.30);
            synthesize_virtual_finger(true, nsstring("mt-b"), 0.70, 0.70);
            rt::after_main(1.0, || unsafe {
                synthesize_virtual_finger(false, nsstring("mt-a"), 0.30, 0.30);
                synthesize_virtual_finger(false, nsstring("mt-b"), 0.70, 0.70);
            });
        });
        return;
    }
    rt::after_main(8.0, || {
        rt::log("[PearRuntime][touch] TEST tapping centre");
        // Repeated taps tell "never delivered" from "occasionally".
        for i in 0..6 {
            rt::after_main(i as f64 * 1.5, test_tap);
        }
    });
}

