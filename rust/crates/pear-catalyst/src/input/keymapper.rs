//! The keymapper: loads the keymap and turns keyboard/mouse input into
//! synthesized touches (see `touch_synth`). Each binding drives a *named*
//! virtual finger, so several bindings can be down at once.

use crate::input::{controller, touch_synth, virtual_gamepad};
use crate::rt;
use pear_objc::{cls, msg0, msg1, nsstring, CGPoint, CGRect, CGSize, Id, Imp, Sel};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const NIL: Id = std::ptr::null_mut();

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Kind {
    #[default]
    Tap,
    Hold,
    Joystick,
    MouseLook,
    Swipe,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Source {
    #[default]
    Keyboard,
    Controller,
}

#[derive(Clone, Debug, Default)]
pub struct Binding {
    pub key: String,
    pub source: Source,
    pub kind: Kind,
    pub point: CGPoint,  // normalized, for tap/hold/joystick
    pub region: CGRect,  // normalized, for mouseLook
    pub radius: f64,
    pub sensitivity: f64,
    pub swipe_from: CGPoint,
    pub swipe_to: CGPoint,
    pub swipe_duration: f64,
}

/// Main-thread state (input handlers, timers and notifications all run on main).
#[derive(Default)]
pub struct State {
    pub bindings: Vec<Rc<Binding>>,
    pub joystick_binding: Option<Rc<Binding>>, // keyboard joystick (wasd)
    look_binding: Option<Rc<Binding>>,
    joy_up: bool,
    joy_down: bool,
    joy_left: bool,
    joy_right: bool,
    joystick_active: bool,
    joystick_touch: CGPoint,
    joystick_target: CGPoint,
    joystick_generation: u64,
    look_active: bool,
    look_point: CGPoint,
    look_generation: u64,
    look_flush_scheduled: bool,
    look_pending: (f32, f32),
    last_press: HashMap<isize, f64>,
    last_any_press: f64,
    keymap_generation: u64,
}

struct MainCell(UnsafeCell<Option<State>>);
unsafe impl Sync for MainCell {}
static STATE: MainCell = MainCell(UnsafeCell::new(None));

pub fn st() -> &'static mut State {
    unsafe { (*STATE.0.get()).get_or_insert_with(State::default) }
}

pub fn binding_for(key: &str, source: Source) -> Option<Rc<Binding>> {
    let k = key.to_lowercase();
    st().bindings.iter().find(|b| b.source == source && b.key.to_lowercase() == k).cloned()
}

// ---------------------------------------------------------------------------
// Keymap loading

fn kind_from(s: &str) -> Kind {
    match s {
        "hold" => Kind::Hold,
        "joystick" => Kind::Joystick,
        "mouseLook" => Kind::MouseLook,
        "swipe" => Kind::Swipe,
        _ => Kind::Tap,
    }
}

unsafe fn get(d: Id, key: &str) -> Id {
    if d.is_null() || !pear_objc::is_kind_of(d, "NSDictionary") {
        return NIL;
    }
    msg1(d, "objectForKey:", nsstring(key))
}

/// `[d[key] doubleValue]` (0 for anything without a numeric value).
unsafe fn dbl(d: Id, key: &str) -> f64 {
    let v = get(d, key);
    if !v.is_null() && pear_objc::responds_to(v, "doubleValue") { msg0(v, "doubleValue") } else { 0.0 }
}

unsafe fn point(d: Id) -> CGPoint {
    CGPoint { x: dbl(d, "x"), y: dbl(d, "y") }
}

