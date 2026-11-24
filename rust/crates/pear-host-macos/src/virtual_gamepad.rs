//! Virtual `GCController` adapter (`PEAR_VIRTUAL_GAMEPAD`): translates host
//! keyboard and mouse input into a native `GCExtendedGamepad` exposed through
//! macOS `GameController.framework`, invoking both polled state and ObjC handler
//! blocks cleanly.

use crate::key_names::host_key_name;
use pear_objc::{
    cls, is_kind_of, msg0, msg1, msg2, msg3, nsarray_count, nsstring, nsstring_to_string,
    objc_retain, sel, CGPoint, CGRect, Id, Imp, NSRange, Sel, NIL,
};
use std::collections::{HashMap, HashSet};
use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[repr(C)]
struct BlockHeader {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
}

const RTLD_LAZY: c_int = 0x1;
const RTLD_GLOBAL: c_int = 0x8;
const DISPATCH_TIME_NOW: u64 = 0;

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_after_f(when: u64, queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn dispatch_time(when: u64, delta: i64) -> u64;
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn class_getInstanceMethod(cls: pear_objc::Class, name: Sel) -> *mut c_void;
    fn class_getClassMethod(cls: pear_objc::Class, name: Sel) -> *mut c_void;
    fn method_setImplementation(method: *mut c_void, imp: Imp) -> Imp;
    fn CGAssociateMouseAndMouseCursorPosition(connected: u32) -> i32;
    fn CGDisplayHideCursor(display: u32) -> i32;
    fn CGDisplayShowCursor(display: u32) -> i32;
    fn CGMainDisplayID() -> u32;
    fn atexit(func: extern "C" fn()) -> c_int;
    fn CACurrentMediaTime() -> f64;
}

static G_VIRTUAL_PAD: AtomicUsize = AtomicUsize::new(0);
static G_CONNECT_NOTIF: AtomicUsize = AtomicUsize::new(0);
static G_READ_SEEN: AtomicBool = AtomicBool::new(false);

static ORIG_CONTROLLERS: AtomicUsize = AtomicUsize::new(0);
static ORIG_CURRENT: AtomicUsize = AtomicUsize::new(0);
static ORIG_START_DISCOVERY: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_SNAPSHOT: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_ATTACHED: AtomicUsize = AtomicUsize::new(0);
static ORIG_VENDOR_NAME: AtomicUsize = AtomicUsize::new(0);
static ORIG_PRODUCT_CAT: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct CaptureState {
    requested: bool,
    suspended: bool,
    escape_down: bool,
}

impl CaptureState {
    fn wants_lock(&self) -> bool {
        self.requested && !self.suspended
    }
    fn set_active(&mut self, active: bool) {
        self.suspended = !active;
        if !active {
            self.escape_down = false;
        }
    }
    fn escape(&mut self, pressed: bool) -> bool {
        if !pressed {
            self.escape_down = false;
            return false;
        }
        if self.escape_down {
            return false;
        }
        self.escape_down = true;
        if !self.requested {
            return false;
        }
        self.requested = false;
        true
    }
}

#[derive(Default)]
struct PadState {
    ready: bool,
    capture: CaptureState,
    captured: bool,
    click_passthrough: bool,
    cursor_disassociated: bool,
    cursor_hidden: bool,
    cursor_api_failed: bool,
    pressed_keys: HashSet<String>,
    pressed_mouse: HashSet<String>,
    mouse_x: f32,
    mouse_y: f32,
    last_mouse_time: f64,
    sensitivity: f64,
    invert_y: bool,
    key_to_control: HashMap<String, Vec<String>>,
    mouse_to_control: HashMap<String, Vec<String>>,
    stick: HashMap<String, String>,
    button_values: HashMap<&'static str, f32>,
    dpad_xy: (f32, f32),
    lstick_xy: (f32, f32),
    rstick_xy: (f32, f32),
}

static STATE: OnceLock<Mutex<PadState>> = OnceLock::new();

fn state() -> &'static Mutex<PadState> {
    STATE.get_or_init(|| {
        Mutex::new(PadState {
            sensitivity: 1.0,
            capture: CaptureState {
                requested: true,
                suspended: false,
                escape_down: false,
            },
            ..Default::default()
        })
    })
}

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("PEAR_VIRTUAL_GAMEPAD")
            .ok()
            .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
    })
}

