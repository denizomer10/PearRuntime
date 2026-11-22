//! UITraitCollection, UIScreen, UIDevice, UIResponder, UITouch, UIEvent,
//! UIPasteboard, UIFeedbackGenerator (+ subclasses), and UIStoryboard.

use pear_objc::{
    cls, is_kind_of, msg0, msg1, msg2, msg3, nsarray_count, nsarray_from_ids,
    nsarray_object_at, nsdata_from_bytes, nsdata_to_vec, nsnumber_f64, nsstring, nsstring_to_string,
    objc_getAssociatedObject, objc_release, objc_retain, objc_setAssociatedObject,
    replace_method as add_method, sel, CGPoint, CGRect, CGSize, Class, Id, Imp, Sel, UIEdgeInsets,
    NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct PearHostScreenInfo {
    pub point_size: CGSize,
    pub scale: f64,
    pub max_fps: isize,
}

extern "C" {
    fn PearMissingReport(kind: c_int, class_name: *const c_char, selector: *const c_char, caller: *const c_void);
    fn sysctlbyname(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *const c_void,
        newlen: usize,
    ) -> c_int;
    fn objc_loadWeak(location: *mut Id) -> Id;
    fn objc_storeWeak(location: *mut Id, obj: Id) -> Id;
    fn object_getClass(obj: Id) -> Class;
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

fn content_size_rank(cat: &str) -> Option<usize> {
    match cat {
        "UIContentSizeCategoryExtraSmall" | "UICTContentSizeCategoryXS" => Some(0),
        "UIContentSizeCategorySmall" | "UICTContentSizeCategoryS" => Some(1),
        "UIContentSizeCategoryMedium" | "UICTContentSizeCategoryM" => Some(2),
        "UIContentSizeCategoryLarge" | "UICTContentSizeCategoryL" => Some(3),
        "UIContentSizeCategoryExtraLarge" | "UICTContentSizeCategoryXL" => Some(4),
        "UIContentSizeCategoryExtraExtraLarge" | "UICTContentSizeCategoryXXL" => Some(5),
        "UIContentSizeCategoryExtraExtraExtraLarge" | "UICTContentSizeCategoryXXXL" => Some(6),
        "UIContentSizeCategoryAccessibilityMedium" | "UICTContentSizeCategoryAccessibilityM" => Some(7),
        "UIContentSizeCategoryAccessibilityLarge" | "UICTContentSizeCategoryAccessibilityL" => Some(8),
        "UIContentSizeCategoryAccessibilityExtraLarge" | "UICTContentSizeCategoryAccessibilityXL" => Some(9),
        "UIContentSizeCategoryAccessibilityExtraExtraLarge" | "UICTContentSizeCategoryAccessibilityXXL" => Some(10),
        "UIContentSizeCategoryAccessibilityExtraExtraExtraLarge" | "UICTContentSizeCategoryAccessibilityXXXL" => Some(11),
        _ => None,
    }
}

#[no_mangle]
pub unsafe extern "C" fn UIContentSizeCategoryCompareToCategory(lhs: Id, rhs: Id) -> isize {
    let l = nsstring_to_string(lhs).unwrap_or_default();
    let r = nsstring_to_string(rhs).unwrap_or_default();
    match (content_size_rank(&l), content_size_rank(&r)) {
        (Some(a), Some(b)) => match a.cmp(&b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        },
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn UIContentSizeCategoryIsAccessibilityCategory(cat: Id) -> bool {
    nsstring_to_string(cat)
        .map(|s| s.contains("Accessibility"))
        .unwrap_or(false)
}

// ============================================================================
// UITraitCollection
// ============================================================================

unsafe extern "C" fn trait_current(self_cls: Id, _cmd: Sel) -> Id {
    msg0(self_cls, "new")
}
unsafe extern "C" fn trait_idiom(_self: Id, _cmd: Sel) -> isize {
    let dev: Id = msg0(cls("UIDevice"), "currentDevice");
    msg0(dev, "userInterfaceIdiom")
}
unsafe extern "C" fn trait_scale(_self: Id, _cmd: Sel) -> f64 {
    let scr: Id = msg0(cls("UIScreen"), "mainScreen");
    msg0(scr, "scale")
}
unsafe extern "C" fn trait_style(_self: Id, _cmd: Sel) -> isize {
    2 // dark
}
unsafe extern "C" fn trait_size_class(_self: Id, _cmd: Sel) -> isize {
    2 // regular
}
unsafe extern "C" fn trait_force_touch(_self: Id, _cmd: Sel) -> isize {
    1 // unavailable
}
unsafe extern "C" fn trait_gamut(_self: Id, _cmd: Sel) -> isize {
    1 // P3
}
unsafe extern "C" fn copy_self(self_obj: Id, _cmd: Sel, _zone: *mut c_void) -> Id {
    objc_retain(self_obj)
}

// ============================================================================
// UIScreen
// ============================================================================

static MAIN_SCREEN: AtomicUsize = AtomicUsize::new(0);
static SCREEN_STATE_KEY: u8 = 0;

#[repr(C)]
#[derive(Copy, Clone)]
struct ScreenState {
    size: CGSize,
    scale: f64,
    brightness: f64,
}

/// The host's screen, in the shape UIScreen keeps.
fn screen_info() -> PearHostScreenInfo {
    let s = pear_host::host().screen();
    PearHostScreenInfo {
        point_size: CGSize { width: s.points.width, height: s.points.height },
        scale: s.scale,
        max_fps: s.max_fps as isize,
    }
}

unsafe fn get_screen_state(scr: Id) -> ScreenState {
    let data = assoc_get(scr, &SCREEN_STATE_KEY);
    if let Some(v) = nsdata_to_vec(data) {
        if v.len() == std::mem::size_of::<ScreenState>() {
            return std::ptr::read_unaligned(v.as_ptr() as *const ScreenState);
        }
    }
    let info = screen_info();
    ScreenState {
        size: info.point_size,
        scale: if info.scale > 0.0 { info.scale } else { 1.0 },
        brightness: 1.0,
    }
}

unsafe fn set_screen_state(scr: Id, st: ScreenState) {
    let bytes = std::slice::from_raw_parts(&st as *const ScreenState as *const u8, std::mem::size_of::<ScreenState>());
    assoc_set(scr, &SCREEN_STATE_KEY, nsdata_from_bytes(bytes));
}

unsafe extern "C" fn screen_main(self_cls: Id, _cmd: Sel) -> Id {
    let cached = MAIN_SCREEN.load(Ordering::Acquire);
    if cached != 0 {
        return cached as Id;
    }
    let scr: Id = msg0(self_cls, "new");
    let info = screen_info();
    set_screen_state(
        scr,
        ScreenState {
            size: info.point_size,
            scale: if info.scale > 0.0 { info.scale } else { 1.0 },
            brightness: 1.0,
        },
    );
    objc_retain(scr);
    MAIN_SCREEN.store(scr as usize, Ordering::Release);
    scr
}

unsafe extern "C" fn screen_screens(self_cls: Id, _cmd: Sel) -> Id {
    let m = screen_main(self_cls, sel("mainScreen"));
    nsarray_from_ids(&[m])
}

unsafe extern "C" fn screen_bounds(self_obj: Id, _cmd: Sel) -> CGRect {
    let st = get_screen_state(self_obj);
    CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: st.size,
    }
}

unsafe extern "C" fn screen_native_bounds(self_obj: Id, _cmd: Sel) -> CGRect {
    let st = get_screen_state(self_obj);
    let w = st.size.width * st.scale;
    let h = st.size.height * st.scale;
    CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: w.min(h),
            height: w.max(h),
        },
    }
}

unsafe extern "C" fn screen_scale(self_obj: Id, _cmd: Sel) -> f64 {
    get_screen_state(self_obj).scale
}

unsafe extern "C" fn screen_max_fps(_self: Id, _cmd: Sel) -> isize {
    screen_info().max_fps
}

unsafe extern "C" fn screen_brightness(self_obj: Id, _cmd: Sel) -> f64 {
    get_screen_state(self_obj).brightness
}

unsafe extern "C" fn screen_set_brightness(self_obj: Id, _cmd: Sel, b: f64) {
    let mut st = get_screen_state(self_obj);
    st.brightness = b;
    set_screen_state(self_obj, st);
}

unsafe extern "C" fn screen_trait_collection(_self: Id, _cmd: Sel) -> Id {
    msg0(cls("UITraitCollection"), "pear_current")
}

unsafe extern "C" fn screen_coord_space(self_obj: Id, _cmd: Sel) -> Id {
    self_obj
}

unsafe extern "C" fn screen_nil(_self: Id, _cmd: Sel) -> Id {
    NIL
}

unsafe extern "C" fn screen_false(_self: Id, _cmd: Sel) -> bool {
    false
}

unsafe extern "C" fn screen_overscan(_self: Id, _cmd: Sel) -> UIEdgeInsets {
    UIEdgeInsets::default()
}

unsafe extern "C" fn screen_display_link(_self: Id, _cmd: Sel, target: Id, selector: Sel) -> Id {
    pear_host::host().display_link(pear_host::GuestObject(target), pear_host::GuestSelector(selector)).0
}

unsafe extern "C" fn screen_pear_update(self_obj: Id, _cmd: Sel, size: CGSize, scale: f64) {
    let mut st = get_screen_state(self_obj);
    st.size = size;
    st.scale = scale;
    set_screen_state(self_obj, st);
}

#[no_mangle]
pub extern "C" fn PearUIKitScreenDidChange(point_size: CGSize, scale: f64) {
    unsafe {
        let scr = msg0::<Id>(cls("UIScreen"), "mainScreen");
        msg2::<CGSize, f64, ()>(scr, "pear_update:scale:", point_size, scale);
    }
}

// ============================================================================
// UIDevice
// ============================================================================

static CURRENT_DEVICE: AtomicUsize = AtomicUsize::new(0);
static DEV_BATT_KEY: u8 = 0;
static DEV_UUID_KEY: u8 = 0;

unsafe extern "C" fn device_current(self_cls: Id, _cmd: Sel) -> Id {
    let cached = CURRENT_DEVICE.load(Ordering::Acquire);
    if cached != 0 {
        return cached as Id;
    }
    let dev: Id = msg0(self_cls, "new");
    objc_retain(dev);
    CURRENT_DEVICE.store(dev as usize, Ordering::Release);
    dev
}

unsafe extern "C" fn device_ipad(_self: Id, _cmd: Sel) -> Id {
    nsstring("iPad")
}
unsafe extern "C" fn device_ipados(_self: Id, _cmd: Sel) -> Id {
    nsstring("iPadOS")
}
unsafe extern "C" fn device_sys_ver(_self: Id, _cmd: Sel) -> Id {
    if let Ok(v) = std::env::var("PEAR_HOST_IOS_VERSION") {
        nsstring(&v)
    } else {
        nsstring("17.0")
    }
}
unsafe extern "C" fn device_orientation(_self: Id, _cmd: Sel) -> isize {
    3 // landscapeLeft
}
unsafe extern "C" fn device_idiom(_self: Id, _cmd: Sel) -> isize {
    1 // pad
}
unsafe extern "C" fn device_false(_self: Id, _cmd: Sel) -> bool {
    false
}
unsafe extern "C" fn device_true(_self: Id, _cmd: Sel) -> bool {
    true
}
unsafe extern "C" fn device_noop(_self: Id, _cmd: Sel) {}
unsafe extern "C" fn device_noop_bool(_self: Id, _cmd: Sel, _b: bool) {}
unsafe extern "C" fn device_zero_int(_self: Id, _cmd: Sel) -> isize {
    0
}

unsafe extern "C" fn device_vendor_id(self_obj: Id, _cmd: Sel) -> Id {
    let existing = assoc_get(self_obj, &DEV_UUID_KEY);
    if !existing.is_null() {
        return existing;
    }
    let mut buf = [0u8; 256];
    let mut len = buf.len();
    let cname = CString::new("kern.uuid").unwrap();
    let hw = if sysctlbyname(cname.as_ptr(), buf.as_mut_ptr() as *mut c_void, &mut len, std::ptr::null(), 0) == 0 {
        CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy().into_owned()
    } else {
        String::new()
    };
    let main_bundle: Id = msg0(cls("NSBundle"), "mainBundle");
    let bid_id: Id = msg0(main_bundle, "bundleIdentifier");
    let bid = nsstring_to_string(bid_id).unwrap_or_default();
    let seed = format!("{hw}|{bid}");
    let mut bytes = [0u8; 16];
    for (i, b) in seed.as_bytes().iter().enumerate() {
        bytes[i % 16] = bytes[i % 16].wrapping_mul(31).wrapping_add(*b);
    }
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    let uuid: Id = msg1(msg0(cls("NSUUID"), "alloc"), "initWithUUIDBytes:", bytes.as_ptr());
    assoc_set(self_obj, &DEV_UUID_KEY, uuid);
    objc_release(uuid);
    assoc_get(self_obj, &DEV_UUID_KEY)
}

unsafe extern "C" fn device_batt_enabled(self_obj: Id, _cmd: Sel) -> bool {
    let v = assoc_get(self_obj, &DEV_BATT_KEY);
    !v.is_null() && msg0::<bool>(v, "boolValue")
}
unsafe extern "C" fn device_set_batt_enabled(self_obj: Id, _cmd: Sel, enabled: bool) {
    assoc_set(self_obj, &DEV_BATT_KEY, pear_objc::nsnumber_bool(enabled));
}
unsafe extern "C" fn device_batt_state(self_obj: Id, _cmd: Sel) -> isize {
    if device_batt_enabled(self_obj, sel("isBatteryMonitoringEnabled")) {
        3 // full
    } else {
        0 // unknown
    }
}
unsafe extern "C" fn device_batt_level(self_obj: Id, _cmd: Sel) -> f32 {
    if device_batt_enabled(self_obj, sel("isBatteryMonitoringEnabled")) {
        1.0
    } else {
        -1.0
    }
}

// ============================================================================
// UIResponder
// ============================================================================

static FIRST_RESPONDER_WEAK: Mutex<usize> = Mutex::new(0);

#[no_mangle]
pub extern "C" fn PearUIKitFirstResponder() -> Id {
    let mut g = FIRST_RESPONDER_WEAK.lock().unwrap();
    let ptr = &mut *g as *mut usize as *mut Id;
    unsafe { objc_loadWeak(ptr) }
}

unsafe fn set_first_responder(obj: Id) {
    let mut g = FIRST_RESPONDER_WEAK.lock().unwrap();
    let ptr = &mut *g as *mut usize as *mut Id;
    objc_storeWeak(ptr, obj);
}

unsafe extern "C" fn resp_nil(_self: Id, _cmd: Sel) -> Id {
    NIL
}
unsafe extern "C" fn resp_false(_self: Id, _cmd: Sel) -> bool {
    false
}
unsafe extern "C" fn resp_true(_self: Id, _cmd: Sel) -> bool {
    true
}
unsafe extern "C" fn resp_is_first(self_obj: Id, _cmd: Sel) -> bool {
    PearUIKitFirstResponder() == self_obj
}
unsafe extern "C" fn resp_become_first(self_obj: Id, _cmd: Sel) -> bool {
    let cur = PearUIKitFirstResponder();
    if cur == self_obj {
        return true;
    }
    if !msg0::<bool>(self_obj, "canBecomeFirstResponder") {
        return false;
    }
    if !cur.is_null() && !msg0::<bool>(cur, "resignFirstResponder") {
        return false;
    }
    set_first_responder(self_obj);
    true
}
unsafe extern "C" fn resp_resign_first(self_obj: Id, _cmd: Sel) -> bool {
    if PearUIKitFirstResponder() == self_obj {
        set_first_responder(NIL);
    }
    true
}

unsafe extern "C" fn resp_forward_2(self_obj: Id, cmd: Sel, a1: Id, a2: Id) {
    let next: Id = msg0(self_obj, "nextResponder");
    if !next.is_null() {
        let f: unsafe extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
        f(next, cmd, a1, a2);
    }
}

unsafe extern "C" fn resp_noop_int_id(_self: Id, _cmd: Sel, _m: isize, _e: Id) {}
unsafe extern "C" fn resp_noop_id(_self: Id, _cmd: Sel, _e: Id) {}
unsafe extern "C" fn resp_noop(_self: Id, _cmd: Sel) {}
unsafe extern "C" fn resp_can_perform(self_obj: Id, _cmd: Sel, action: Sel, _sender: Id) -> bool {
    let r: c_int = msg1(self_obj, "respondsToSelector:", action);
    r != 0
}

// ============================================================================
// UITouch / UIEvent / PearUIKitDeliverPointer
// ============================================================================

static TOUCH_STATE_KEY: u8 = 0;
static TOUCH_VIEW_KEY: u8 = 0;
static TOUCH_WINDOW_KEY: u8 = 0;

#[repr(C)]
#[derive(Copy, Clone)]
struct TouchState {
    location: CGPoint,
    previous: CGPoint,
    timestamp: f64,
    phase: isize,
}

unsafe fn get_touch_state(t: Id) -> TouchState {
    let data = assoc_get(t, &TOUCH_STATE_KEY);
    if let Some(v) = nsdata_to_vec(data) {
        if v.len() == std::mem::size_of::<TouchState>() {
            return std::ptr::read_unaligned(v.as_ptr() as *const TouchState);
        }
    }
    TouchState {
        location: CGPoint::default(),
        previous: CGPoint::default(),
        timestamp: 0.0,
        phase: 0,
    }
}

unsafe fn set_touch_state(t: Id, st: TouchState) {
    let bytes = std::slice::from_raw_parts(&st as *const TouchState as *const u8, std::mem::size_of::<TouchState>());
    assoc_set(t, &TOUCH_STATE_KEY, nsdata_from_bytes(bytes));
}

unsafe extern "C" fn touch_init(
    self_obj: Id,
    _cmd: Sel,
    view: Id,
    window: Id,
    location: CGPoint,
    timestamp: f64,
) -> Id {
    let obj: Id = msg0(self_obj, "init");
    if !obj.is_null() {
        assoc_set(obj, &TOUCH_VIEW_KEY, view);
        assoc_set(obj, &TOUCH_WINDOW_KEY, window);
        set_touch_state(
            obj,
            TouchState {
                location,
                previous: location,
                timestamp,
                phase: 0, // began
            },
        );
    }
    obj
}

unsafe extern "C" fn touch_pear_move(self_obj: Id, _cmd: Sel, pt: CGPoint, phase: isize, timestamp: f64) {
    let mut st = get_touch_state(self_obj);
    st.previous = st.location;
    st.location = pt;
    st.phase = phase;
    st.timestamp = timestamp;
    set_touch_state(self_obj, st);
}

unsafe extern "C" fn touch_timestamp(self_obj: Id, _cmd: Sel) -> f64 {
    get_touch_state(self_obj).timestamp
}
unsafe extern "C" fn touch_phase(self_obj: Id, _cmd: Sel) -> isize {
    get_touch_state(self_obj).phase
}
unsafe extern "C" fn touch_tap_count(_self: Id, _cmd: Sel) -> usize {
    1
}
unsafe extern "C" fn touch_type(_self: Id, _cmd: Sel) -> isize {
    0 // direct
}
unsafe extern "C" fn touch_force(self_obj: Id, _cmd: Sel) -> f64 {
    if get_touch_state(self_obj).phase == 3 {
        0.0
    } else {
        1.0
    }
}
unsafe extern "C" fn touch_one(_self: Id, _cmd: Sel) -> f64 {
    1.0
}
unsafe extern "C" fn touch_radius(_self: Id, _cmd: Sel) -> f64 {
    20.0
}
unsafe extern "C" fn touch_radius_tol(_self: Id, _cmd: Sel) -> f64 {
    5.0
}
unsafe extern "C" fn touch_view(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &TOUCH_VIEW_KEY)
}
unsafe extern "C" fn touch_window(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &TOUCH_WINDOW_KEY)
}
unsafe extern "C" fn touch_empty_arr(_self: Id, _cmd: Sel) -> Id {
    msg0(cls("NSArray"), "array")
}
unsafe extern "C" fn touch_location_in_view(self_obj: Id, _cmd: Sel, view: Id) -> CGPoint {
    let st = get_touch_state(self_obj);
    if !view.is_null() {
        let win = assoc_get(self_obj, &TOUCH_WINDOW_KEY);
        msg2(view, "convertPoint:fromView:", st.location, win)
    } else {
        st.location
    }
}
unsafe extern "C" fn touch_prev_location_in_view(self_obj: Id, _cmd: Sel, view: Id) -> CGPoint {
    let st = get_touch_state(self_obj);
    if !view.is_null() {
        let win = assoc_get(self_obj, &TOUCH_WINDOW_KEY);
        msg2(view, "convertPoint:fromView:", st.previous, win)
    } else {
        st.previous
    }
}