unsafe fn load_keymap(path: &str) -> Vec<Rc<Binding>> {
    let data: Id = msg1(cls("NSData"), "dataWithContentsOfFile:", nsstring(path));
    if data.is_null() {
        rt::log(&format!("[PearRuntime] keymap not found at {path}"));
        return vec![];
    }
    let mut error: Id = NIL;
    let root: Id = pear_objc::msg3(cls("NSJSONSerialization"), "JSONObjectWithData:options:error:", data, 0usize, &mut error as *mut Id);
    if root.is_null() || !pear_objc::is_kind_of(root, "NSDictionary") {
        rt::log(&format!("[PearRuntime] invalid keymap: {}", if error.is_null() { "(null)".into() } else { rt::description(error) }));
        return vec![];
    }
    let list = get(root, "bindings");
    let mut result = Vec::new();
    if list.is_null() || !pear_objc::is_kind_of(list, "NSArray") {
        return result;
    }
    for i in 0..pear_objc::nsarray_count(list) {
        let raw = pear_objc::nsarray_object_at(list, i);
        if !pear_objc::is_kind_of(raw, "NSDictionary") {
            continue;
        }
        let mut b = Binding::default();
        let key = get(raw, "key");
        b.key = if !key.is_null() && pear_objc::is_kind_of(key, "NSString") { rt::ns_string(key) } else { String::new() };
        let source = get(raw, "source");
        b.source = if !source.is_null() && pear_objc::is_kind_of(source, "NSString") && rt::ns_string(source) == "controller" {
            Source::Controller
        } else {
            Source::Keyboard
        };
        let action = get(raw, "action");
        if !action.is_null() && pear_objc::is_kind_of(action, "NSDictionary") {
            let first: Id = msg0(msg0::<Id>(action, "allKeys"), "firstObject");
            let payload: Id = if first.is_null() { NIL } else { msg1(action, "objectForKey:", first) };
            b.kind = if !first.is_null() && pear_objc::is_kind_of(first, "NSString") { kind_from(&rt::ns_string(first)) } else { Kind::Tap };
            if !payload.is_null() && pear_objc::is_kind_of(payload, "NSDictionary") {
                if !get(payload, "center").is_null() {
                    b.point = point(get(payload, "center"));
                    b.radius = dbl(payload, "radius");
                } else if !get(payload, "region").is_null() {
                    let r = get(payload, "region");
                    b.region = CGRect { origin: point(r), size: CGSize { width: dbl(r, "width"), height: dbl(r, "height") } };
                    let s = dbl(payload, "sensitivity");
                    b.sensitivity = if s != 0.0 { s } else { 1.0 };
                } else if !get(payload, "from").is_null() || !get(payload, "to").is_null() {
                    b.swipe_from = point(get(payload, "from"));
                    b.swipe_to = point(get(payload, "to"));
                    let d = dbl(payload, "duration");
                    b.swipe_duration = if d != 0.0 { d } else { 0.25 };
                } else if !get(payload, "_0").is_null() {
                    b.point = point(get(payload, "_0"));
                } else {
                    b.point = point(payload);
                }
            }
        }
        result.push(Rc::new(b));
    }
    result
}

/// (Re)loads the keymap and re-resolves the joystick and look bindings, so
/// edits saved by PearSilicon.app's overlay apply without relaunching.
fn apply_keymap(path: Option<&str>) {
    let bindings = unsafe { pear_objc::autorelease_pool(|| path.map(|p| load_keymap(p)).unwrap_or_default()) };
    let s = st();
    s.bindings = bindings;
    s.joystick_binding = None;
    s.look_binding = None;
    for b in s.bindings.clone() {
        if b.source != Source::Keyboard {
            continue;
        }
        if b.kind == Kind::Joystick && s.joystick_binding.is_none() {
            s.joystick_binding = Some(b.clone());
        }
        if b.kind == Kind::MouseLook && s.look_binding.is_none() {
            s.look_binding = Some(b.clone());
        }
    }
    rt::log(&format!("[PearRuntime] loaded {} bindings from {}", s.bindings.len(), path.unwrap_or("(null)")));
}

fn mtime(path: &str) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| m.mtime())
}