fn native_controller_identity() -> bool {
    std::env::var("PEAR_CONTROLLER_NATIVE")
        .ok()
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

fn known_controller_family(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["xbox", "dualshock", "dualsense", "playstation", "sony"]
        .iter()
        .any(|pat| lower.contains(pat))
}

const KNOWN_CONTROLS: [&str; 16] = [
    "a", "b", "x", "y", "l1", "r1", "l2", "r2", "dpadUp", "dpadDown", "dpadLeft", "dpadRight",
    "menu", "options", "l3", "r3",
];

fn is_dpad(control: &str) -> bool {
    matches!(control, "dpadUp" | "dpadDown" | "dpadLeft" | "dpadRight")
}

unsafe fn parse_controls(value: Id) -> Vec<String> {
    let items: Vec<Id> = if value.is_null() {
        vec![]
    } else if is_kind_of(value, "NSString") {
        vec![value]
    } else if is_kind_of(value, "NSArray") {
        (0..nsarray_count(value))
            .map(|i| pear_objc::nsarray_object_at(value, i))
            .collect()
    } else {
        vec![]
    };
    let mut out = Vec::new();
    for item in items {
        if !is_kind_of(item, "NSString") {
            continue;
        }
        if let Some(s) = nsstring_to_string(item) {
            if KNOWN_CONTROLS.contains(&s.as_str()) && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

unsafe fn dict_entries(d: Id) -> Vec<(Id, Id)> {
    let keys: Id = msg0(d, "allKeys");
    (0..nsarray_count(keys))
        .map(|i| {
            let k = pear_objc::nsarray_object_at(keys, i);
            (k, msg1(d, "objectForKey:", k))
        })
        .collect()
}

unsafe fn load_mapping(p: &mut PadState) {
    let mut keys: HashMap<String, Vec<String>> = HashMap::new();
    let mut mouse: HashMap<String, Vec<String>> = HashMap::new();
    let mut stick: HashMap<String, String> = HashMap::new();
    p.sensitivity = 1.0;
    p.invert_y = false;

    let raw = std::env::var("PEAR_VIRTUAL_GAMEPAD_MAP").unwrap_or_default();
    let mut root: Id = NIL;
    if !raw.is_empty() {
        let data: Id = msg1(nsstring(&raw), "dataUsingEncoding:", 4usize);
        let mut error: Id = NIL;
        let parsed: Id = if data.is_null() {
            NIL
        } else {
            msg3(
                cls("NSJSONSerialization"),
                "JSONObjectWithData:options:error:",
                data,
                0usize,
                &mut error as *mut Id,
            )
        };
        if !parsed.is_null() && is_kind_of(parsed, "NSDictionary") {
            root = parsed;
        } else {
            let reason = if error.is_null() {
                "not JSON".to_string()
            } else {
                nsstring_to_string(msg0(error, "localizedDescription")).unwrap_or_default()
            };
            super::log_msg("gamepad", &format!("invalid PEAR_VIRTUAL_GAMEPAD_MAP: {reason}"));
        }
    }

    if !root.is_null() {
        let buttons: Id = msg1(root, "objectForKey:", nsstring("buttons"));
        if !buttons.is_null() && is_kind_of(buttons, "NSDictionary") {
            for (k, v) in dict_entries(buttons) {
                if let Some(ks) = nsstring_to_string(k).filter(|s| !s.is_empty()) {
                    let list = parse_controls(v);
                    if !list.is_empty() {
                        keys.insert(ks.to_ascii_lowercase(), list);
                    }
                }
            }
        }
        let mouse_buttons: Id = msg1(root, "objectForKey:", nsstring("mouseButtons"));
        if !mouse_buttons.is_null() && is_kind_of(mouse_buttons, "NSDictionary") {
            let non_digits: Id = msg0(
                msg0::<Id>(cls("NSCharacterSet"), "decimalDigitCharacterSet"),
                "invertedSet",
            );
            for (k, v) in dict_entries(mouse_buttons) {
                if is_kind_of(k, "NSString") && msg0::<usize>(k, "length") > 0 {
                    let r: NSRange = msg1(k, "rangeOfCharacterFromSet:", non_digits);
                    if r.location == isize::MAX as usize {
                        let list = parse_controls(v);
                        if !list.is_empty() {
                            mouse.insert(msg0::<isize>(k, "integerValue").to_string(), list);
                        }
                    }
                }
            }
        }
        let left: Id = msg1(root, "objectForKey:", nsstring("leftStick"));
        if !left.is_null() && is_kind_of(left, "NSDictionary") {
            for (direction, key) in dict_entries(left) {
                if let (Some(d), Some(k)) = (nsstring_to_string(direction), nsstring_to_string(key)) {
                    if !k.is_empty() && matches!(d.as_str(), "up" | "down" | "left" | "right") {
                        stick.insert(d, k.to_ascii_lowercase());
                    }
                }
            }
        }
        let sensitivity: Id = msg1(root, "objectForKey:", nsstring("sensitivity"));
        if !sensitivity.is_null() && is_kind_of(sensitivity, "NSNumber") {
            p.sensitivity = msg0::<f64>(sensitivity, "doubleValue").clamp(0.2, 3.0);
        }
        let invert: Id = msg1(root, "objectForKey:", nsstring("invertY"));
        if !invert.is_null() && is_kind_of(invert, "NSNumber") {
            p.invert_y = msg0::<bool>(invert, "boolValue");
        }
    } else if raw.is_empty() {
        for (k, c) in [
            ("space", "a"),
            ("shift", "b"),
            ("e", "x"),
            ("r", "y"),
            ("q", "l1"),
            ("f", "r1"),
            ("up", "dpadUp"),
            ("down", "dpadDown"),
            ("left", "dpadLeft"),
            ("right", "dpadRight"),
            ("enter", "menu"),
            ("tab", "options"),
        ] {
            keys.insert(k.into(), vec![c.into()]);
        }
        for (k, c) in [("1", "l2"), ("0", "r2")] {
            mouse.insert(k.into(), vec![c.into()]);
        }
        for (d, k) in [("up", "w"), ("down", "s"), ("left", "a"), ("right", "d")] {
            stick.insert(d.into(), k.into());
        }
    }

    super::log_msg(
        "gamepad",
        &format!(
            "mapping keys={} mouse={} stick={} sensitivity={:.2} invertY={}",
            keys.len(),
            mouse.len(),
            stick.len(),
            p.sensitivity,
            p.invert_y as i32
        ),
    );
    p.key_to_control = keys;
    p.mouse_to_control = mouse;
    p.stick = stick;
}

// ---------------------------------------------------------------------------
// GCController swizzles

unsafe fn pad_ptr() -> Id {
    G_VIRTUAL_PAD.load(Ordering::Acquire) as Id
}

unsafe extern "C" fn gc_controllers(self_cls: Id, _cmd: Sel) -> Id {
    G_READ_SEEN.store(true, Ordering::Release);
    let orig = ORIG_CONTROLLERS.load(Ordering::Acquire);
    let arr: Id = if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(self_cls, sel("controllers"))
    } else {
        NIL
    };
    let pad = pad_ptr();
    let extras = super::physical_gamepad::extra_controllers();
    if pad.is_null() && extras.is_empty() {
        return if arr.is_null() { msg0(cls("NSArray"), "array") } else { arr };
    }
    let out: Id = if arr.is_null() {
        msg0(cls("NSMutableArray"), "array")
    } else {
        msg1(cls("NSMutableArray"), "arrayWithArray:", arr)
    };
    if !pad.is_null() && !msg1::<Id, bool>(out, "containsObject:", pad) {
        msg1::<Id, ()>(out, "addObject:", pad);
    }
    for ep in extras {
        if !ep.is_null() && !msg1::<Id, bool>(out, "containsObject:", ep) {
            msg1::<Id, ()>(out, "addObject:", ep);
        }
    }
    msg1(cls("NSArray"), "arrayWithArray:", out)
}

unsafe extern "C" fn gc_current(self_cls: Id, _cmd: Sel) -> Id {
    G_READ_SEEN.store(true, Ordering::Release);
    let orig = ORIG_CURRENT.load(Ordering::Acquire);
    let cur: Id = if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(self_cls, sel("current"))
    } else {
        NIL
    };
    if !cur.is_null() {
        return cur;
    }
    if let Some(ep) = super::physical_gamepad::extra_controllers().into_iter().next() {
        return ep;
    }
    pad_ptr()
}

extern "C" fn post_connect_main(_ctx: *mut c_void) {
    unsafe {
        pear_objc::autorelease_pool(|| {
            let pad = pad_ptr();
            if pad.is_null() {
                return;
            }
            let notif = G_CONNECT_NOTIF.load(Ordering::Acquire) as Id;
            let name = if notif.is_null() {
                nsstring("GCControllerDidConnectNotification")
            } else {
                notif
            };
            let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
            msg2::<Id, Id, ()>(nc, "postNotificationName:object:", name, pad);
        });
    }
}

extern "C" fn replay_connect_main(_ctx: *mut c_void) {
    if !G_READ_SEEN.load(Ordering::Acquire) {
        post_connect_main(std::ptr::null_mut());
    }
}

unsafe extern "C" fn gc_start_discovery(self_cls: Id, cmd: Sel, completion: Id) {
    let orig = ORIG_START_DISCOVERY.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(orig);
        f(self_cls, cmd, completion);
    }
    if !pad_ptr().is_null() {
        let when = dispatch_time(DISPATCH_TIME_NOW, 10_000_000);
        dispatch_after_f(when, &_dispatch_main_q as *const c_void, std::ptr::null_mut(), post_connect_main);
    }
}

unsafe extern "C" fn gc_is_snapshot(self_obj: Id, cmd: Sel) -> bool {
    if self_obj == pad_ptr() && !self_obj.is_null() {
        return false;
    }
    let orig = ORIG_IS_SNAPSHOT.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> bool = std::mem::transmute(orig);
        f(self_obj, cmd)
    } else {
        false
    }
}