static EVENT_TOUCHES_KEY: u8 = 0;
static EVENT_TS_KEY: u8 = 0;

unsafe extern "C" fn event_init(self_obj: Id, _cmd: Sel, touches: Id, timestamp: f64) -> Id {
    let obj: Id = msg0(self_obj, "init");
    if !obj.is_null() {
        assoc_set(obj, &EVENT_TOUCHES_KEY, touches);
        assoc_set(obj, &EVENT_TS_KEY, nsnumber_f64(timestamp));
    }
    obj
}
unsafe extern "C" fn event_zero(_self: Id, _cmd: Sel) -> isize {
    0
}
unsafe extern "C" fn event_timestamp(self_obj: Id, _cmd: Sel) -> f64 {
    let n = assoc_get(self_obj, &EVENT_TS_KEY);
    if n.is_null() {
        0.0
    } else {
        msg0(n, "doubleValue")
    }
}
unsafe extern "C" fn event_all_touches(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &EVENT_TOUCHES_KEY)
}
unsafe extern "C" fn event_touches_for_view(self_obj: Id, _cmd: Sel, view: Id) -> Id {
    let set = assoc_get(self_obj, &EVENT_TOUCHES_KEY);
    let out: Id = msg0(cls("NSMutableSet"), "set");
    let arr: Id = msg0(set, "allObjects");
    let n = nsarray_count(arr);
    for i in 0..n {
        let t = nsarray_object_at(arr, i);
        if msg0::<Id>(t, "view") == view {
            msg1::<Id, ()>(out, "addObject:", t);
        }
    }
    out
}
unsafe extern "C" fn event_touches_for_window(self_obj: Id, _cmd: Sel, window: Id) -> Id {
    let set = assoc_get(self_obj, &EVENT_TOUCHES_KEY);
    let out: Id = msg0(cls("NSMutableSet"), "set");
    let arr: Id = msg0(set, "allObjects");
    let n = nsarray_count(arr);
    for i in 0..n {
        let t = nsarray_object_at(arr, i);
        if msg0::<Id>(t, "window") == window {
            msg1::<Id, ()>(out, "addObject:", t);
        }
    }
    out
}
unsafe extern "C" fn event_coalesced(_self: Id, _cmd: Sel, touch: Id) -> Id {
    if touch.is_null() {
        NIL
    } else {
        nsarray_from_ids(&[touch])
    }
}
unsafe extern "C" fn event_predicted(_self: Id, _cmd: Sel, _touch: Id) -> Id {
    msg0(cls("NSArray"), "array")
}