/// Polls the keymap's modification time once a second (the file is replaced
/// atomically, so a vnode watch would need re-arming every time).
fn watch_keymap(path: Option<String>) {
    let Some(path) = path.filter(|p| !p.is_empty()) else { return };
    let s = st();
    s.keymap_generation += 1;
    let generation = s.keymap_generation;
    let mut last = mtime(&path).unwrap_or(0);
    rt::main_ticker(1.0, 1.0, 0.2, move || {
        if st().keymap_generation != generation {
            return true;
        }
        if let Some(m) = mtime(&path) {
            if m != last {
                last = m;
                apply_keymap(Some(&path));
            }
        }
        false
    });
}

// ---------------------------------------------------------------------------
// Touch synthesis

fn finger(down: bool, key: &str, x: f64, y: f64) {
    unsafe { pear_objc::autorelease_pool(|| touch_synth::synthesize_virtual_finger(down, nsstring(key), x, y)) };
}

/// A stable finger name per binding, so its hold/joystick runs alongside others.
fn finger_key(b: &Binding) -> String {
    format!("f:{}", b.key.to_lowercase())
}

/// A swipe: press at `from`, drag to `to` over `duration`, lift.
fn synthesize_swipe(b: &Binding) {
    let key = finger_key(b);
    let (from, to) = (b.swipe_from, b.swipe_to);
    let duration = if b.swipe_duration > 0.0 { b.swipe_duration } else { 0.25 };
    const STEPS: i32 = 14;
    finger(true, &key, from.x, from.y);
    for i in 1..=STEPS {
        let t = i as f64 / STEPS as f64;
        let (nx, ny) = (from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        let key = key.clone();
        rt::after_main(t * duration, move || {
            finger(true, &key, nx, ny);
            if i == STEPS {
                finger(false, &key, nx, ny);
            }
        });
    }
}

pub fn test_swipe() {
    synthesize_swipe(&Binding {
        kind: Kind::Swipe,
        key: "test-swipe".into(),
        swipe_from: CGPoint { x: 0.3, y: 0.5 },
        swipe_to: CGPoint { x: 0.7, y: 0.5 },
        swipe_duration: 0.3,
        ..Default::default()
    });
}

pub fn synthesize_touch(b: &Binding, down: bool, p: CGPoint) {
    match b.kind {
        // A tap is a complete gesture: one down/up pair on the press only.
        Kind::Tap if down => {
            let key = finger_key(b);
            finger(true, &key, p.x, p.y);
            rt::after_main(0.05, move || finger(false, &key, p.x, p.y));
        }
        // Press-hold: a finger that stays down until the key is released.
        Kind::Hold => finger(down, &finger_key(b), p.x, p.y),
        Kind::Swipe if down => synthesize_swipe(b),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Joystick: while any direction is held the touch sits at
// `center + direction * radius`; moving between keys drags the finger.

pub fn update_joystick_with_binding(b: Option<&Rc<Binding>>, mut dx: f64, mut dy: f64) {
    let Some(b) = b else { return };
    let len = (dx * dx + dy * dy).sqrt();
    let want = len > 0.15; // analog dead zone; keyboard values are 0/1
    if len > 1.0 {
        dx /= len;
        dy /= len;
    }
    let center = b.point;
    let target = CGPoint { x: center.x + dx * b.radius, y: center.y + dy * b.radius };
    const KEY: &str = "f:joystick";
    let s = st();
    if want && !s.joystick_active {
        s.joystick_active = true;
        s.joystick_touch = target;
        s.joystick_target = target;
        s.joystick_generation += 1;
        let generation = s.joystick_generation;
        // Press at the stick centre, then drag next frame: mobile sticks use
        // (current - touchDown), so jumping straight to the offset is a zero vector.
        finger(true, KEY, center.x, center.y);
        rt::after_main(0.02, move || {
            let s = st();
            if s.joystick_active && generation == s.joystick_generation {
                finger(true, KEY, s.joystick_target.x, s.joystick_target.y);
            }
        });
    } else if want && s.joystick_active {
        s.joystick_touch = target;
        s.joystick_target = target;
        finger(true, KEY, target.x, target.y);
    } else if !want && s.joystick_active {
        s.joystick_active = false;
        s.joystick_generation += 1;
        finger(false, KEY, s.joystick_touch.x, s.joystick_touch.y);
    }
}

fn update_joystick() {
    let s = st();
    let f = |v: bool| if v { 1.0 } else { 0.0 };
    let (dx, dy) = (f(s.joy_right) - f(s.joy_left), f(s.joy_down) - f(s.joy_up));
    let b = s.joystick_binding.clone();
    update_joystick_with_binding(b.as_ref(), dx, dy);
}

// ---------------------------------------------------------------------------
// Mouse look: mouse movement drags a finger inside the mouseLook region; it
// lifts shortly after movement stops and re-presses at the centre when the
// drag would leave the region, so rotation can continue indefinitely.

const LOOK_KEY: &str = "f:mouselook";
const LOOK_SCALE: f64 = 0.0015; // mouse points -> normalized window units

fn mid(r: CGRect) -> CGPoint {
    CGPoint { x: r.origin.x + r.size.width / 2.0, y: r.origin.y + r.size.height / 2.0 }
}

fn look_flush() {
    let s = st();
    s.look_flush_scheduled = false;
    let Some(b) = s.look_binding.clone() else { return };
    let (dx, dy) = s.look_pending;
    s.look_pending = (0.0, 0.0);
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    // GCMouse reports movement globally: only steer while the game is in front.
    if unsafe { msg0::<isize>(crate::core::runtime::ui_app(), "applicationState") } != 0 {
        return;
    }
    let (dx, dy) = (dx as f64, dy as f64);
    let region = b.region;
    let step = LOOK_SCALE * if b.sensitivity > 0.0 { b.sensitivity } else { 1.0 };
    if !s.look_active {
        s.look_active = true;
        s.look_point = mid(region);
        finger(true, LOOK_KEY, s.look_point.x, s.look_point.y);
    }
    let (min_x, min_y) = (region.origin.x, region.origin.y);
    let (max_x, max_y) = (min_x + region.size.width, min_y + region.size.height);
    let mut nx = s.look_point.x + dx * step;
    let mut ny = s.look_point.y + dy * step;
    if nx < min_x || nx > max_x || ny < min_y || ny > max_y {
        finger(false, LOOK_KEY, s.look_point.x, s.look_point.y);
        s.look_point = mid(region);
        finger(true, LOOK_KEY, s.look_point.x, s.look_point.y);
        nx = (s.look_point.x + dx * step).max(min_x).min(max_x);
        ny = (s.look_point.y + dy * step).max(min_y).min(max_y);
    }
    s.look_point = CGPoint { x: nx, y: ny };
    finger(true, LOOK_KEY, nx, ny);
    // Keep the pointer in the window, or clicks land on the desktop and
    // GCMouse stops reporting deltas.
    unsafe { touch_synth::contain_cursor_in_window() };
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!("[PearRuntime][look] d=({dx:.1},{dy:.1}) at ({nx:.3},{ny:.3})"));
    }
    s.look_generation += 1;
    let generation = s.look_generation;
    rt::after_main(0.35, move || {
        let s = st();
        if generation != s.look_generation || !s.look_active {
            return;
        }
        s.look_active = false;
        finger(false, LOOK_KEY, s.look_point.x, s.look_point.y);
    });
}

/// Coalesces raw mouse movement (GCMouse fires at device rate) to ~60 Hz.
pub fn update_mouse_look(dx: f32, dy: f32) {
    if virtual_gamepad::enabled() {
        virtual_gamepad::virtual_gamepad_mouse(dx, dy);
        return;
    }
    let s = st();
    if s.look_binding.is_none() {
        return;
    }
    s.look_pending.0 += dx;
    s.look_pending.1 += dy;
    if s.look_flush_scheduled {
        return;
    }
    s.look_flush_scheduled = true;
    rt::after_main(1.0 / 60.0, look_flush);
}

// ---------------------------------------------------------------------------
// Input rate guard (mandatory): a held key and a scripted repeat both look like
// machine input. Per key ~8 presses/s, overall ~25/s; releases never blocked.

const MIN_KEY_INTERVAL: f64 = 0.12;
const MIN_ANY_KEY_INTERVAL: f64 = 0.04;

extern "C" {
    fn CACurrentMediaTime() -> f64;
}

fn input_rate_allows(code: isize) -> bool {
    let s = st();
    let now = unsafe { CACurrentMediaTime() };
    if now - s.last_any_press < MIN_ANY_KEY_INTERVAL {
        return false;
    }
    let last = s.last_press.get(&code).copied().unwrap_or(0.0);
    if now - last < MIN_KEY_INTERVAL {
        if rt::env_set("PEAR_DEBUG_TOUCH") {
            rt::log(&format!("[PearRuntime][inputrate] rejected key {code} ({:.1} ms since last)", (now - last) * 1000.0));
        }
        return false;
    }
    s.last_any_press = now;
    s.last_press.insert(code, now);
    true
}

/// One key state change → touches (plain and chained handler paths).
pub fn handle_key_press(code: isize, pressed: bool) {
    let key = key_name_for_code(code);
    if virtual_gamepad::enabled() {
        unsafe {
            pear_objc::autorelease_pool(|| virtual_gamepad::virtual_gamepad_key(key.as_deref().map(|k| nsstring(k)).unwrap_or(NIL), pressed));
        }
        return;
    }
    let Some(key) = key else { return };
    if pressed && !input_rate_allows(code) {
        return;
    }
    // W/A/S/D feed the joystick only while a joystick binding exists.
    let s = st();
    if s.joystick_binding.is_some() {
        let slot = match key.as_str() {
            "w" => Some(&mut s.joy_up),
            "a" => Some(&mut s.joy_left),
            "s" => Some(&mut s.joy_down),
            "d" => Some(&mut s.joy_right),
            _ => None,
        };
        if let Some(slot) = slot {
            *slot = pressed;
            update_joystick();
            return;
        }
    }
    let Some(b) = binding_for(&key, Source::Keyboard) else { return };
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!("[PearRuntime][key] {key} {}", if pressed { "down" } else { "up" }));
    }
    synthesize_touch(&b, pressed, b.point);
}