unsafe extern "C" fn gc_is_attached(self_obj: Id, cmd: Sel) -> bool {
    if self_obj == pad_ptr() && !self_obj.is_null() {
        return true;
    }
    let orig = ORIG_IS_ATTACHED.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> bool = std::mem::transmute(orig);
        f(self_obj, cmd)
    } else {
        false
    }
}

unsafe extern "C" fn gc_vendor_name(self_obj: Id, cmd: Sel) -> Id {
    let orig = ORIG_VENDOR_NAME.load(Ordering::Acquire);
    let name: Id = if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(self_obj, cmd)
    } else {
        NIL
    };
    if self_obj == pad_ptr()
        || (!native_controller_identity() && !known_controller_family(&nsstring_to_string(name).unwrap_or_default()))
    {
        return nsstring("Xbox Wireless Controller");
    }
    name
}

unsafe extern "C" fn gc_product_category(self_obj: Id, cmd: Sel) -> Id {
    let orig = ORIG_PRODUCT_CAT.load(Ordering::Acquire);
    let cat: Id = if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(self_obj, cmd)
    } else {
        NIL
    };
    if self_obj == pad_ptr()
        || (!native_controller_identity() && !known_controller_family(&nsstring_to_string(cat).unwrap_or_default()))
    {
        return nsstring("Xbox One");
    }
    cat
}