static ACTIVE_TOUCH: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub extern "C" fn PearUIKitDeliverPointer(window: Id, phase: isize, point: CGPoint, timestamp: f64) {
    unsafe {
        if window.is_null() {
            return;
        }
        let mut touch = ACTIVE_TOUCH.load(Ordering::Acquire) as Id;
        if phase == 0 {
            if !touch.is_null() {
                objc_release(touch);
            }
            let hit: Id = msg2(window, "hitTest:withEvent:", point, NIL);
            let target = if hit.is_null() { window } else { hit };
            let alloc: Id = msg0(cls("UITouch"), "alloc");
            let f: unsafe extern "C" fn(Id, Sel, Id, Id, CGPoint, f64) -> Id =
                std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
            touch = f(alloc, sel("initWithView:window:location:timestamp:"), target, window, point, timestamp);
            ACTIVE_TOUCH.store(touch as usize, Ordering::Release);
        } else if !touch.is_null() {
            msg3::<CGPoint, isize, f64, ()>(touch, "pear_moveTo:phase:timestamp:", point, phase, timestamp);
        } else {
            return;
        }

        if touch.is_null() {
            return;
        }
        let set: Id = msg1(cls("NSSet"), "setWithObject:", touch);
        let ev_alloc: Id = msg0(cls("UIEvent"), "alloc");
        let ev: Id = msg2(ev_alloc, "initWithTouches:timestamp:", set, timestamp);
        let target_view: Id = msg0(touch, "view");
        if !target_view.is_null() {
            match phase {
                0 => msg2::<Id, Id, ()>(target_view, "touchesBegan:withEvent:", set, ev),
                1 => msg2::<Id, Id, ()>(target_view, "touchesMoved:withEvent:", set, ev),
                3 => msg2::<Id, Id, ()>(target_view, "touchesEnded:withEvent:", set, ev),
                4 => msg2::<Id, Id, ()>(target_view, "touchesCancelled:withEvent:", set, ev),
                _ => {}
            }
        }
        objc_release(ev);
        if phase == 3 || phase == 4 {
            let old = ACTIVE_TOUCH.swap(0, Ordering::AcqRel) as Id;
            if !old.is_null() {
                objc_release(old);
            }
        }
    }
}