// ---------------------------------------------------------------------------
// Handler chaining: GameController has one handler per input stream; swizzle
// the setters so the game's handler and ours both run (the game's first).

static ORIGINAL_SET_MOUSE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_SET_KEY: AtomicUsize = AtomicUsize::new(0);
static GAME_MOUSE: AtomicUsize = AtomicUsize::new(0); // copied block
static GAME_KEY: AtomicUsize = AtomicUsize::new(0);
static COMBINED_MOUSE: AtomicUsize = AtomicUsize::new(0);
static COMBINED_KEY: AtomicUsize = AtomicUsize::new(0);

type SetHandler = unsafe extern "C" fn(Id, Sel, Id);
type MouseBlock = unsafe extern "C" fn(Id, Id, f32, f32);
type KeyBlock = unsafe extern "C" fn(Id, Id, Id, isize, bool);

unsafe fn store_block(slot: &AtomicUsize, block: Id) {
    let old = slot.swap(rt::copy_block(block) as usize, Ordering::AcqRel);
    rt::release_block(old as Id);
}

unsafe fn chained_setter(original: &AtomicUsize, game: &AtomicUsize, combined: &AtomicUsize, this: Id, cmd: Sel, handler: Id) {
    let o = original.load(Ordering::Acquire);
    if o == 0 {
        return;
    }
    let set: SetHandler = std::mem::transmute(o);
    let c = combined.load(Ordering::Acquire) as Id;
    if handler == c {
        set(this, cmd, handler);
        return;
    }
    // The game installed (or cleared) its own handler: remember it and put
    // the chained handler back in its place.
    store_block(game, handler);
    set(this, cmd, c);
}