unsafe fn swizzle_instance(c: pear_objc::Class, name: &str, imp: Imp, slot: &AtomicUsize) {
    let m = class_getInstanceMethod(c, sel(name));
    if !m.is_null() {
        let prev = method_setImplementation(m, imp);
        slot.store(prev as usize, Ordering::Release);
    }
}

unsafe fn swizzle_class(c: pear_objc::Class, name: &str, imp: Imp, slot: &AtomicUsize) {
    let m = class_getClassMethod(c, sel(name));
    if !m.is_null() {
        let prev = method_setImplementation(m, imp);
        slot.store(prev as usize, Ordering::Release);
    }
}

// ---------------------------------------------------------------------------
// Block invocation for GCExtendedGamepad elements

unsafe fn invoke_button_block(blk: Id, btn: Id, value: f32, pressed: bool) {
    if blk.is_null() {
        return;
    }
    let invoke = (*(blk as *const BlockHeader)).invoke;
    if !invoke.is_null() {
        let f: unsafe extern "C" fn(Id, Id, f32, bool) = std::mem::transmute(invoke);
        f(blk, btn, value, pressed);
    }
}

unsafe fn invoke_dpad_block(blk: Id, dpad: Id, x: f32, y: f32) {
    if blk.is_null() {
        return;
    }
    let invoke = (*(blk as *const BlockHeader)).invoke;
    if !invoke.is_null() {
        let f: unsafe extern "C" fn(Id, Id, f32, f32) = std::mem::transmute(invoke);
        f(blk, dpad, x, y);
    }
}