// ============================================================================
// UIPasteboard
// ============================================================================

static PB_NAME_KEY: u8 = 0;
static GENERAL_PB: AtomicUsize = AtomicUsize::new(0);
/// UIPasteboardNameGeneral.
const GENERAL_PASTEBOARD_NAME: &str = "com.apple.UIKit.pboard.general";

/// The host pasteboard behind a UIPasteboard: None for the general one.
unsafe fn pb_host_name(obj: Id) -> Option<String> {
    nsstring_to_string(assoc_get(obj, &PB_NAME_KEY))
}

unsafe fn pb_read(obj: Id) -> Vec<pear_host::PasteboardItem> {
    pear_host::host().pasteboard_read(pb_host_name(obj).as_deref())
}

unsafe fn pb_write(obj: Id, items: &[pear_host::PasteboardItem]) {
    pear_host::host().pasteboard_write(pb_host_name(obj).as_deref(), items);
}

unsafe fn pb_has_type(obj: Id, utis: &[&str]) -> bool {
    pear_host::host().pasteboard_types(pb_host_name(obj).as_deref()).iter().any(|t| utis.contains(&t.as_str()))
}

unsafe fn ns_url(s: &str) -> Id {
    msg1(cls("NSURL"), "URLWithString:", nsstring(s))
}