unsafe extern "C" fn set_mouse_moved(this: Id, cmd: Sel, handler: Id) {
    chained_setter(&ORIGINAL_SET_MOUSE, &GAME_MOUSE, &COMBINED_MOUSE, this, cmd, handler);
}
unsafe extern "C" fn set_key_changed(this: Id, cmd: Sel, handler: Id) {
    chained_setter(&ORIGINAL_SET_KEY, &GAME_KEY, &COMBINED_KEY, this, cmd, handler);
}

unsafe extern "C" fn combined_mouse(_block: Id, input: Id, dx: f32, dy: f32) {
    let game = GAME_MOUSE.load(Ordering::Acquire) as Id;
    if !game.is_null() && !virtual_gamepad::enabled() {
        if let Some(f) = rt::block_invoke::<MouseBlock>(game) {
            f(game, input, dx, dy);
        }
    }
    update_mouse_look(dx, dy);
}

unsafe extern "C" fn combined_key(_block: Id, input: Id, button: Id, code: isize, pressed: bool) {
    let game = GAME_KEY.load(Ordering::Acquire) as Id;
    if !game.is_null() && !virtual_gamepad::enabled() {
        if let Some(f) = rt::block_invoke::<KeyBlock>(game) {
            f(game, input, button, code, pressed);
        }
    }
    handle_key_press(code, pressed);
}