unsafe fn invoke_axis_block(blk: Id, axis: Id, value: f32) {
    if blk.is_null() {
        return;
    }
    let invoke = (*(blk as *const BlockHeader)).invoke;
    if !invoke.is_null() {
        let f: unsafe extern "C" fn(Id, Id, f32) = std::mem::transmute(invoke);
        f(blk, axis, value);
    }
}

unsafe fn invoke_gamepad_block(blk: Id, gamepad: Id, element: Id) {
    if blk.is_null() {
        return;
    }
    let invoke = (*(blk as *const BlockHeader)).invoke;
    if !invoke.is_null() {
        let f: unsafe extern "C" fn(Id, Id, Id) = std::mem::transmute(invoke);
        f(blk, gamepad, element);
    }
}

fn button_selector(control: &str) -> Option<&'static str> {
    Some(match control {
        "a" => "buttonA",
        "b" => "buttonB",
        "x" => "buttonX",
        "y" => "buttonY",
        "l1" => "leftShoulder",
        "r1" => "rightShoulder",
        "l2" => "leftTrigger",
        "r2" => "rightTrigger",
        "menu" => "buttonMenu",
        "options" => "buttonOptions",
        "l3" => "leftThumbstickButton",
        "r3" => "rightThumbstickButton",
        _ => return None,
    })
}

unsafe fn set_button(p: &mut PadState, control: &str, value: f32) {
    let Some(sel_name) = button_selector(control) else { return };
    let prev = p.button_values.get(sel_name).copied().unwrap_or(0.0);
    if (prev - value).abs() < 0.0001 {
        return;
    }
    p.button_values.insert(sel_name, value);
    let pad = pad_ptr();
    if pad.is_null() {
        return;
    }
    let ext: Id = msg0(pad, "extendedGamepad");
    if ext.is_null() {
        return;
    }
    let btn: Id = msg0(ext, sel_name);
    if btn.is_null() {
        return;
    }
    msg1::<f32, ()>(btn, "setValue:", value);
    let was_pressed = prev >= 0.1;
    let now_pressed = value >= 0.1;
    let vch: Id = msg0(btn, "valueChangedHandler");
    invoke_button_block(vch, btn, value, now_pressed);
    if was_pressed != now_pressed {
        let pch: Id = msg0(btn, "pressedChangedHandler");
        invoke_button_block(pch, btn, value, now_pressed);
    }
    let evch: Id = msg0(ext, "valueChangedHandler");
    invoke_gamepad_block(evch, ext, btn);
}

unsafe fn set_pad_axes(slot: &mut (f32, f32), selector: &str, x: f32, y: f32) {
    if (slot.0 - x).abs() < 0.0001 && (slot.1 - y).abs() < 0.0001 {
        return;
    }
    let prev = *slot;
    *slot = (x, y);
    let pad = pad_ptr();
    if pad.is_null() {
        return;
    }
    let ext: Id = msg0(pad, "extendedGamepad");
    if ext.is_null() {
        return;
    }
    let dpad: Id = msg0(ext, selector);
    if dpad.is_null() {
        return;
    }
    msg2::<f32, f32, ()>(dpad, "setValueForXAxis:yAxis:", x, y);
    if (prev.0 - x).abs() >= 0.0001 {
        let x_axis: Id = msg0(dpad, "xAxis");
        if !x_axis.is_null() {
            invoke_axis_block(msg0(x_axis, "valueChangedHandler"), x_axis, x);
        }
    }
    if (prev.1 - y).abs() >= 0.0001 {
        let y_axis: Id = msg0(dpad, "yAxis");
        if !y_axis.is_null() {
            invoke_axis_block(msg0(y_axis, "valueChangedHandler"), y_axis, y);
        }
    }
    let dvch: Id = msg0(dpad, "valueChangedHandler");
    invoke_dpad_block(dvch, dpad, x, y);
    let evch: Id = msg0(ext, "valueChangedHandler");
    invoke_gamepad_block(evch, ext, dpad);
}

// ---------------------------------------------------------------------------
// Cursor capture & state synchronization

extern "C" fn restore_cursor_atexit() {
    if let Ok(mut p) = state().lock() {
        restore_cursor(&mut p);
    }
}

fn restore_cursor(p: &mut PadState) {
    unsafe {
        if p.cursor_hidden {
            CGDisplayShowCursor(CGMainDisplayID());
            p.cursor_hidden = false;
        }
        if p.cursor_disassociated {
            CGAssociateMouseAndMouseCursorPosition(1);
            p.cursor_disassociated = false;
        }
    }
    p.captured = false;
}