/// NSString and NSURL objects of `array` as pasteboard items.
unsafe fn pb_items_from(array: Id) -> Vec<pear_host::PasteboardItem> {
    let mut items = Vec::new();
    for i in 0..nsarray_count(array) {
        let o = nsarray_object_at(array, i);
        if is_kind_of(o, "NSString") {
            items.push(pear_host::PasteboardItem::Text(nsstring_to_string(o).unwrap_or_default()));
        } else if is_kind_of(o, "NSURL") {
            items.push(pear_host::PasteboardItem::Url(nsstring_to_string(msg0(o, "absoluteString")).unwrap_or_default()));
        }
    }
    items
}

unsafe extern "C" fn pb_general(self_cls: Id, _cmd: Sel) -> Id {
    let cached = GENERAL_PB.load(Ordering::Acquire);
    if cached != 0 {
        return cached as Id;
    }
    let pb: Id = msg0(self_cls, "new");
    objc_retain(pb);
    GENERAL_PB.store(pb as usize, Ordering::Release);
    pb
}

unsafe extern "C" fn pb_with_name(self_cls: Id, _cmd: Sel, name: Id, _create: bool) -> Id {
    let pb: Id = msg0(self_cls, "new");
    if nsstring_to_string(name).as_deref() != Some(GENERAL_PASTEBOARD_NAME) {
        assoc_set(pb, &PB_NAME_KEY, name);
    }
    pb
}