fn install_handler_chaining() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let swizzle = |class: &str, selector: &str, imp: Imp, slot: &AtomicUsize| {
            let c = rt::class_named(class);
            let m = if c.is_null() { std::ptr::null_mut() } else { rt::class_getInstanceMethod(c, pear_objc::sel(selector)) };
            if !m.is_null() {
                slot.store(rt::method_setImplementation(m, imp) as usize, Ordering::Release);
            }
        };
        swizzle("GCMouseInput", "setMouseMovedHandler:", set_mouse_moved as Imp, &ORIGINAL_SET_MOUSE);
        swizzle("GCKeyboardInput", "setKeyChangedHandler:", set_key_changed as Imp, &ORIGINAL_SET_KEY);
        rt::log(&format!(
            "[PearRuntime][input] handler chaining installed (mouse={} keyboard={})",
            (ORIGINAL_SET_MOUSE.load(Ordering::Acquire) != 0) as i32,
            (ORIGINAL_SET_KEY.load(Ordering::Acquire) != 0) as i32
        ));
    });
}

/// Installs `combined` on `input` through the original setter when swizzled.
unsafe fn attach(input: Id, selector: &str, original: &AtomicUsize, combined: Id) {
    if input.is_null() {
        return;
    }
    let o = original.load(Ordering::Acquire);
    if o != 0 && pear_objc::responds_to(input, selector) {
        let set: SetHandler = std::mem::transmute(o);
        set(input, pear_objc::sel(selector), combined);
    } else {
        msg1::<Id, ()>(input, selector, combined);
    }
}

extern "C" {
    static GCKeyboardDidConnectNotification: Id;
    static GCMouseDidConnectNotification: Id;
}