unsafe fn foreground() -> bool {
    let app: Id = msg0(cls("NSApplication"), "sharedApplication");
    !app.is_null() && msg0::<bool>(app, "isActive")
}

fn set_cursor_captured(p: &mut PadState, capture: bool) {
    if capture {
        if !p.cursor_disassociated && !p.cursor_api_failed {
            let err = unsafe { CGAssociateMouseAndMouseCursorPosition(0) };
            p.cursor_disassociated = err == 0;
            if !p.cursor_disassociated {
                p.cursor_api_failed = true;
                unsafe {
                    super::log_msg("gamepad", &format!("cursor disassociate failed ({err}); keeping local deltas"));
                }
            }
        }
        if p.cursor_disassociated && !p.cursor_hidden {
            unsafe { CGDisplayHideCursor(CGMainDisplayID()) };
            p.cursor_hidden = true;
        }
        p.captured = true;
    } else {
        restore_cursor(p);
    }
}

fn key_pressed(p: &PadState, key: Option<&String>) -> bool {
    key.is_some_and(|k| !k.is_empty() && p.pressed_keys.contains(&k.to_ascii_lowercase()))
}

fn pressed_controls(p: &PadState) -> HashSet<String> {
    let mut set = HashSet::new();
    for k in &p.pressed_keys {
        if let Some(list) = p.key_to_control.get(k) {
            set.extend(list.iter().cloned());
        }
    }
    for m in &p.pressed_mouse {
        if let Some(list) = p.mouse_to_control.get(m) {
            set.extend(list.iter().cloned());
        }
    }
    set
}

unsafe fn sync_dpad(p: &mut PadState) {
    if !p.ready {
        return;
    }
    let c = pressed_controls(p);
    let f = |n: &str| if c.contains(n) { 1.0f32 } else { 0.0 };
    let (x, y) = (f("dpadRight") - f("dpadLeft"), f("dpadUp") - f("dpadDown"));
    set_pad_axes(&mut p.dpad_xy, "dpad", x, y);
}

unsafe fn sync_left_stick(p: &mut PadState) {
    if !p.ready {
        return;
    }
    let f = |d: &str| if key_pressed(p, p.stick.get(d)) { 1.0f32 } else { 0.0 };
    let (mut x, mut y) = (f("right") - f("left"), f("up") - f("down"));
    let len = x.hypot(y);
    if len > 1.0 {
        x /= len;
        y /= len;
    }
    set_pad_axes(&mut p.lstick_xy, "leftThumbstick", x, y);
}

unsafe fn apply_controls(p: &mut PadState, list: Option<Vec<String>>) {
    let active = pressed_controls(p);
    for c in list.unwrap_or_default() {
        if !is_dpad(&c) {
            set_button(p, &c, if active.contains(&c) { 1.0 } else { 0.0 });
        }
    }
    sync_dpad(p);
}

unsafe fn neutral(p: &mut PadState) {
    p.pressed_keys.clear();
    p.pressed_mouse.clear();
    p.mouse_x = 0.0;
    p.mouse_y = 0.0;
    if !p.ready {
        return;
    }
    let mapped: Vec<String> = p
        .key_to_control
        .values()
        .chain(p.mouse_to_control.values())
        .flatten()
        .cloned()
        .collect();
    for c in mapped {
        if !is_dpad(&c) {
            set_button(p, &c, 0.0);
        }
    }
    set_pad_axes(&mut p.dpad_xy, "dpad", 0.0, 0.0);
    set_pad_axes(&mut p.lstick_xy, "leftThumbstick", 0.0, 0.0);
    set_pad_axes(&mut p.rstick_xy, "rightThumbstick", 0.0, 0.0);
}

unsafe fn set_active_locked(p: &mut PadState, active: bool) {
    p.capture.set_active(active);
    if !active {
        p.click_passthrough = false;
        neutral(p);
        set_cursor_captured(p, false);
        return;
    }
    if p.capture.wants_lock() {
        set_cursor_captured(p, true);
    }
}

// ---------------------------------------------------------------------------
// Public hooks called from PearHostView / AppObserver / PearBackendInitialize

pub fn on_app_active(active: bool) {
    if !enabled() {
        return;
    }
    unsafe {
        pear_objc::autorelease_pool(|| {
            if let Ok(mut p) = state().lock() {
                set_active_locked(&mut p, active);
            }
        });
    }
}

