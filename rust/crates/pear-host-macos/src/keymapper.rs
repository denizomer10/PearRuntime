//! Keymapper (`PEAR_KEYMAP`): loads a PearSilicon `Keymap` JSON file, polls
//! modifications for live overlay updates, and translates keyboard, mouse-look,
//! and physical controller bindings into concurrent multi-touch events via
//! `PearUIKitDeliverTouch`.

use crate::key_names::host_key_name;
use pear_objc::{
    autorelease_pool, cls, is_kind_of, msg0, msg1, msg2, msg3, nsarray_count, nsarray_object_at,
    nsdata_from_bytes, nsstring, nsstring_to_string, sel, CGPoint, CGRect, Id, Imp, Sel, NIL,
};
use std::collections::{HashMap, HashSet};
use std::ffi::{c_char, c_int, c_void, CString};
use std::os::unix::fs::MetadataExt;
use std::sync::{Mutex, OnceLock};

const FINGER_WASD: u32 = 1;
const FINGER_MOUSE_LOOK: u32 = 2;
const FINGER_CTRL_LSTICK: u32 = 3;
const FINGER_CTRL_RSTICK: u32 = 4;
const FINGER_BINDING_BASE: u32 = 100;

const PHASE_BEGAN: isize = 0;
const PHASE_MOVED: isize = 1;
const PHASE_ENDED: isize = 3;

const RTLD_LAZY: c_int = 0x1;
const RTLD_GLOBAL: c_int = 0x8;

extern "C" {
    fn PearUIKitDeliverTouch(window: Id, finger_id: u32, phase: isize, point: CGPoint, timestamp: f64);
    fn CACurrentMediaTime() -> f64;
    fn CGAssociateMouseAndMouseCursorPosition(connected: u32) -> i32;
    fn CGDisplayHideCursor(display: u32) -> i32;
    fn CGDisplayShowCursor(display: u32) -> i32;
    fn CGMainDisplayID() -> u32;
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn atexit(func: extern "C" fn()) -> c_int;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Tap,
    Hold,
    Joystick,
    MouseLook,
    Swipe,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    Keyboard,
    Controller,
}

#[derive(Clone, Debug)]
struct Binding {
    finger_id: u32,
    key: String,
    source: Source,
    kind: Kind,
    point: CGPoint,
    region: CGRect,
    radius: f64,
    sensitivity: f64,
    swipe_from: CGPoint,
    swipe_to: CGPoint,
    swipe_duration: f64,
}

#[derive(Clone, Debug)]
struct PendingRelease {
    finger_id: u32,
    norm_point: CGPoint,
    release_at: f64,
}

#[derive(Clone, Debug)]
struct ActiveSwipe {
    finger_id: u32,
    from: CGPoint,
    to: CGPoint,
    start_time: f64,
    duration: f64,
}

#[derive(Default)]
struct KeymapState {
    path: String,
    last_mtime: i64,
    poll_ticks: u32,
    bindings: Vec<Binding>,
    wasd_binding: Option<Binding>,
    look_binding: Option<Binding>,
    ctrl_lstick: Option<Binding>,
    ctrl_rstick: Option<Binding>,
    has_controller_bindings: bool,
    gc_loaded: bool,
    pressed_keys: HashSet<String>,
    down_times: HashMap<u32, f64>,
    active_norm_points: HashMap<u32, CGPoint>,
    pending_releases: Vec<PendingRelease>,
    active_swipes: Vec<ActiveSwipe>,
    joy_active: bool,
    joy_dragged: bool,
    joy_start_time: f64,
    joy_last_norm: CGPoint,
    joy_target_norm: CGPoint,
    look_captured: bool,
    look_active: bool,
    look_point: CGPoint,
    look_pending: (f64, f64),
    look_last_move: f64,
    click_passthrough: bool,
    cursor_disassociated: bool,
    cursor_hidden: bool,
    cursor_api_failed: bool,
    ctrl_prev_pressed: HashMap<String, bool>,
    ctrl_lstick_active: bool,
    ctrl_lstick_norm: CGPoint,
    ctrl_rstick_active: bool,
    ctrl_rstick_norm: CGPoint,
}