unsafe fn install_keyboard() {
    install_handler_chaining();
    if COMBINED_KEY.load(Ordering::Acquire) == 0 {
        COMBINED_KEY.store(rt::global_block(combined_key as *const c_void) as usize, Ordering::Release);
    }
    let attach_keyboard = |keyboard: Id| unsafe {
        if !keyboard.is_null() {
            attach(msg0(keyboard, "keyboardInput"), "setKeyChangedHandler:", &ORIGINAL_SET_KEY, COMBINED_KEY.load(Ordering::Acquire) as Id);
        }
    };
    let coalesced: Id = msg0(cls("GCKeyboard"), "coalescedKeyboard");
    if !coalesced.is_null() {
        rt::log("[PearRuntime][input] keyboard attached (coalesced)");
        attach_keyboard(coalesced);
    } else {
        rt::log("[PearRuntime][input] no GCKeyboard.coalescedKeyboard yet");
    }
    rt::observe_main(&rt::ns_string(GCKeyboardDidConnectNotification), move |note| unsafe {
        rt::log("[PearRuntime][input] keyboard connected");
        attach_keyboard(msg0(note, "object"));
    });
}

unsafe fn install_mouse() {
    install_handler_chaining();
    if COMBINED_MOUSE.load(Ordering::Acquire) == 0 {
        COMBINED_MOUSE.store(rt::global_block(combined_mouse as *const c_void) as usize, Ordering::Release);
    }
    let attach_mouse = |mouse: Id| unsafe {
        if !mouse.is_null() {
            attach(msg0(mouse, "mouseInput"), "setMouseMovedHandler:", &ORIGINAL_SET_MOUSE, COMBINED_MOUSE.load(Ordering::Acquire) as Id);
        }
    };
    let current: Id = msg0(cls("GCMouse"), "current");
    if !current.is_null() {
        rt::log("[PearRuntime][input] mouse attached (current)");
        attach_mouse(current);
    } else {
        rt::log("[PearRuntime][input] no GCMouse.current yet");
    }
    rt::observe_main(&rt::ns_string(GCMouseDidConnectNotification), move |note| unsafe {
        rt::log("[PearRuntime][input] mouse connected");
        attach_mouse(msg0(note, "object"));
    });
}

// ---------------------------------------------------------------------------
// System key sound suppression: UIKit beeps for unhandled key presses; consume
// only the events a keyboard binding (or the WASD joystick) already owns.

unsafe fn key_name_for_event(event: Id) -> Option<String> {
    let code: u16 = msg0(event, "keyCode");
    let named = match code {
        123 => "left",
        124 => "right",
        125 => "down",
        126 => "up",
        115 => "home",
        119 => "end",
        116 => "pageup",
        121 => "pagedown",
        51 => "backspace",
        117 => "delete",
        48 => "tab",
        36 | 76 => "enter",
        53 => "escape",
        122 => "f1",
        120 => "f2",
        99 => "f3",
        118 => "f4",
        96 => "f5",
        97 => "f6",
        98 => "f7",
        100 => "f8",
        101 => "f9",
        109 => "f10",
        103 => "f11",
        111 => "f12",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.into());
    }
    let chars: Id = msg0(event, "charactersIgnoringModifiers");
    if chars.is_null() || !pear_objc::is_kind_of(chars, "NSString") || msg0::<usize>(chars, "length") == 0 {
        return None;
    }
    let c: u16 = msg1(chars, "characterAtIndex:", 0usize);
    let name = match c {
        0x20 => "space",
        0x09 => "tab",
        0x0d | 0x0a => "enter",
        0x1b => "escape",
        0x7f => "backspace",
        0x2d => "minus",
        0x3d => "equal",
        0x5b => "openbracket",
        0x5d => "closebracket",
        0x5c => "backslash",
        0x3b => "semicolon",
        0x27 => "quote",
        0x60 => "grave",
        0x2c => "comma",
        0x2e => "period",
        0x2f => "slash",
        _ => "",
    };
    if !name.is_empty() {
        return Some(name.into());
    }
    let lower: Id = msg0(chars, "lowercaseString");
    if msg0::<usize>(lower, "length") == 1 {
        let l: u16 = msg1(lower, "characterAtIndex:", 0usize);
        if (b'a' as u16..=b'z' as u16).contains(&l) || (b'0' as u16..=b'9' as u16).contains(&l) {
            return Some(rt::ns_string(lower));
        }
    }
    None
}