pub fn handle_key(code: u16, modifiers: usize, down: bool) -> bool {
    if !enabled() {
        return false;
    }
    // Preserve Command shortcuts (Cmd+Q, Cmd+Tab, etc.) unless it's the Command key itself.
    if (modifiers & (1 << 20)) != 0 && code != 55 && code != 54 {
        return false;
    }
    let Some(name) = host_key_name(code) else {
        return false;
    };
    unsafe {
        pear_objc::autorelease_pool(|| {
            let Ok(mut p) = state().lock() else { return false };
            if !p.ready || !foreground() {
                return false;
            }
            if name == "escape" {
                if p.capture.escape(down) {
                    neutral(&mut p);
                    set_cursor_captured(&mut p, false);
                    return true;
                }
                if !p.key_to_control.get("escape").is_some_and(|v| !v.is_empty()) {
                    return false;
                }
            }
            let mapped = p.key_to_control.get(name).is_some_and(|v| !v.is_empty())
                || p.stick.values().any(|v| v == name);
            if !mapped {
                return false;
            }
            if p.pressed_keys.contains(name) == down {
                return true;
            }
            if down {
                p.pressed_keys.insert(name.to_string());
            } else {
                p.pressed_keys.remove(name);
            }
            let list = p.key_to_control.get(name).cloned();
            apply_controls(&mut p, list);
            sync_left_stick(&mut p);
            true
        })
    }
}

pub fn handle_mouse_button(button: isize, down: bool, view: Id, event: Id) -> bool {
    if !enabled() {
        return false;
    }
    unsafe {
        pear_objc::autorelease_pool(|| {
            let Ok(mut p) = state().lock() else { return false };
            if !p.ready || !foreground() {
                return false;
            }
            if button == 0 && !down && p.click_passthrough {
                p.click_passthrough = false;
                return false;
            }
            if !p.captured {
                if button == 0 && down && !view.is_null() {
                    let loc: CGPoint = msg0(event, "locationInWindow");
                    let pt: CGPoint = msg2(view, "convertPoint:fromView:", loc, NIL);
                    let b: CGRect = msg0(view, "bounds");
                    let inside = pt.x >= b.origin.x
                        && pt.x < b.origin.x + b.size.width
                        && pt.y >= b.origin.y
                        && pt.y < b.origin.y + b.size.height;
                    if inside {
                        p.capture.requested = true;
                        set_active_locked(&mut p, true);
                        p.click_passthrough = true;
                    }
                }
                return false;
            }
            let key = button.to_string();
            let list = p.mouse_to_control.get(&key).cloned();
            if down {
                p.pressed_mouse.insert(key);
            } else {
                p.pressed_mouse.remove(&key);
            }
            apply_controls(&mut p, list);
            true
        })
    }
}

pub fn is_look_captured() -> bool {
    enabled() && state().lock().ok().is_some_and(|p| p.captured)
}

pub fn handle_mouse_move(dx: f64, dy: f64) {
    if !enabled() || !dx.is_finite() || !dy.is_finite() {
        return;
    }
    unsafe {
        pear_objc::autorelease_pool(|| {
            let Ok(mut p) = state().lock() else { return };
            if !p.ready || !p.captured || !foreground() {
                return;
            }
            let scale = 0.035f32 * p.sensitivity as f32;
            // NSEvent deltaY is positive downward; GCExtendedGamepad rightThumbstick +Y is up.
            let vertical = if p.invert_y { dy as f32 } else { -(dy as f32) };
            p.mouse_x = (p.mouse_x + (dx as f32) * scale).clamp(-1.0, 1.0);
            p.mouse_y = (p.mouse_y + vertical * scale).clamp(-1.0, 1.0);
            p.last_mouse_time = CACurrentMediaTime();
        });
    }
}

unsafe extern "C" fn timer_tick(_self: Id, _cmd: Sel, _timer: Id) {
    pear_objc::autorelease_pool(|| {
        let Ok(mut p) = state().lock() else { return };
        if !p.ready {
            return;
        }
        let fg = foreground();
        if p.capture.suspended == fg {
            set_active_locked(&mut p, fg);
        }
        if !fg {
            return;
        }
        let now = CACurrentMediaTime();
        if !p.captured || now - p.last_mouse_time > 0.08 {
            p.mouse_x = 0.0;
            p.mouse_y = 0.0;
        }
        let (mx, my) = (p.mouse_x, p.mouse_y);
        set_pad_axes(&mut p.rstick_xy, "rightThumbstick", mx, my);
        p.mouse_x *= 0.7;
        p.mouse_y *= 0.7;
    });
}