static STATE: OnceLock<Mutex<KeymapState>> = OnceLock::new();

fn state() -> &'static Mutex<KeymapState> {
    STATE.get_or_init(|| Mutex::new(KeymapState::default()))
}

fn keymap_path() -> Option<String> {
    static PATH: OnceLock<Option<String>> = OnceLock::new();
    PATH.get_or_init(|| {
        std::env::var("PEAR_KEYMAP")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
    .clone()
}

pub fn enabled() -> bool {
    !super::virtual_gamepad::enabled() && keymap_path().is_some()
}

fn mtime(path: &str) -> Option<i64> {
    std::fs::metadata(path).ok().map(|m| m.mtime())
}

unsafe fn dict_get(d: Id, key: &str) -> Id {
    if d.is_null() || !is_kind_of(d, "NSDictionary") {
        return NIL;
    }
    msg1(d, "objectForKey:", nsstring(key))
}

unsafe fn dict_f64(d: Id, key: &str) -> f64 {
    let v = dict_get(d, key);
    if !v.is_null() && pear_objc::responds_to(v, "doubleValue") {
        msg0(v, "doubleValue")
    } else {
        0.0
    }
}

unsafe fn dict_point(d: Id) -> CGPoint {
    CGPoint {
        x: dict_f64(d, "x"),
        y: dict_f64(d, "y"),
    }
}

unsafe fn load_bindings_from_file(path: &str) -> Vec<Binding> {
    let Ok(bytes) = std::fs::read(path) else {
        super::log_msg("keymap", &format!("failed to read keymap at {path}"));
        return vec![];
    };
    let data = nsdata_from_bytes(&bytes);
    if data.is_null() {
        return vec![];
    }
    let mut error: Id = NIL;
    let root: Id = msg3(
        cls("NSJSONSerialization"),
        "JSONObjectWithData:options:error:",
        data,
        0usize,
        &mut error as *mut Id,
    );
    if root.is_null() || !is_kind_of(root, "NSDictionary") {
        super::log_msg("keymap", &format!("invalid JSON in {path}"));
        return vec![];
    }
    let list = dict_get(root, "bindings");
    if list.is_null() || !is_kind_of(list, "NSArray") {
        return vec![];
    }

    let mut result = Vec::new();
    let count = nsarray_count(list);
    for i in 0..count {
        let raw = nsarray_object_at(list, i);
        if !is_kind_of(raw, "NSDictionary") {
            continue;
        }
        let key_str = nsstring_to_string(dict_get(raw, "key"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        if key_str.is_empty() {
            continue;
        }
        let source = if nsstring_to_string(dict_get(raw, "source")).as_deref() == Some("controller") {
            Source::Controller
        } else {
            Source::Keyboard
        };
        let action = dict_get(raw, "action");
        if action.is_null() || !is_kind_of(action, "NSDictionary") {
            continue;
        }
        let keys_arr: Id = msg0(action, "allKeys");
        let first_key = if nsarray_count(keys_arr) > 0 {
            nsarray_object_at(keys_arr, 0)
        } else {
            NIL
        };
        let kind_name = nsstring_to_string(first_key).unwrap_or_else(|| "tap".to_string());
        let kind = match kind_name.as_str() {
            "hold" => Kind::Hold,
            "joystick" => Kind::Joystick,
            "mouseLook" => Kind::MouseLook,
            "swipe" => Kind::Swipe,
            _ => Kind::Tap,
        };
        let payload = if first_key.is_null() {
            NIL
        } else {
            msg1(action, "objectForKey:", first_key)
        };
        let mut b = Binding {
            finger_id: FINGER_BINDING_BASE + result.len() as u32,
            key: key_str,
            source,
            kind,
            point: CGPoint { x: 0.5, y: 0.5 },
            region: CGRect {
                origin: CGPoint { x: 0.2, y: 0.1 },
                size: pear_objc::CGSize { width: 0.6, height: 0.8 },
            },
            radius: 0.1,
            sensitivity: 1.0,
            swipe_from: CGPoint { x: 0.5, y: 0.5 },
            swipe_to: CGPoint { x: 0.75, y: 0.5 },
            swipe_duration: 0.25,
        };
        if !payload.is_null() && is_kind_of(payload, "NSDictionary") {
            let center = dict_get(payload, "center");
            let region = dict_get(payload, "region");
            let from = dict_get(payload, "from");
            let inner = dict_get(payload, "_0");
            if !center.is_null() {
                b.point = dict_point(center);
                let r = dict_f64(payload, "radius");
                b.radius = if r > 0.0 { r } else { 0.1 };
            } else if !region.is_null() {
                b.region = CGRect {
                    origin: dict_point(region),
                    size: pear_objc::CGSize {
                        width: dict_f64(region, "width").max(0.05),
                        height: dict_f64(region, "height").max(0.05),
                    },
                };
                let s = dict_f64(payload, "sensitivity");
                b.sensitivity = if s > 0.0 { s } else { 1.0 };
            } else if !from.is_null() || !dict_get(payload, "to").is_null() {
                b.swipe_from = dict_point(from);
                b.swipe_to = dict_point(dict_get(payload, "to"));
                let d = dict_f64(payload, "duration");
                b.swipe_duration = if d > 0.0 { d } else { 0.25 };
            } else if !inner.is_null() {
                b.point = dict_point(inner);
            } else {
                b.point = dict_point(payload);
            }
        }
        result.push(b);
    }
    result
}

unsafe fn release_all_touches_locked(k: &mut KeymapState) {
    let (_, ui_win) = super::primary_view_and_ui_window();
    let ts = CACurrentMediaTime();
    if !ui_win.is_null() {
        let active: Vec<(u32, CGPoint)> = k.active_norm_points.drain().collect();
        for (fid, norm) in active {
            PearUIKitDeliverTouch(ui_win, fid, PHASE_ENDED, to_ui_point(ui_win, norm), ts);
        }
        if k.joy_active {
            PearUIKitDeliverTouch(
                ui_win,
                FINGER_WASD,
                PHASE_ENDED,
                to_ui_point(ui_win, k.joy_last_norm),
                ts,
            );
        }
        if k.look_active {
            PearUIKitDeliverTouch(
                ui_win,
                FINGER_MOUSE_LOOK,
                PHASE_ENDED,
                to_ui_point(ui_win, k.look_point),
                ts,
            );
        }
        if k.ctrl_lstick_active {
            PearUIKitDeliverTouch(
                ui_win,
                FINGER_CTRL_LSTICK,
                PHASE_ENDED,
                to_ui_point(ui_win, k.ctrl_lstick_norm),
                ts,
            );
        }
        if k.ctrl_rstick_active {
            PearUIKitDeliverTouch(
                ui_win,
                FINGER_CTRL_RSTICK,
                PHASE_ENDED,
                to_ui_point(ui_win, k.ctrl_rstick_norm),
                ts,
            );
        }
    } else {
        k.active_norm_points.clear();
    }
    k.down_times.clear();
    k.pending_releases.clear();
    k.active_swipes.clear();
    k.pressed_keys.clear();
    k.ctrl_prev_pressed.clear();
    k.joy_active = false;
    k.joy_dragged = false;
    k.look_active = false;
    k.ctrl_lstick_active = false;
    k.ctrl_rstick_active = false;
}

unsafe fn apply_keymap_locked(k: &mut KeymapState, path: &str, log_reload: bool) {
    release_all_touches_locked(k);
    let bindings = load_bindings_from_file(path);
    k.wasd_binding = None;
    k.look_binding = None;
    k.ctrl_lstick = None;
    k.ctrl_rstick = None;
    k.has_controller_bindings = false;

    for b in &bindings {
        match b.source {
            Source::Keyboard => {
                if b.kind == Kind::Joystick && b.key == "wasd" && k.wasd_binding.is_none() {
                    k.wasd_binding = Some(b.clone());
                } else if b.kind == Kind::MouseLook && b.key == "mouse" && k.look_binding.is_none() {
                    k.look_binding = Some(b.clone());
                }
            }
            Source::Controller => {
                k.has_controller_bindings = true;
                if b.key == "leftstick" && k.ctrl_lstick.is_none() {
                    k.ctrl_lstick = Some(b.clone());
                } else if b.key == "rightstick" && k.ctrl_rstick.is_none() {
                    k.ctrl_rstick = Some(b.clone());
                }
            }
        }
    }
    if k.has_controller_bindings && !k.gc_loaded {
        dlopen(
            c"/System/Library/Frameworks/GameController.framework/GameController".as_ptr(),
            RTLD_LAZY | RTLD_GLOBAL,
        );
        k.gc_loaded = true;
    }
    let count = bindings.len();
    k.bindings = bindings;
    if log_reload {
        super::log_msg("keymap", &format!("loaded {count} binding(s) from {path}"));
    }
}

unsafe fn to_ui_point(ui_window: Id, norm: CGPoint) -> CGPoint {
    let bounds: CGRect = msg0(ui_window, "bounds");
    CGPoint {
        x: bounds.origin.x + norm.x.clamp(0.0, 1.0) * bounds.size.width,
        y: bounds.origin.y + norm.y.clamp(0.0, 1.0) * bounds.size.height,
    }
}

unsafe fn deliver_norm(finger_id: u32, phase: isize, norm: CGPoint, ts: f64) {
    let (_, ui_win) = super::primary_view_and_ui_window();
    if ui_win.is_null() {
        return;
    }
    PearUIKitDeliverTouch(ui_win, finger_id, phase, to_ui_point(ui_win, norm), ts);
}

fn set_look_captured(k: &mut KeymapState, capture: bool) {
    if capture {
        if !k.cursor_disassociated && !k.cursor_api_failed {
            let err = unsafe { CGAssociateMouseAndMouseCursorPosition(0) };
            k.cursor_disassociated = err == 0;
            if !k.cursor_disassociated {
                k.cursor_api_failed = true;
            }
        }
        if k.cursor_disassociated && !k.cursor_hidden {
            unsafe { CGDisplayHideCursor(CGMainDisplayID()) };
            k.cursor_hidden = true;
        }
        k.look_captured = true;
    } else {
        unsafe {
            if k.cursor_hidden {
                CGDisplayShowCursor(CGMainDisplayID());
                k.cursor_hidden = false;
            }
            if k.cursor_disassociated {
                CGAssociateMouseAndMouseCursorPosition(1);
                k.cursor_disassociated = false;
            }
        }
        k.look_captured = false;
        k.look_pending = (0.0, 0.0);
    }
}

extern "C" fn restore_cursor_atexit() {
    if let Ok(mut k) = state().lock() {
        set_look_captured(&mut k, false);
    }
}

unsafe fn foreground() -> bool {
    let app: Id = msg0(cls("NSApplication"), "sharedApplication");
    !app.is_null() && msg0::<bool>(app, "isActive")
}

unsafe fn update_wasd_locked(k: &mut KeymapState, ts: f64) {
    let Some(joy) = k.wasd_binding.clone() else {
        return;
    };
    let f = |key: &str| if k.pressed_keys.contains(key) { 1.0f64 } else { 0.0 };
    let mut dx = f("d") - f("a");
    let mut dy = f("s") - f("w");
    let len = dx.hypot(dy);
    if len > 0.1 {
        if len > 1.0 {
            dx /= len;
            dy /= len;
        }
        let target = CGPoint {
            x: (joy.point.x + dx * joy.radius).clamp(0.0, 1.0),
            y: (joy.point.y + dy * joy.radius).clamp(0.0, 1.0),
        };
        k.joy_target_norm = target;
        if !k.joy_active {
            k.joy_active = true;
            k.joy_dragged = false;
            k.joy_start_time = ts;
            k.joy_last_norm = joy.point;
            deliver_norm(FINGER_WASD, PHASE_BEGAN, joy.point, ts);
        } else if k.joy_dragged {
            k.joy_last_norm = target;
            deliver_norm(FINGER_WASD, PHASE_MOVED, target, ts);
        }
    } else if k.joy_active {
        k.joy_active = false;
        k.joy_dragged = false;
        deliver_norm(FINGER_WASD, PHASE_ENDED, k.joy_last_norm, ts);
    }
}

unsafe fn trigger_binding_locked(k: &mut KeymapState, b: &Binding, down: bool, ts: f64) {
    match b.kind {
        Kind::Tap => {
            if down {
                if !k.down_times.contains_key(&b.finger_id) {
                    k.pending_releases.retain(|r| r.finger_id != b.finger_id);
                    k.down_times.insert(b.finger_id, ts);
                    k.active_norm_points.insert(b.finger_id, b.point);
                    deliver_norm(b.finger_id, PHASE_BEGAN, b.point, ts);
                }
            } else if let Some(started) = k.down_times.remove(&b.finger_id) {
                let elapsed = ts - started;
                if elapsed >= 0.045 {
                    k.active_norm_points.remove(&b.finger_id);
                    deliver_norm(b.finger_id, PHASE_ENDED, b.point, ts);
                } else {
                    k.pending_releases.push(PendingRelease {
                        finger_id: b.finger_id,
                        norm_point: b.point,
                        release_at: started + 0.045,
                    });
                }
            }
        }
        Kind::Hold => {
            if down {
                if !k.down_times.contains_key(&b.finger_id) {
                    k.down_times.insert(b.finger_id, ts);
                    k.active_norm_points.insert(b.finger_id, b.point);
                    deliver_norm(b.finger_id, PHASE_BEGAN, b.point, ts);
                }
            } else if k.down_times.remove(&b.finger_id).is_some() {
                k.active_norm_points.remove(&b.finger_id);
                deliver_norm(b.finger_id, PHASE_ENDED, b.point, ts);
            }
        }
        Kind::Swipe if down => {
            k.active_swipes.retain(|s| s.finger_id != b.finger_id);
            k.active_norm_points.insert(b.finger_id, b.swipe_from);
            deliver_norm(b.finger_id, PHASE_BEGAN, b.swipe_from, ts);
            k.active_swipes.push(ActiveSwipe {
                finger_id: b.finger_id,
                from: b.swipe_from,
                to: b.swipe_to,
                start_time: ts,
                duration: b.swipe_duration.max(0.05),
            });
        }
        _ => {}
    }
}

pub fn on_app_active(active: bool) {
    if !enabled() {
        return;
    }
    unsafe {
        autorelease_pool(|| {
            if let Ok(mut k) = state().lock() {
                if !active {
                    k.click_passthrough = false;
                    set_look_captured(&mut k, false);
                    release_all_touches_locked(&mut k);
                }
            }
        });
    }
}

pub fn handle_key(code: u16, modifiers: usize, down: bool) -> bool {
    if !enabled() {
        return false;
    }
    if (modifiers & (1 << 20)) != 0 && code != 55 && code != 54 {
        return false;
    }
    let Some(name) = host_key_name(code) else {
        return false;
    };
    unsafe {
        autorelease_pool(|| {
            let Ok(mut k) = state().lock() else { return false };
            if !foreground() {
                return false;
            }
            let ts = CACurrentMediaTime();

            if name == "escape" && down && k.look_captured {
                set_look_captured(&mut k, false);
                if k.look_active {
                    k.look_active = false;
                    deliver_norm(FINGER_MOUSE_LOOK, PHASE_ENDED, k.look_point, ts);
                }
                let has_escape = k
                    .bindings
                    .iter()
                    .any(|b| b.source == Source::Keyboard && b.key == "escape");
                if !has_escape {
                    return true;
                }
            }

            if down {
                if !k.pressed_keys.insert(name.to_string()) {
                    let is_mapped = (k.wasd_binding.is_some()
                        && matches!(name, "w" | "a" | "s" | "d"))
                        || k.bindings
                            .iter()
                            .any(|b| b.source == Source::Keyboard && b.key == name);
                    return is_mapped;
                }
            } else {
                k.pressed_keys.remove(name);
            }

            if k.wasd_binding.is_some() && matches!(name, "w" | "a" | "s" | "d") {
                update_wasd_locked(&mut k, ts);
                return true;
            }

            let matched: Vec<Binding> = k
                .bindings
                .iter()
                .filter(|b| {
                    b.source == Source::Keyboard
                        && b.key == name
                        && !matches!(b.kind, Kind::Joystick | Kind::MouseLook)
                })
                .cloned()
                .collect();
            if matched.is_empty() {
                return false;
            }
            for b in &matched {
                trigger_binding_locked(&mut k, b, down, ts);
            }
            true
        })
    }
}

pub fn handle_mouse_button(button: isize, down: bool, view: Id, event: Id) -> bool {
    if !enabled() {
        return false;
    }
    unsafe {
        autorelease_pool(|| {
            let Ok(mut k) = state().lock() else { return false };
            if k.look_binding.is_none() || !foreground() {
                return false;
            }
            if button == 0 && !down && k.click_passthrough {
                k.click_passthrough = false;
                return false;
            }
            if !k.look_captured {
                if button == 0 && down && !view.is_null() {
                    let loc: CGPoint = msg0(event, "locationInWindow");
                    let pt: CGPoint = msg2(view, "convertPoint:fromView:", loc, NIL);
                    let b: CGRect = msg0(view, "bounds");
                    let inside = pt.x >= b.origin.x
                        && pt.x < b.origin.x + b.size.width
                        && pt.y >= b.origin.y
                        && pt.y < b.origin.y + b.size.height;
                    if inside {
                        set_look_captured(&mut k, true);
                        k.click_passthrough = true;
                    }
                }
                return false;
            }
            true
        })
    }
}

pub fn is_look_captured() -> bool {
    enabled() && state().lock().ok().is_some_and(|k| k.look_captured)
}

pub fn handle_mouse_move(dx: f64, dy: f64) {
    if !enabled() || !dx.is_finite() || !dy.is_finite() {
        return;
    }
    if let Ok(mut k) = state().lock() {
        if k.look_binding.is_none() || !k.look_captured {
            return;
        }
        k.look_pending.0 += dx;
        k.look_pending.1 += dy;
    }
}

fn region_mid(r: CGRect) -> CGPoint {
    CGPoint {
        x: r.origin.x + r.size.width / 2.0,
        y: r.origin.y + r.size.height / 2.0,
    }
}

unsafe fn drive_analog_stick(
    active: &mut bool,
    last_norm: &mut CGPoint,
    finger_id: u32,
    b: &Binding,
    dx: f64,
    dy: f64,
    ts: f64,
) {
    let mut x = dx;
    let mut y = -dy;
    let len = x.hypot(y);
    if len > 0.15 {
        if len > 1.0 {
            x /= len;
            y /= len;
        }
        let target = CGPoint {
            x: (b.point.x + x * b.radius).clamp(0.0, 1.0),
            y: (b.point.y + y * b.radius).clamp(0.0, 1.0),
        };
        if !*active {
            *active = true;
            *last_norm = b.point;
            deliver_norm(finger_id, PHASE_BEGAN, b.point, ts);
        } else {
            *last_norm = target;
            deliver_norm(finger_id, PHASE_MOVED, target, ts);
        }
    } else if *active {
        *active = false;
        deliver_norm(finger_id, PHASE_ENDED, *last_norm, ts);
    }
}

unsafe fn poll_physical_controllers_locked(k: &mut KeymapState, ts: f64) {
    if !k.has_controller_bindings {
        return;
    }
    let gc_cls = cls("GCController");
    if gc_cls.is_null() {
        return;
    }
    let arr: Id = msg0(gc_cls, "controllers");
    if nsarray_count(arr) == 0 {
        return;
    }
    let c = nsarray_object_at(arr, 0);
    let ext: Id = msg0(c, "extendedGamepad");
    if ext.is_null() {
        return;
    }

    let read_btn = |sel_name: &str| -> bool {
        let btn: Id = msg0(ext, sel_name);
        !btn.is_null() && msg0::<bool>(btn, "isPressed")
    };
    let dpad: Id = msg0(ext, "dpad");
    let button_states: [(&str, bool); 14] = [
        ("buttona", read_btn("buttonA")),
        ("buttonb", read_btn("buttonB")),
        ("buttonx", read_btn("buttonX")),
        ("buttony", read_btn("buttonY")),
        ("leftshoulder", read_btn("leftShoulder")),
        ("rightshoulder", read_btn("rightShoulder")),
        ("lefttrigger", read_btn("leftTrigger")),
        ("righttrigger", read_btn("rightTrigger")),
        ("menu", read_btn("buttonMenu")),
        ("options", read_btn("buttonOptions")),
        ("dpadup", !dpad.is_null() && msg0::<bool>(msg0(dpad, "up"), "isPressed")),
        ("dpaddown", !dpad.is_null() && msg0::<bool>(msg0(dpad, "down"), "isPressed")),
        ("dpadleft", !dpad.is_null() && msg0::<bool>(msg0(dpad, "left"), "isPressed")),
        ("dpadright", !dpad.is_null() && msg0::<bool>(msg0(dpad, "right"), "isPressed")),
    ];

    for (name, pressed) in button_states {
        let was = k.ctrl_prev_pressed.get(name).copied().unwrap_or(false);
        if was == pressed {
            continue;
        }
        k.ctrl_prev_pressed.insert(name.to_string(), pressed);
        let matched: Vec<Binding> = k
            .bindings
            .iter()
            .filter(|b| b.source == Source::Controller && b.key == name)
            .cloned()
            .collect();
        for b in &matched {
            trigger_binding_locked(k, b, pressed, ts);
        }
    }

    if let Some(b) = k.ctrl_lstick.clone() {
        let stick: Id = msg0(ext, "leftThumbstick");
        if !stick.is_null() {
            let dx: f32 = msg0(msg0(stick, "xAxis"), "value");
            let dy: f32 = msg0(msg0(stick, "yAxis"), "value");
            let mut active = k.ctrl_lstick_active;
            let mut norm = k.ctrl_lstick_norm;
            drive_analog_stick(&mut active, &mut norm, FINGER_CTRL_LSTICK, &b, dx as f64, dy as f64, ts);
            k.ctrl_lstick_active = active;
            k.ctrl_lstick_norm = norm;
        }
    }

    if let Some(b) = k.ctrl_rstick.clone() {
        let stick: Id = msg0(ext, "rightThumbstick");
        if !stick.is_null() {
            let dx: f32 = msg0(msg0(stick, "xAxis"), "value");
            let dy: f32 = msg0(msg0(stick, "yAxis"), "value");
            let mut active = k.ctrl_rstick_active;
            let mut norm = k.ctrl_rstick_norm;
            drive_analog_stick(&mut active, &mut norm, FINGER_CTRL_RSTICK, &b, dx as f64, dy as f64, ts);
            k.ctrl_rstick_active = active;
            k.ctrl_rstick_norm = norm;
        }
    }
}

unsafe extern "C" fn timer_tick(_self: Id, _cmd: Sel, _timer: Id) {
    autorelease_pool(|| {
        let Ok(mut k) = state().lock() else { return };
        if k.path.is_empty() {
            return;
        }
        let ts = CACurrentMediaTime();

        // Poll file mtime every ~1s for live keymap edits from PearSilicon overlay.
        k.poll_ticks = k.poll_ticks.wrapping_add(1);
        if k.poll_ticks % 60 == 0 {
            if let Some(m) = mtime(&k.path) {
                if m != k.last_mtime {
                    k.last_mtime = m;
                    let p = k.path.clone();
                    apply_keymap_locked(&mut k, &p, true);
                }
            }
        }

        // Complete pending short taps.
        let mut i = 0;
        while i < k.pending_releases.len() {
            if ts >= k.pending_releases[i].release_at {
                let r = k.pending_releases.swap_remove(i);
                k.active_norm_points.remove(&r.finger_id);
                deliver_norm(r.finger_id, PHASE_ENDED, r.norm_point, ts);
            } else {
                i += 1;
            }
        }

        // Promote initial WASD joystick touch from center to target vector.
        if k.joy_active && !k.joy_dragged && ts - k.joy_start_time >= 0.015 {
            k.joy_dragged = true;
            let target = k.joy_target_norm;
            k.joy_last_norm = target;
            deliver_norm(FINGER_WASD, PHASE_MOVED, target, ts);
        }

        // Advance active swipes.
        let mut s_idx = 0;
        while s_idx < k.active_swipes.len() {
            let sw = k.active_swipes[s_idx].clone();
            let t = ((ts - sw.start_time) / sw.duration).clamp(0.0, 1.0);
            let cur = CGPoint {
                x: sw.from.x + (sw.to.x - sw.from.x) * t,
                y: sw.from.y + (sw.to.y - sw.from.y) * t,
            };
            if t >= 1.0 {
                k.active_swipes.swap_remove(s_idx);
                k.active_norm_points.remove(&sw.finger_id);
                deliver_norm(sw.finger_id, PHASE_ENDED, cur, ts);
            } else {
                k.active_norm_points.insert(sw.finger_id, cur);
                deliver_norm(sw.finger_id, PHASE_MOVED, cur, ts);
                s_idx += 1;
            }
        }

        // Process mouseLook accumulated deltas + idle finger lift.
        if let Some(lb) = k.look_binding.clone() {
            let (dx, dy) = k.look_pending;
            k.look_pending = (0.0, 0.0);
            if k.look_captured && foreground() && (dx != 0.0 || dy != 0.0) {
                const LOOK_SCALE: f64 = 0.0015;
                let step = LOOK_SCALE * lb.sensitivity;
                if !k.look_active {
                    k.look_active = true;
                    k.look_point = region_mid(lb.region);
                    deliver_norm(FINGER_MOUSE_LOOK, PHASE_BEGAN, k.look_point, ts);
                }
                let min_x = lb.region.origin.x;
                let min_y = lb.region.origin.y;
                let max_x = min_x + lb.region.size.width;
                let max_y = min_y + lb.region.size.height;
                let mut nx = k.look_point.x + dx * step;
                let mut ny = k.look_point.y + dy * step;
                if nx < min_x || nx > max_x || ny < min_y || ny > max_y {
                    deliver_norm(FINGER_MOUSE_LOOK, PHASE_ENDED, k.look_point, ts);
                    k.look_point = region_mid(lb.region);
                    deliver_norm(FINGER_MOUSE_LOOK, PHASE_BEGAN, k.look_point, ts);
                    nx = (k.look_point.x + dx * step).clamp(min_x, max_x);
                    ny = (k.look_point.y + dy * step).clamp(min_y, max_y);
                }
                k.look_point = CGPoint { x: nx, y: ny };
                k.look_last_move = ts;
                deliver_norm(FINGER_MOUSE_LOOK, PHASE_MOVED, k.look_point, ts);
            } else if k.look_active && ts - k.look_last_move >= 0.35 {
                k.look_active = false;
                deliver_norm(FINGER_MOUSE_LOOK, PHASE_ENDED, k.look_point, ts);
            }
        }

        poll_physical_controllers_locked(&mut k, ts);
    });
}

pub unsafe fn install() {
    if !enabled() {
        return;
    }
    let Some(path) = keymap_path() else {
        return;
    };
    {
        let Ok(mut k) = state().lock() else { return };
        k.path = path.clone();
        k.last_mtime = mtime(&path).unwrap_or(0);
        apply_keymap_locked(&mut k, &path, true);
    }

    atexit(restore_cursor_atexit);

    if cls("PearKeymapTimer").is_null() {
        let name = CString::new("PearKeymapTimer").unwrap();
        let c = super::alloc_class("NSObject", &name);
        if !c.is_null() {
            pear_objc::replace_method(c, "tick:", timer_tick as Imp, "v@:@");
            super::register_class(c);
        }
    }
    let target: Id = msg0(cls("PearKeymapTimer"), "new");
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
}