unsafe fn install_key_event_silencer() {
    if st().bindings.is_empty() {
        return;
    }
    let event_class = rt::class_named("NSEvent");
    if event_class.is_null() || !pear_objc::responds_to(event_class, "addLocalMonitorForEventsMatchingMask:handler:") {
        return;
    }
    let mask: usize = (1 << 10) | (1 << 11); // keyDown | keyUp
    let handler = rt::block1_ret(|event| unsafe {
        let Some(name) = key_name_for_event(event) else { return event };
        let mut mapped = binding_for(&name, Source::Keyboard).is_some();
        if !mapped && st().joystick_binding.is_some() {
            mapped = matches!(name.as_str(), "w" | "a" | "s" | "d");
        }
        if mapped { NIL } else { event }
    });
    let monitor: Id = pear_objc::msg2(event_class, "addLocalMonitorForEventsMatchingMask:handler:", mask, handler);
    rt::release_block(handler);
    if !monitor.is_null() {
        pear_objc::objc_retain(monitor);
    }
    rt::log(&format!("[PearRuntime][input] system key sound suppression {}", if monitor.is_null() { "unavailable" } else { "installed" }));
}

// ---------------------------------------------------------------------------
// GCKeyCode (USB HID usage ids) → keymap key names; None means no binding
// can name the key.

pub fn key_name_for_code(code: isize) -> Option<String> {
    let s = match code {
        4..=29 => return Some(((b'a' + (code - 4) as u8) as char).to_string()),
        30..=38 => return Some((1 + code - 30).to_string()),
        39 => "0",
        40 | 88 => "enter",
        41 => "escape",
        42 => "backspace",
        76 => "delete",
        43 => "tab",
        44 => "space",
        73 => "insert",
        74 => "home",
        77 => "end",
        75 => "pageup",
        78 => "pagedown",
        80 => "left",
        79 => "right",
        82 => "up",
        81 => "down",
        45 => "minus",
        46 => "equal",
        47 => "openbracket",
        48 => "closebracket",
        49 => "backslash",
        51 => "semicolon",
        52 => "quote",
        53 => "grave",
        54 => "comma",
        55 => "period",
        56 => "slash",
        100 => "backslash", // non-US backslash
        50 => "backslash",  // non-US pound
        225 | 229 => "shift",
        224 | 228 => "control",
        226 | 230 => "option",
        227 | 231 => "command",
        57 => "capslock",
        58..=69 => return Some(format!("f{}", code - 57)),
        104..=111 => return Some(format!("f{}", code - 91)),
        98 => "keypad0",
        89..=97 => return Some(format!("keypad{}", code - 88)),
        99 => "keypadperiod",
        85 => "keypadasterisk",
        87 => "keypadplus",
        86 => "keypadminus",
        84 => "keypadslash",
        103 => "keypadequal",
        _ => return None,
    };
    Some(s.into())
}

// ---------------------------------------------------------------------------
// Start

/// The keymapper part of the bootstrap (input module enabled).
pub fn start() {
    unsafe {
        let keymap = if virtual_gamepad::enabled() { None } else { std::env::var("PEAR_KEYMAP").ok() };
        apply_keymap(keymap.as_deref());
        watch_keymap(keymap);
        install_key_event_silencer();
        // Controller discovery is independent of the keymapper (an app with
        // its own GameController support must see controllers without a
        // keymap); it runs after the keymap so the pad fallback sees bindings.
        controller::install_bridge();
        // Input translation is opt-in: no keymap, no keymapper handlers.
        virtual_gamepad::install();
        if !st().bindings.is_empty() || virtual_gamepad::enabled() {
            install_mouse();
            install_keyboard();
        } else {
            rt::log("[PearRuntime] keymapper disabled (no bindings); input translation off");
        }
    }
}