pub unsafe fn install() {
    if !enabled() {
        return;
    }
    let gc_handle = dlopen(
        c"/System/Library/Frameworks/GameController.framework/GameController".as_ptr(),
        RTLD_LAZY | RTLD_GLOBAL,
    );
    if !gc_handle.is_null() {
        let sym = dlsym(gc_handle, c"GCControllerDidConnectNotification".as_ptr());
        if !sym.is_null() {
            let notif_ptr = *(sym as *const Id);
            if !notif_ptr.is_null() {
                objc_retain(notif_ptr);
                G_CONNECT_NOTIF.store(notif_ptr as usize, Ordering::Release);
            }
        }
    }

    let gc_cls = cls("GCController");
    if gc_cls.is_null() {
        super::log_msg("gamepad", "GCController class unavailable");
        return;
    }
    let pad = msg0::<Id>(gc_cls, "controllerWithExtendedGamepad");
    if pad.is_null() {
        super::log_msg("gamepad", "controllerWithExtendedGamepad returned nil");
        return;
    }
    objc_retain(pad);
    msg1::<isize, ()>(pad, "setPlayerIndex:", 0);
    G_VIRTUAL_PAD.store(pad as usize, Ordering::Release);

    swizzle_class(gc_cls, "controllers", gc_controllers as Imp, &ORIG_CONTROLLERS);
    swizzle_class(gc_cls, "current", gc_current as Imp, &ORIG_CURRENT);
    swizzle_class(
        gc_cls,
        "startWirelessControllerDiscoveryWithCompletionHandler:",
        gc_start_discovery as Imp,
        &ORIG_START_DISCOVERY,
    );
    swizzle_instance(gc_cls, "isSnapshot", gc_is_snapshot as Imp, &ORIG_IS_SNAPSHOT);
    swizzle_instance(gc_cls, "isAttachedToDevice", gc_is_attached as Imp, &ORIG_IS_ATTACHED);
    swizzle_instance(gc_cls, "vendorName", gc_vendor_name as Imp, &ORIG_VENDOR_NAME);
    swizzle_instance(gc_cls, "productCategory", gc_product_category as Imp, &ORIG_PRODUCT_CAT);

    {
        let Ok(mut p) = state().lock() else { return };
        p.pressed_keys.clear();
        p.pressed_mouse.clear();
        p.capture.requested = false;
        load_mapping(&mut p);
        p.ready = true;
    }

    atexit(restore_cursor_atexit);

    if cls("PearVirtualGamepadTimer").is_null() {
        let name = CString::new("PearVirtualGamepadTimer").unwrap();
        let c = super::alloc_class("NSObject", &name);
        if !c.is_null() {
            pear_objc::replace_method(c, "tick:", timer_tick as Imp, "v@:@");
            super::register_class(c);
        }
    }
    let target: Id = msg0(cls("PearVirtualGamepadTimer"), "new");
    if !target.is_null() {
        let f: unsafe extern "C" fn(Id, Sel, f64, Id, Sel, Id, bool) -> Id =
            std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
        let timer = f(
            cls("NSTimer"),
            sel("scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:"),
            1.0 / 60.0,
            target,
            sel("tick:"),
            NIL,
            true,
        );
        if !timer.is_null() {
            let rl: Id = msg0(cls("NSRunLoop"), "mainRunLoop");
            msg2::<Id, Id, ()>(rl, "addTimer:forMode:", timer, nsstring("NSRunLoopCommonModes"));
        }
    }

    if !std::env::var("PEAR_CONTROLLER_REPLAY").ok().is_some_and(|v| v == "0") {
        for ms in [200_i64, 1500, 3500] {
            let when = dispatch_time(DISPATCH_TIME_NOW, ms * 1_000_000);
            dispatch_after_f(
                when,
                &_dispatch_main_q as *const c_void,
                std::ptr::null_mut(),
                if ms == 200 { post_connect_main } else { replay_connect_main },
            );
        }
    }

    super::log_msg("gamepad", "virtual GCController active (click game window to capture look, Esc to release)");
}