unsafe extern "C" fn pb_name(self_obj: Id, _cmd: Sel) -> Id {
    nsstring(&pb_host_name(self_obj).unwrap_or_else(|| GENERAL_PASTEBOARD_NAME.to_string()))
}
unsafe extern "C" fn pb_change_count(self_obj: Id, _cmd: Sel) -> isize {
    pear_host::host().pasteboard_change_count(pb_host_name(self_obj).as_deref())
}
unsafe extern "C" fn pb_string(self_obj: Id, _cmd: Sel) -> Id {
    pb_read(self_obj)
        .into_iter()
        .find_map(|i| if let pear_host::PasteboardItem::Text(s) = i { Some(nsstring(&s)) } else { None })
        .unwrap_or(NIL)
}
unsafe extern "C" fn pb_set_string(self_obj: Id, _cmd: Sel, val: Id) {
    let items = nsstring_to_string(val).map(pear_host::PasteboardItem::Text).into_iter().collect::<Vec<_>>();
    pb_write(self_obj, &items);
}
unsafe extern "C" fn pb_strings(self_obj: Id, _cmd: Sel) -> Id {
    let out: Vec<Id> = pb_read(self_obj)
        .into_iter()
        .filter_map(|i| if let pear_host::PasteboardItem::Text(s) = i { Some(nsstring(&s)) } else { None })
        .collect();
    if out.is_empty() { NIL } else { nsarray_from_ids(&out) }
}
unsafe extern "C" fn pb_set_strings(self_obj: Id, _cmd: Sel, val: Id) {
    let items = if val.is_null() { Vec::new() } else { pb_items_from(val) };
    pb_write(self_obj, &items);
}
unsafe extern "C" fn pb_has_strings(self_obj: Id, _cmd: Sel) -> bool {
    pb_has_type(self_obj, &[pear_host::UTI_TEXT])
}
unsafe extern "C" fn pb_url(self_obj: Id, _cmd: Sel) -> Id {
    pb_read(self_obj)
        .into_iter()
        .find_map(|i| if let pear_host::PasteboardItem::Url(u) = i { Some(ns_url(&u)) } else { None })
        .unwrap_or(NIL)
}
unsafe extern "C" fn pb_set_url(self_obj: Id, _cmd: Sel, val: Id) {
    let items = if val.is_null() { Vec::new() } else { pb_items_from(nsarray_from_ids(&[val])) };
    pb_write(self_obj, &items);
}
unsafe extern "C" fn pb_urls(self_obj: Id, _cmd: Sel) -> Id {
    let out: Vec<Id> = pb_read(self_obj)
        .into_iter()
        .filter_map(|i| if let pear_host::PasteboardItem::Url(u) = i { Some(ns_url(&u)) } else { None })
        .filter(|u| !u.is_null())
        .collect();
    nsarray_from_ids(&out)
}
unsafe extern "C" fn pb_set_urls(self_obj: Id, _cmd: Sel, val: Id) {
    let items = if val.is_null() { Vec::new() } else { pb_items_from(val) };
    pb_write(self_obj, &items);
}
unsafe extern "C" fn pb_has_urls(self_obj: Id, _cmd: Sel) -> bool {
    pb_has_type(self_obj, &[pear_host::UTI_URL, pear_host::UTI_FILE_URL])
}
unsafe extern "C" fn pb_has_images(self_obj: Id, _cmd: Sel) -> bool {
    pb_has_type(self_obj, &["public.png", "public.jpeg", "public.tiff", "public.heic", "public.image"])
}
unsafe extern "C" fn pb_data_for_type(self_obj: Id, _cmd: Sel, ty: Id) -> Id {
    let uti = nsstring_to_string(ty).unwrap_or_default();
    match pear_host::host().pasteboard_data(pb_host_name(self_obj).as_deref(), &uti) {
        Some(bytes) => nsdata_from_bytes(&bytes),
        None => NIL,
    }
}
unsafe extern "C" fn pb_set_data_for_type(self_obj: Id, _cmd: Sel, data: Id, ty: Id) {
    let uti = nsstring_to_string(ty).unwrap_or_default();
    let items = match nsdata_to_vec(data) {
        Some(bytes) => vec![pear_host::PasteboardItem::Data { uti, bytes }],
        None => Vec::new(),
    };
    pb_write(self_obj, &items);
}
unsafe extern "C" fn pb_value_for_type(self_obj: Id, _cmd: Sel, ty: Id) -> Id {
    let s = nsstring_to_string(ty).unwrap_or_default();
    if s == "public.utf8-plain-text" || s == "public.text" {
        pb_string(self_obj, sel("string"))
    } else {
        pb_data_for_type(self_obj, sel("dataForPasteboardType:"), ty)
    }
}
unsafe extern "C" fn pb_set_value_for_type(self_obj: Id, _cmd: Sel, val: Id, ty: Id) {
    if is_kind_of(val, "NSString") {
        pb_set_string(self_obj, sel("setString:"), val);
    } else if is_kind_of(val, "NSData") {
        pb_set_data_for_type(self_obj, sel("setData:forPasteboardType:"), val, ty);
    }
}

// ============================================================================
// UIFeedbackGenerator & UIStoryboard
// ============================================================================

unsafe extern "C" fn fb_noop(_self: Id, _cmd: Sel) {}
unsafe extern "C" fn fb_noop_f64(_self: Id, _cmd: Sel, _v: f64) {}
unsafe extern "C" fn fb_noop_int(_self: Id, _cmd: Sel, _v: isize) {}
unsafe extern "C" fn fb_init_style(self_obj: Id, _cmd: Sel, _style: isize) -> Id {
    msg0(self_obj, "init")
}

static SB_NAME_KEY: u8 = 0;

unsafe extern "C" fn sb_with_name(_self_cls: Id, _cmd: Sel, name: Id, bundle: Id) -> Id {
    let name_str = nsstring_to_string(name).unwrap_or_default();
    if name_str.is_empty() {
        return NIL;
    }
    let b = if bundle.is_null() {
        msg0(cls("NSBundle"), "mainBundle")
    } else {
        bundle
    };
    let path: Id = msg2(b, "pathForResource:ofType:", name, nsstring("storyboardc"));
    if path.is_null() {
        return NIL;
    }
    let sb: Id = msg0(cls("UIStoryboard"), "new");
    assoc_set(sb, &SB_NAME_KEY, name);
    sb
}

unsafe extern "C" fn sb_init_vc(self_obj: Id, _cmd: Sel) -> Id {
    let name = nsstring_to_string(assoc_get(self_obj, &SB_NAME_KEY)).unwrap_or_default();
    let cls_c = CString::new("UIStoryboard").unwrap();
    let msg_c = CString::new(format!("instantiateInitialViewController ({name}.storyboardc)")).unwrap();
    PearMissingReport(1, cls_c.as_ptr(), msg_c.as_ptr(), std::ptr::null());
    NIL
}

unsafe extern "C" fn sb_vc_with_id(_self: Id, _cmd: Sel, _ident: Id) -> Id {
    let vc: Id = msg0(cls("UIViewController"), "new");
    if vc.is_null() {
        return NIL;
    }
    let _view: Id = msg0(vc, "view");
    pear_objc::objc_autorelease(vc)
}

pub unsafe fn register_classes() {
    let tc = cls("UITraitCollection");
    if !tc.is_null() {
        add_class_method(tc, "pear_current", trait_current as Imp, "@@:");
        add_class_method(tc, "currentTraitCollection", trait_current as Imp, "@@:");
        add_method(tc, "userInterfaceIdiom", trait_idiom as Imp, "q@:");
        add_method(tc, "displayScale", trait_scale as Imp, "d@:");
        add_method(tc, "userInterfaceStyle", trait_style as Imp, "q@:");
        add_method(tc, "horizontalSizeClass", trait_size_class as Imp, "q@:");
        add_method(tc, "verticalSizeClass", trait_size_class as Imp, "q@:");
        add_method(tc, "forceTouchCapability", trait_force_touch as Imp, "q@:");
        add_method(tc, "displayGamut", trait_gamut as Imp, "q@:");
        add_method(tc, "copyWithZone:", copy_self as Imp, "@@:^{_NSZone=}");
    }

    let scr = cls("UIScreen");
    if !scr.is_null() {
        add_class_method(scr, "mainScreen", screen_main as Imp, "@@:");
        add_class_method(scr, "screens", screen_screens as Imp, "@@:");
        add_method(scr, "bounds", screen_bounds as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
        add_method(scr, "nativeBounds", screen_native_bounds as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
        add_method(scr, "applicationFrame", screen_bounds as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
        add_method(scr, "scale", screen_scale as Imp, "d@:");
        add_method(scr, "nativeScale", screen_scale as Imp, "d@:");
        add_method(scr, "maximumFramesPerSecond", screen_max_fps as Imp, "q@:");
        add_method(scr, "brightness", screen_brightness as Imp, "d@:");
        add_method(scr, "setBrightness:", screen_set_brightness as Imp, "v@:d");
        add_method(scr, "displayGamut", trait_gamut as Imp, "q@:");
        add_method(scr, "traitCollection", screen_trait_collection as Imp, "@@:");
        add_method(scr, "coordinateSpace", screen_coord_space as Imp, "@@:");
        add_method(scr, "fixedCoordinateSpace", screen_coord_space as Imp, "@@:");
        add_method(scr, "mirroredScreen", screen_nil as Imp, "@@:");
        add_method(scr, "isCaptured", screen_false as Imp, "B@:");
        add_method(scr, "overscanCompensationInsets", screen_overscan as Imp, "{UIEdgeInsets=dddd}@:");
        add_method(scr, "displayLinkWithTarget:selector:", screen_display_link as Imp, "@@:@:");
        add_method(scr, "pear_update:scale:", screen_pear_update as Imp, "v@:{CGSize=dd}d");
    }

    let dev = cls("UIDevice");
    if !dev.is_null() {
        add_class_method(dev, "currentDevice", device_current as Imp, "@@:");
        add_method(dev, "name", device_ipad as Imp, "@@:");
        add_method(dev, "model", device_ipad as Imp, "@@:");
        add_method(dev, "localizedModel", device_ipad as Imp, "@@:");
        add_method(dev, "systemName", device_ipados as Imp, "@@:");
        add_method(dev, "systemVersion", device_sys_ver as Imp, "@@:");
        add_method(dev, "orientation", device_orientation as Imp, "q@:");
        add_method(dev, "isGeneratingDeviceOrientationNotifications", device_false as Imp, "B@:");
        add_method(dev, "beginGeneratingDeviceOrientationNotifications", device_noop as Imp, "v@:");
        add_method(dev, "endGeneratingDeviceOrientationNotifications", device_noop as Imp, "v@:");
        add_method(dev, "userInterfaceIdiom", device_idiom as Imp, "q@:");
        add_method(dev, "identifierForVendor", device_vendor_id as Imp, "@@:");
        add_method(dev, "isBatteryMonitoringEnabled", device_batt_enabled as Imp, "B@:");
        add_method(dev, "setBatteryMonitoringEnabled:", device_set_batt_enabled as Imp, "v@:B");
        add_method(dev, "batteryState", device_batt_state as Imp, "q@:");
        add_method(dev, "batteryLevel", device_batt_level as Imp, "f@:");
        add_method(dev, "isMultitaskingSupported", device_true as Imp, "B@:");
        add_method(dev, "isProximityMonitoringEnabled", device_false as Imp, "B@:");
        add_method(dev, "setProximityMonitoringEnabled:", device_noop_bool as Imp, "v@:B");
        add_method(dev, "proximityState", device_false as Imp, "B@:");
        add_method(dev, "playInputClick", device_noop as Imp, "v@:");
        add_method(dev, "_feedbackSupportLevel", device_zero_int as Imp, "q@:");
    }

    let resp = cls("UIResponder");
    if !resp.is_null() {
        add_method(resp, "nextResponder", resp_nil as Imp, "@@:");
        add_method(resp, "canBecomeFirstResponder", resp_false as Imp, "B@:");
        add_method(resp, "canResignFirstResponder", resp_true as Imp, "B@:");
        add_method(resp, "isFirstResponder", resp_is_first as Imp, "B@:");
        add_method(resp, "becomeFirstResponder", resp_become_first as Imp, "B@:");
        add_method(resp, "resignFirstResponder", resp_resign_first as Imp, "B@:");
        for s in [
            "touchesBegan:withEvent:",
            "touchesMoved:withEvent:",
            "touchesEnded:withEvent:",
            "touchesCancelled:withEvent:",
            "pressesBegan:withEvent:",
            "pressesEnded:withEvent:",
            "pressesChanged:withEvent:",
            "pressesCancelled:withEvent:",
        ] {
            add_method(resp, s, resp_forward_2 as Imp, "v@:@@");
        }
        add_method(resp, "motionBegan:withEvent:", resp_noop_int_id as Imp, "v@:q@");
        add_method(resp, "motionEnded:withEvent:", resp_noop_int_id as Imp, "v@:q@");
        add_method(resp, "remoteControlReceivedWithEvent:", resp_noop_id as Imp, "v@:@");
        add_method(resp, "keyCommands", resp_nil as Imp, "@@:");
        add_method(resp, "inputView", resp_nil as Imp, "@@:");
        add_method(resp, "inputAccessoryView", resp_nil as Imp, "@@:");
        add_method(resp, "undoManager", resp_nil as Imp, "@@:");
        add_method(resp, "reloadInputViews", resp_noop as Imp, "v@:");
        add_method(resp, "canPerformAction:withSender:", resp_can_perform as Imp, "B@::@");
    }

    let touch = cls("UITouch");
    if !touch.is_null() {
        add_method(touch, "initWithView:window:location:timestamp:", touch_init as Imp, "@@:@@{CGPoint=dd}d");
        add_method(touch, "pear_moveTo:phase:timestamp:", touch_pear_move as Imp, "v@:{CGPoint=dd}qd");
        add_method(touch, "timestamp", touch_timestamp as Imp, "d@:");
        add_method(touch, "phase", touch_phase as Imp, "q@:");
        add_method(touch, "tapCount", touch_tap_count as Imp, "Q@:");
        add_method(touch, "type", touch_type as Imp, "q@:");
        add_method(touch, "force", touch_force as Imp, "d@:");
        add_method(touch, "maximumPossibleForce", touch_one as Imp, "d@:");
        add_method(touch, "majorRadius", touch_radius as Imp, "d@:");
        add_method(touch, "majorRadiusTolerance", touch_radius_tol as Imp, "d@:");
        add_method(touch, "view", touch_view as Imp, "@@:");
        add_method(touch, "window", touch_window as Imp, "@@:");
        add_method(touch, "gestureRecognizers", touch_empty_arr as Imp, "@@:");
        add_method(touch, "locationInView:", touch_location_in_view as Imp, "{CGPoint=dd}@:@");
        add_method(touch, "previousLocationInView:", touch_prev_location_in_view as Imp, "{CGPoint=dd}@:@");
        add_method(touch, "preciseLocationInView:", touch_location_in_view as Imp, "{CGPoint=dd}@:@");
        add_method(touch, "precisePreviousLocationInView:", touch_prev_location_in_view as Imp, "{CGPoint=dd}@:@");
    }

    let ev = cls("UIEvent");
    if !ev.is_null() {
        add_method(ev, "initWithTouches:timestamp:", event_init as Imp, "@@:@d");
        add_method(ev, "type", event_zero as Imp, "q@:");
        add_method(ev, "subtype", event_zero as Imp, "q@:");
        add_method(ev, "timestamp", event_timestamp as Imp, "d@:");
        add_method(ev, "allTouches", event_all_touches as Imp, "@@:");
        add_method(ev, "touchesForView:", event_touches_for_view as Imp, "@@:@");
        add_method(ev, "touchesForWindow:", event_touches_for_window as Imp, "@@:@");
        add_method(ev, "coalescedTouchesForTouch:", event_coalesced as Imp, "@@:@");
        add_method(ev, "predictedTouchesForTouch:", event_predicted as Imp, "@@:@");
    }

    let pb = cls("UIPasteboard");
    if !pb.is_null() {
        add_class_method(pb, "generalPasteboard", pb_general as Imp, "@@:");
        add_class_method(pb, "pasteboardWithName:create:", pb_with_name as Imp, "@@:@B");
        add_method(pb, "name", pb_name as Imp, "@@:");
        add_method(pb, "changeCount", pb_change_count as Imp, "q@:");
        add_method(pb, "string", pb_string as Imp, "@@:");
        add_method(pb, "setString:", pb_set_string as Imp, "v@:@");
        add_method(pb, "strings", pb_strings as Imp, "@@:");
        add_method(pb, "setStrings:", pb_set_strings as Imp, "v@:@");
        add_method(pb, "hasStrings", pb_has_strings as Imp, "B@:");
        // UIPasteboard's properties are URL / URLs (the getters were
        // registered as url / urls before, which apps never call).
        add_method(pb, "URL", pb_url as Imp, "@@:");
        add_method(pb, "setURL:", pb_set_url as Imp, "v@:@");
        add_method(pb, "URLs", pb_urls as Imp, "@@:");
        add_method(pb, "setURLs:", pb_set_urls as Imp, "v@:@");
        add_method(pb, "hasURLs", pb_has_urls as Imp, "B@:");
        add_method(pb, "hasImages", pb_has_images as Imp, "B@:");
        add_method(pb, "dataForPasteboardType:", pb_data_for_type as Imp, "@@:@");
        add_method(pb, "setData:forPasteboardType:", pb_set_data_for_type as Imp, "v@:@@");
        add_method(pb, "valueForPasteboardType:", pb_value_for_type as Imp, "@@:@");
        add_method(pb, "setValue:forPasteboardType:", pb_set_value_for_type as Imp, "v@:@@");
    }

    let fb = cls("UIFeedbackGenerator");
    if !fb.is_null() {
        add_method(fb, "prepare", fb_noop as Imp, "v@:");
    }
    let imp_fb = cls("UIImpactFeedbackGenerator");
    if !imp_fb.is_null() {
        add_method(imp_fb, "initWithStyle:", fb_init_style as Imp, "@@:q");
        add_method(imp_fb, "impactOccurred", fb_noop as Imp, "v@:");
        add_method(imp_fb, "impactOccurredWithIntensity:", fb_noop_f64 as Imp, "v@:d");
    }
    let notif_fb = cls("UINotificationFeedbackGenerator");
    if !notif_fb.is_null() {
        add_method(notif_fb, "notificationOccurred:", fb_noop_int as Imp, "v@:q");
    }
    let sel_fb = cls("UISelectionFeedbackGenerator");
    if !sel_fb.is_null() {
        add_method(sel_fb, "selectionChanged", fb_noop as Imp, "v@:");
    }

    let sb = cls("UIStoryboard");
    if !sb.is_null() {
        add_class_method(sb, "storyboardWithName:bundle:", sb_with_name as Imp, "@@:@@");
        add_method(sb, "instantiateInitialViewController", sb_init_vc as Imp, "@@:");
        add_method(sb, "instantiateViewControllerWithIdentifier:", sb_vc_with_id as Imp, "@@:@");
    }
}
