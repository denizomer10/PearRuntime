//! GameController discovery, compatibility and the pad → touch keymapper.
//!
//! GameController enumerates HID devices asynchronously: a game that asks for
//! `GCController.controllers` during launch can see an empty list. The bridge
//! starts discovery after launch, keeps the discovered controllers, replays a
//! missed connect to late observers only, and (with a keymap) maps the
//! standard pad to the same virtual joystick/taps the keyboard drives.

use crate::input::keymapper::{self, Binding, Kind, Source};
use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsstring, CGPoint, CGRect, CGSize, Id, Imp, Sel};
use std::cell::UnsafeCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

const NIL: Id = std::ptr::null_mut();

extern "C" {
    static GCControllerDidConnectNotification: Id;
    static GCControllerDidDisconnectNotification: Id;
    fn CFAbsoluteTimeGetCurrent() -> f64;
}

fn now() -> f64 {
    unsafe { CFAbsoluteTimeGetCurrent() }
}

fn is_main() -> bool {
    unsafe { msg0(cls("NSThread"), "isMainThread") }
}

/// Retained Foundation collections guarded by one lock (`@synchronized`).
struct Known {
    controllers: usize,   // NSMutableSet<GCController>
    connect_dates: usize, // NSMutableDictionary<NSValue(nonretained), NSDate>
    app_seen: usize,      // NSHashTable (weak)
}
static KNOWN: Mutex<Known> = Mutex::new(Known { controllers: 0, connect_dates: 0, app_seen: 0 });

static RUNTIME_QUERY_DEPTH: AtomicUsize = AtomicUsize::new(0); // main-thread only
static BOOTSTRAP_TIME: Mutex<f64> = Mutex::new(0.0);
static NATIVE_SUPPORT: AtomicBool = AtomicBool::new(false);
static APP_CONNECT_OBSERVERS: AtomicUsize = AtomicUsize::new(0);
static RUNTIME_REGISTERING: AtomicBool = AtomicBool::new(false);
static TARGETED_REPLAY: AtomicBool = AtomicBool::new(true);
static LAST_APP_QUERY: Mutex<f64> = Mutex::new(0.0);

unsafe fn runtime_controllers() -> Id {
    RUNTIME_QUERY_DEPTH.fetch_add(1, Ordering::AcqRel);
    let c: Id = msg0(cls("GCController"), "controllers");
    RUNTIME_QUERY_DEPTH.fetch_sub(1, Ordering::AcqRel);
    if c.is_null() { msg0(cls("NSArray"), "array") } else { c }
}

fn mark_native_support(reason: &str) {
    if NATIVE_SUPPORT.swap(true, Ordering::AcqRel) {
        return;
    }
    rt::log(&format!("[PearRuntime][input] app drives controllers natively ({reason}); pad->touch fallback off"));
}

unsafe fn mark_seen(controllers: Id) {
    if controllers.is_null() || pear_objc::nsarray_count(controllers) == 0 {
        return;
    }
    let mut k = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if k.app_seen == 0 {
        k.app_seen = pear_objc::objc_retain(msg0(cls("NSHashTable"), "weakObjectsHashTable")) as usize;
    }
    for i in 0..pear_objc::nsarray_count(controllers) {
        msg1::<Id, ()>(k.app_seen as Id, "addObject:", pear_objc::nsarray_object_at(controllers, i));
    }
}

unsafe fn vendor(controller: Id) -> String {
    let v: Id = msg0(controller, "vendorName");
    if v.is_null() { "?".into() } else { rt::ns_string(v) }
}

unsafe fn date_key(controller: Id) -> Id {
    msg1(cls("NSValue"), "valueWithNonretainedObject:", controller)
}

unsafe fn note_known(controller: Id) {
    let k = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    msg1::<Id, ()>(k.controllers as Id, "addObject:", controller);
    msg2::<Id, Id, ()>(k.connect_dates as Id, "setObject:forKey:", msg0::<Id>(cls("NSDate"), "date"), date_key(controller));
}

// ---------------------------------------------------------------------------
// Controller identity: some games accept only the vendor-documented families;
// normalize others to an Xbox identity (the input profile is unchanged).

static ORIGINAL_VENDOR: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_CATEGORY: AtomicUsize = AtomicUsize::new(0);
static LOGGED_KEY: u8 = 0;

fn known_family(name: &str) -> bool {
    let l = name.to_lowercase();
    ["xbox", "dualshock", "dualsense", "playstation", "sony"].iter().any(|k| l.contains(k))
}

unsafe fn original_string(slot: &AtomicUsize, this: Id, selector: &str) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(slot.load(Ordering::Acquire));
    f(this, pear_objc::sel(selector))
}

unsafe extern "C" fn vendor_name(this: Id, _: Sel) -> Id {
    let name = original_string(&ORIGINAL_VENDOR, this, "vendorName");
    let s = if name.is_null() { String::new() } else { rt::ns_string(name) };
    if known_family(&s) {
        return name;
    }
    // Engines read vendorName every frame: log once per controller.
    if pear_objc::assoc_get(this, &LOGGED_KEY).is_null() {
        pear_objc::assoc_set(this, &LOGGED_KEY, pear_objc::nsnumber_bool(true));
        rt::log(&format!("[PearRuntime][input] controller '{}' reported as Xbox Wireless Controller", if name.is_null() { "?" } else { &s }));
    }
    nsstring("Xbox Wireless Controller")
}

unsafe extern "C" fn product_category(this: Id, _: Sel) -> Id {
    let name = original_string(&ORIGINAL_CATEGORY, this, "productCategory");
    if !name.is_null() && known_family(&rt::ns_string(name)) {
        return name;
    }
    nsstring("Xbox One")
}

fn install_controller_compatibility() {
    if rt::env_bool("PEAR_CONTROLLER_NATIVE") {
        return;
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let c = cls("GCController");
        for (selector, imp, slot) in [("vendorName", vendor_name as Imp, &ORIGINAL_VENDOR), ("productCategory", product_category as Imp, &ORIGINAL_CATEGORY)] {
            let m = rt::class_getInstanceMethod(c, pear_objc::sel(selector));
            if !m.is_null() {
                slot.store(rt::method_getImplementation(m) as usize, Ordering::Release);
                rt::method_setImplementation(m, imp);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Pad → touch keymapper (touch-only games). The app's own valueChangedHandler
// has priority: when it installs one, translation stops.

#[derive(Default)]
struct Pad {
    force: bool,
    enabled: bool,
    has_controller_bindings: bool,
    current: usize, // GCController (unretained, as the original's strong ref is cleared on disconnect)
    controller_joystick: Option<Rc<Binding>>,
    look_binding: Option<Rc<Binding>>,
    pad_keys: [bool; 10], // W A S D attack skill interact menu sprint burst
    look_active: bool,
    look_point: CGPoint,
    look_generation: u64,
    pressed: HashSet<String>,
}

struct MainCell(UnsafeCell<Option<Pad>>);
unsafe impl Sync for MainCell {}
static PAD: MainCell = MainCell(UnsafeCell::new(None));

fn pad() -> &'static mut Pad {
    unsafe { (*PAD.0.get()).get_or_insert_with(|| Pad { look_point: CGPoint { x: 0.75, y: 0.5 }, ..Default::default() }) }
}

static ORIGINAL_SET_GAMEPAD: AtomicUsize = AtomicUsize::new(0);
static COMBINED_GAMEPAD: AtomicUsize = AtomicUsize::new(0);
static GAME_HANDLER_KEY: u8 = 0;
const LOOK_KEY: &str = "f:controllerlook";

type GamepadBlock = unsafe extern "C" fn(Id, Id, Id);

/// The game's own handler, stored per gamepad.
unsafe fn game_handler(gamepad: Id) -> Id {
    if gamepad.is_null() { NIL } else { pear_objc::assoc_get(gamepad, &GAME_HANDLER_KEY) }
}

fn finger(down: bool, key: &str, x: f64, y: f64) {
    unsafe { pear_objc::autorelease_pool(|| crate::input::touch_synth::synthesize_virtual_finger(down, nsstring(key), x, y)) };
}

fn look_sensitivity(p: &Pad) -> f64 {
    match &p.look_binding {
        Some(b) if b.sensitivity > 0.0 => b.sensitivity,
        _ => 1.0,
    }
}

fn look_region(p: &Pad) -> CGRect {
    match &p.look_binding {
        Some(b) => b.region,
        None => CGRect { origin: CGPoint { x: 0.55, y: 0.18 }, size: CGSize { width: 0.40, height: 0.64 } },
    }
}

fn look_step(dx: f64, dy: f64) {
    if dx.abs() < 0.00001 && dy.abs() < 0.00001 {
        return;
    }
    let p = pad();
    let r = look_region(p);
    let center = CGPoint { x: r.origin.x + r.size.width / 2.0, y: r.origin.y + r.size.height / 2.0 };
    if !p.look_active {
        p.look_active = true;
        p.look_point = center;
        finger(true, LOOK_KEY, center.x, center.y);
    }
    let s = look_sensitivity(p);
    let (min_x, min_y, max_x, max_y) = (r.origin.x, r.origin.y, r.origin.x + r.size.width, r.origin.y + r.size.height);
    let mut nx = p.look_point.x + dx * s;
    let mut ny = p.look_point.y + dy * s;
    if nx < min_x || nx > max_x || ny < min_y || ny > max_y {
        finger(false, LOOK_KEY, p.look_point.x, p.look_point.y);
        p.look_point = center;
        finger(true, LOOK_KEY, center.x, center.y);
        nx = (center.x + dx * s).max(min_x).min(max_x);
        ny = (center.y + dy * s).max(min_y).min(max_y);
    }
    p.look_point = CGPoint { x: nx, y: ny };
    finger(true, LOOK_KEY, nx, ny);
    p.look_generation += 1;
    let generation = p.look_generation;
    rt::after_main(0.28, move || {
        let p = pad();
        if generation != p.look_generation || !p.look_active {
            return;
        }
        p.look_active = false;
        finger(false, LOOK_KEY, p.look_point.x, p.look_point.y);
    });
}

unsafe fn axis(stick: Id, name: &str) -> f32 {
    if stick.is_null() { 0.0 } else { msg0(msg0::<Id>(stick, name), "value") }
}

fn start_look_timer() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        rt::main_ticker(1.0 / 60.0, 1.0 / 60.0, 1.0 / 240.0, || unsafe {
            let p = pad();
            if !p.enabled || p.current == 0 {
                return false;
            }
            let gamepad: Id = msg0(p.current as Id, "extendedGamepad");
            if gamepad.is_null() || !game_handler(gamepad).is_null() {
                return false;
            }
            if NATIVE_SUPPORT.load(Ordering::Acquire) && p.look_binding.is_none() && !p.force {
                return false;
            }
            // Without an explicit right-stick binding the default right-hand
            // region applies, so one button binding does not disable the camera.
            if msg0::<isize>(crate::core::runtime::ui_app(), "applicationState") != 0 {
                return false;
            }
            let stick: Id = msg0(gamepad, "rightThumbstick");
            let (x, y) = (axis(stick, "xAxis"), axis(stick, "yAxis"));
            let magnitude = (x * x + y * y).sqrt();
            const DEADZONE: f32 = 0.12;
            if magnitude <= DEADZONE {
                return false;
            }
            let scaled = ((magnitude - DEADZONE) / (1.0 - DEADZONE)).powf(1.35);
            // 1.5 at full deflection: a gamepad-like rotation speed.
            let speed = 1.5f32 / 60.0;
            look_step(((x / magnitude) * scaled * speed) as f64, (-(y / magnitude) * scaled * speed) as f64);
            false
        });
    });
}

unsafe fn pressed(element: Id) -> bool {
    !element.is_null() && msg0::<bool>(element, "isPressed")
}

unsafe fn button_state(gamepad: Id, key: &str) -> bool {
    let e = |n: &str| -> Id { msg0(gamepad, n) };
    let dpad = |n: &str| -> bool { pressed(msg0(e("dpad"), n)) };
    match key {
        "buttonA" | "buttonB" | "buttonX" | "buttonY" | "leftShoulder" | "rightShoulder" => pressed(e(key)),
        "leftTrigger" | "rightTrigger" => {
            let t = e(key);
            !t.is_null() && msg0::<f32>(t, "value") > 0.5
        }
        "dpadUp" => dpad("up"),
        "dpadDown" => dpad("down"),
        "dpadLeft" => dpad("left"),
        "dpadRight" => dpad("right"),
        "menu" => pressed(e("buttonMenu")),
        "options" => pear_objc::responds_to(gamepad, "buttonOptions") && pressed(e("buttonOptions")),
        _ => false,
    }
}

fn controller_key_changed(key: &str, down: bool) {
    let p = pad();
    if p.pressed.contains(key) == down {
        return;
    }
    if down {
        p.pressed.insert(key.to_string());
    } else {
        p.pressed.remove(key);
    }
    let Some(b) = keymapper::binding_for(key, Source::Controller) else { return };
    if matches!(b.kind, Kind::Joystick | Kind::MouseLook) {
        return;
    }
    keymapper::synthesize_touch(&b, down, b.point);
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!("[PearRuntime][controller] {key} {}", if down { "down" } else { "up" }));
    }
}

unsafe fn translate_controller_bindings(gamepad: Id) {
    for key in ["buttonA", "buttonB", "buttonX", "buttonY", "leftShoulder", "rightShoulder", "leftTrigger", "rightTrigger", "dpadUp", "dpadDown", "dpadLeft", "dpadRight", "menu", "options"] {
        controller_key_changed(key, button_state(gamepad, key));
    }
    let joystick = pad().controller_joystick.clone().or_else(|| keymapper::st().joystick_binding.clone());
    if let Some(j) = joystick {
        let stick: Id = msg0(gamepad, "leftThumbstick");
        let (mut x, mut y) = (axis(stick, "xAxis"), axis(stick, "yAxis"));
        if x.abs() < 0.12 && y.abs() < 0.12 {
            x = 0.0;
            y = 0.0;
        }
        keymapper::update_joystick_with_binding(Some(&j), x as f64, -y as f64);
    }
}

fn pad_set_key(code: isize, down: bool, slot: usize) {
    let p = pad();
    if p.pad_keys[slot] == down {
        return;
    }
    p.pad_keys[slot] = down;
    keymapper::handle_key_press(code, down);
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::log(&format!("[PearRuntime][controller] key {code} {}", if down { "down" } else { "up" }));
    }
}

unsafe fn translate_gamepad(gamepad: Id) {
    let p = pad();
    if !p.enabled || !game_handler(gamepad).is_null() {
        return;
    }
    // A game with its own controller path polls the pad; synthesized touches
    // on top make it flip back to touchscreen every frame.
    if NATIVE_SUPPORT.load(Ordering::Acquire) && !p.has_controller_bindings && !p.force {
        return;
    }
    if p.has_controller_bindings {
        translate_controller_bindings(gamepad);
        return;
    }
    // Keymaps predating controller bindings: the pad drives the keyboard bindings.
    const DEADZONE: f32 = 0.35;
    let stick: Id = msg0(gamepad, "leftThumbstick");
    let (x, y) = (axis(stick, "xAxis"), axis(stick, "yAxis"));
    let b = |n: &str| pressed(msg0(gamepad, n));
    pad_set_key(26, y > DEADZONE, 0); // W
    pad_set_key(22, y < -DEADZONE, 2); // S
    pad_set_key(4, x < -DEADZONE, 1); // A
    pad_set_key(7, x > DEADZONE, 3); // D
    pad_set_key(44, b("buttonA"), 4); // space
    pad_set_key(41, b("buttonB"), 7); // escape
    pad_set_key(8, b("buttonX"), 5); // E
    pad_set_key(9, b("buttonY"), 6); // F
    pad_set_key(225, b("leftShoulder"), 8); // left shift
    pad_set_key(20, b("rightShoulder"), 9); // Q
}

unsafe extern "C" fn combined_gamepad(_block: Id, gamepad: Id, element: Id) {
    let game = game_handler(gamepad);
    if !game.is_null() {
        if let Some(f) = rt::block_invoke::<GamepadBlock>(game) {
            f(game, gamepad, element);
        }
        return;
    }
    translate_gamepad(gamepad);
}

unsafe extern "C" fn set_gamepad_changed(this: Id, cmd: Sel, handler: Id) {
    let o = ORIGINAL_SET_GAMEPAD.load(Ordering::Acquire);
    if o == 0 {
        return;
    }
    let set: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(o);
    let combined = COMBINED_GAMEPAD.load(Ordering::Acquire) as Id;
    if handler == combined {
        set(this, cmd, handler);
        return;
    }
    pear_objc::assoc_set_copy(this, &GAME_HANDLER_KEY, handler);
    if !handler.is_null() {
        mark_native_support("gamepad valueChangedHandler");
    }
    set(this, cmd, combined);
}

unsafe fn pad_attach(controller: Id) {
    let p = pad();
    if !p.enabled {
        return;
    }
    let gamepad: Id = msg0(controller, "extendedGamepad");
    if gamepad.is_null() {
        return;
    }
    p.current = controller as usize;
    let combined = COMBINED_GAMEPAD.load(Ordering::Acquire) as Id;
    let o = ORIGINAL_SET_GAMEPAD.load(Ordering::Acquire);
    if o != 0 {
        let set: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(o);
        set(gamepad, pear_objc::sel("setValueChangedHandler:"), combined);
    } else {
        msg1::<Id, ()>(gamepad, "setValueChangedHandler:", combined);
    }
}

unsafe fn install_gamepad_keymapper() {
    let setting = std::env::var("PEAR_CONTROLLER_KEYMAP").ok();
    let p = pad();
    p.force = setting.as_deref().is_some_and(|s| s.to_lowercase() == "force");
    let setting_on = setting.as_deref().map(|_| rt::env_bool("PEAR_CONTROLLER_KEYMAP")).unwrap_or(true);
    let bindings = keymapper::st().bindings.clone();
    p.enabled = !bindings.is_empty() && (setting_on || p.force);
    p.has_controller_bindings = false;
    p.controller_joystick = None;
    p.look_binding = None;
    for b in bindings.iter().filter(|b| b.source == Source::Controller) {
        p.has_controller_bindings = true;
        if b.kind == Kind::Joystick && b.key == "leftStick" && p.controller_joystick.is_none() {
            p.controller_joystick = Some(b.clone());
        }
        if b.kind == Kind::MouseLook && b.key == "rightStick" && p.look_binding.is_none() {
            p.look_binding = Some(b.clone());
        }
    }
    if !p.enabled {
        rt::log("[PearRuntime][input] gamepad keymapper off (no bindings or opted out)");
        return;
    }
    if COMBINED_GAMEPAD.load(Ordering::Acquire) == 0 {
        COMBINED_GAMEPAD.store(rt::global_block(combined_gamepad as *const c_void) as usize, Ordering::Release);
    }
    let c = rt::class_named("GCExtendedGamepad");
    let m = if c.is_null() { std::ptr::null_mut() } else { rt::class_getInstanceMethod(c, pear_objc::sel("setValueChangedHandler:")) };
    if !m.is_null() {
        ORIGINAL_SET_GAMEPAD.store(rt::method_setImplementation(m, set_gamepad_changed as Imp) as usize, Ordering::Release);
    }
    start_look_timer();
    let attached = runtime_controllers();
    for i in 0..pear_objc::nsarray_count(attached) {
        pad_attach(pear_objc::nsarray_object_at(attached, i));
    }
    let p = pad();
    rt::log(&format!(
        "[PearRuntime][input] gamepad keymapper installed (controllers={}, custom={}, look={})",
        pear_objc::nsarray_count(attached),
        p.has_controller_bindings as i32,
        p.look_binding.is_some() as i32
    ));
}

// ---------------------------------------------------------------------------
// Targeted connect replay: deliver a missed connect only to an observer that
// registered after the pad connected, and only when the app did not enumerate
// the list in the same main-thread turn (Unity/Unreal would get a second
// player slot from a duplicate connect).

unsafe fn note_app_connect_observer(deliver: Box<dyn Fn(Id)>) {
    APP_CONNECT_OBSERVERS.fetch_add(1, Ordering::AcqRel);
    mark_native_support("observes controller connects");
    if !TARGETED_REPLAY.load(Ordering::Acquire) {
        return;
    }
    let registered_at = now();
    let mut missed: Vec<usize> = Vec::new();
    {
        let k = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
        if k.controllers == 0 {
            return;
        }
        let all: Id = msg0(k.controllers as Id, "allObjects");
        for i in 0..pear_objc::nsarray_count(all) {
            let c = pear_objc::nsarray_object_at(all, i);
            let connected: Id = msg1(k.connect_dates as Id, "objectForKey:", date_key(c));
            if !connected.is_null() && msg0::<f64>(connected, "timeIntervalSinceReferenceDate") <= registered_at {
                missed.push(pear_objc::objc_retain(c) as usize);
            }
        }
    }
    if missed.is_empty() {
        return;
    }
    rt::async_main(move || unsafe {
        pear_objc::autorelease_pool(|| {
            if *LAST_APP_QUERY.lock().unwrap_or_else(|e| e.into_inner()) >= registered_at - 0.05 {
                rt::log("[PearRuntime][input] observer enumerated controllers itself; no replay");
            } else {
                let ids: Vec<Id> = missed.iter().map(|c| *c as Id).collect();
                mark_seen(pear_objc::nsarray_from_ids(&ids));
                for c in &ids {
                    rt::log(&format!("[PearRuntime][input] delivering missed controller connect to late observer: {}", vendor(*c)));
                    let note: Id = msg2(cls("NSNotification"), "notificationWithName:object:", GCControllerDidConnectNotification, *c);
                    deliver(note);
                }
            }
            for c in &missed {
                pear_objc::objc_release(*c as Id);
            }
        })
    });
}

static ORIGINAL_ADD_SELECTOR: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_ADD_BLOCK: AtomicUsize = AtomicUsize::new(0);

unsafe fn is_connect(name: Id) -> bool {
    !name.is_null() && msg1::<Id, bool>(name, "isEqualToString:", GCControllerDidConnectNotification)
}

/// A weak reference that outlives the hook (for the deferred replay).
struct Weak(Box<UnsafeCell<Id>>);
impl Weak {
    unsafe fn new(obj: Id) -> Weak {
        let w = Weak(Box::new(UnsafeCell::new(NIL)));
        pear_objc::objc_storeWeak(w.0.get(), obj);
        w
    }
    unsafe fn load(&self) -> Id {
        let o = pear_objc::objc_loadWeakRetained(self.0.get());
        if o.is_null() { o } else { rt::autoreleased(o) }
    }
}
impl Drop for Weak {
    fn drop(&mut self) {
        unsafe { pear_objc::objc_destroyWeak(self.0.get()) };
    }
}

unsafe extern "C" fn add_observer_selector(this: Id, cmd: Sel, observer: Id, selector: Sel, name: Id, object: Id) {
    let f: unsafe extern "C" fn(Id, Sel, Id, Sel, Id, Id) = std::mem::transmute(ORIGINAL_ADD_SELECTOR.load(Ordering::Acquire));
    f(this, cmd, observer, selector, name, object);
    if RUNTIME_REGISTERING.load(Ordering::Acquire) || !object.is_null() || observer.is_null() || !is_connect(name) {
        return;
    }
    if rt::env_set("PEAR_DEBUG_GC") {
        rt::log(&format!(
            "[PearRuntime][input] app connect observer {} -[{} {}]",
            rt::description(observer),
            pear_objc::class_name(observer),
            rt::cstr(rt::sel_getName(selector))
        ));
    }
    let weak = Weak::new(observer);
    let selector = selector as usize;
    note_app_connect_observer(Box::new(move |note| unsafe {
        let strong = weak.load();
        if !strong.is_null() && msg1::<Sel, bool>(strong, "respondsToSelector:", selector as Sel) {
            let send: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(pear_objc::objc_msgSend as *const ());
            send(strong, selector as Sel, note);
        }
    }));
}

extern "C" {
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> i32;
}
#[repr(C)]
struct DlInfo {
    fname: *const std::ffi::c_char,
    fbase: *mut c_void,
    sname: *const std::ffi::c_char,
    saddr: *mut c_void,
}

unsafe extern "C" fn add_observer_block(this: Id, cmd: Sel, name: Id, object: Id, queue: Id, block: Id) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, Id, Id, Id, Id) -> Id = std::mem::transmute(ORIGINAL_ADD_BLOCK.load(Ordering::Acquire));
    let token = f(this, cmd, name, object, queue, block);
    if RUNTIME_REGISTERING.load(Ordering::Acquire) || !object.is_null() || block.is_null() || !is_connect(name) {
        return token;
    }
    let copied = rt::copy_block(block) as usize;
    if rt::env_set("PEAR_DEBUG_GC") {
        let invoke = *((copied as *const u8).add(16) as *const *const c_void);
        let mut info = DlInfo { fname: std::ptr::null(), fbase: NIL, sname: std::ptr::null(), saddr: NIL };
        dladdr(invoke, &mut info);
        let file = if info.fname.is_null() { "?".to_string() } else { rt::cstr(info.fname).rsplit('/').next().unwrap_or("?").to_string() };
        rt::log(&format!(
            "[PearRuntime][input] app connect block observer from {file} ({})",
            if info.sname.is_null() { "?" } else { rt::cstr(info.sname) }
        ));
    }
    let weak_token = Weak::new(token);
    let queue = if queue.is_null() { 0 } else { pear_objc::objc_retain(queue) as usize };
    note_app_connect_observer(Box::new(move |note| unsafe {
        if weak_token.load().is_null() {
            return; // the app already removed its observer
        }
        let call = move |note: Id| unsafe {
            if let Some(f) = rt::block_invoke::<unsafe extern "C" fn(Id, Id)>(copied as Id) {
                f(copied as Id, note);
            }
        };
        let current: Id = msg0(cls("NSOperationQueue"), "currentQueue");
        if queue != 0 && queue as Id != current {
            let note = pear_objc::objc_retain(note) as usize;
            let op = rt::block0(move || {
                call(note as Id);
                unsafe { pear_objc::objc_release(note as Id) };
            });
            msg1::<Id, ()>(queue as Id, "addOperationWithBlock:", op);
            rt::release_block(op);
        } else {
            call(note);
        }
    }));
    token
}

fn install_observer_tracking() {
    unsafe {
        TARGETED_REPLAY.store(std::env::var("PEAR_CONTROLLER_REPLAY").is_err() || rt::env_bool("PEAR_CONTROLLER_REPLAY"), Ordering::Release);
        let c = cls("NSNotificationCenter");
        for (selector, imp, slot) in [
            ("addObserver:selector:name:object:", add_observer_selector as Imp, &ORIGINAL_ADD_SELECTOR),
            ("addObserverForName:object:queue:usingBlock:", add_observer_block as Imp, &ORIGINAL_ADD_BLOCK),
        ] {
            let m = rt::class_getInstanceMethod(c, pear_objc::sel(selector));
            if !m.is_null() {
                slot.store(rt::method_setImplementation(m, imp) as usize, Ordering::Release);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The bridge. GameController must not be touched before the app finished
// launching: an early `+controllers` query starts the Catalyst controller
// session before UIKit is up, and that session never reports a pad that was
// already connected. Only NotificationCenter tracking runs at load time.

static BRIDGE_TOKEN: AtomicUsize = AtomicUsize::new(0);

unsafe fn take_token_and_install() {
    let token = BRIDGE_TOKEN.swap(0, Ordering::AcqRel);
    if token == 0 {
        return;
    }
    let center: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg1::<Id, ()>(center, "removeObserver:", token as Id);
    pear_objc::objc_release(token as Id);
    install_bridge_now();
}

pub fn install_bridge() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        {
            let mut k = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
            k.controllers = pear_objc::objc_retain(msg0(cls("NSMutableSet"), "set")) as usize;
            k.connect_dates = pear_objc::objc_retain(msg0(cls("NSMutableDictionary"), "dictionary")) as usize;
        }
        *BOOTSTRAP_TIME.lock().unwrap_or_else(|e| e.into_inner()) = now();
        install_observer_tracking();
        let token = rt::observe_main("UIApplicationDidFinishLaunchingNotification", |_| unsafe { take_token_and_install() });
        BRIDGE_TOKEN.store(pear_objc::objc_retain(token) as usize, Ordering::Release);
        // A runtime loaded into an app that already launched must not wait for
        // a notification that already happened.
        rt::async_main(|| unsafe {
            let app = crate::core::runtime::ui_app();
            let scenes: Id = msg0(app, "connectedScenes");
            if !msg0::<Id>(app, "delegate").is_null()
                && msg0::<isize>(app, "applicationState") != 2
                && BRIDGE_TOKEN.load(Ordering::Acquire) != 0
                && msg0::<usize>(scenes, "count") > 0
            {
                take_token_and_install();
            }
        });
    });
}

static ORIGINAL_CONTROLLERS: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_CURRENT: AtomicUsize = AtomicUsize::new(0);
static ALLOW_WAIT: AtomicBool = AtomicBool::new(true);
static WAITED: AtomicBool = AtomicBool::new(false);
static DEBUG_QUERIES: AtomicUsize = AtomicUsize::new(0);

unsafe fn original_controllers(c: Id) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(ORIGINAL_CONTROLLERS.load(Ordering::Acquire));
    f(c, pear_objc::sel("controllers"))
}

/// During the discovery window only, the first empty main-thread query pumps
/// the run loop until discovery settles; later queries never block. Every app
/// query records which pads the app knows.
unsafe extern "C" fn controllers_hook(c: Id, _: Sel) -> Id {
    let count = |r: Id| if r.is_null() { 0 } else { pear_objc::nsarray_count(r) };
    // PEAR_DEBUG_GC: every underlying query is logged (the first 20), the
    // runtime's own included, like the separate debug hook it replaces.
    let original = |c: Id| -> Id {
        let r = original_controllers(c);
        if rt::env_set("PEAR_DEBUG_GC") && DEBUG_QUERIES.fetch_add(1, Ordering::AcqRel) < 20 {
            rt::log(&format!("[PearRuntime][input] app queried controllers -> {}", count(r)));
        }
        r
    };
    let mut result = original(c);
    if is_main() && RUNTIME_QUERY_DEPTH.load(Ordering::Acquire) > 0 {
        return result;
    }
    if count(result) == 0 && ALLOW_WAIT.load(Ordering::Acquire) && is_main() && !WAITED.swap(true, Ordering::AcqRel) {
        let window_end = *BOOTSTRAP_TIME.lock().unwrap_or_else(|e| e.into_inner()) + 1.5;
        let run_loop: Id = msg0(cls("NSRunLoop"), "currentRunLoop");
        let mode: Id = nsstring("kCFRunLoopDefaultMode");
        while count(result) == 0 && now() < window_end {
            let date: Id = msg1(cls("NSDate"), "dateWithTimeIntervalSinceNow:", 0.05f64);
            let _: bool = msg2(run_loop, "runMode:beforeDate:", mode, date);
            result = original(c);
        }
        rt::log(&format!("[PearRuntime][input] first controller query settled -> {}", count(result)));
    }
    mark_native_support("queried controllers");
    mark_seen(result);
    if is_main() {
        *LAST_APP_QUERY.lock().unwrap_or_else(|e| e.into_inner()) = now();
    }
    result
}

unsafe extern "C" fn current_hook(c: Id, _: Sel) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(ORIGINAL_CURRENT.load(Ordering::Acquire));
    let current = f(c, pear_objc::sel("current"));
    if !current.is_null() && !(is_main() && RUNTIME_QUERY_DEPTH.load(Ordering::Acquire) > 0) {
        mark_native_support("queried current controller");
        mark_seen(pear_objc::nsarray_from_ids(&[current]));
        if is_main() {
            *LAST_APP_QUERY.lock().unwrap_or_else(|e| e.into_inner()) = now();
        }
    }
    current
}

static ORIGINAL_MHY_INSTANCE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_MHY_INIT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn mhy_instance(this: Id, cmd: Sel) -> Id {
    rt::log("[PearRuntime][input] MHYGameController instance requested");
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(ORIGINAL_MHY_INSTANCE.load(Ordering::Acquire));
    f(this, cmd)
}

unsafe extern "C" fn mhy_init(this: Id, cmd: Sel) -> Id {
    rt::log("[PearRuntime][input] MHYGameController init called");
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(ORIGINAL_MHY_INIT.load(Ordering::Acquire));
    f(this, cmd)
}

unsafe fn install_bridge_now() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        // The first-query discovery window starts now, not at load time.
        *BOOTSTRAP_TIME.lock().unwrap_or_else(|e| e.into_inner()) = now();
        RUNTIME_REGISTERING.store(true, Ordering::Release);
        rt::observe_main(&rt::ns_string(GCControllerDidConnectNotification), |note| unsafe {
            let controller: Id = msg0(note, "object");
            if controller.is_null() || !pear_objc::is_kind_of(controller, "GCController") {
                return;
            }
            note_known(controller);
            // The app's own observers receive this real notification as well.
            if APP_CONNECT_OBSERVERS.load(Ordering::Acquire) > 0 {
                mark_seen(pear_objc::nsarray_from_ids(&[controller]));
            }
            rt::log(&format!(
                "[PearRuntime][input] game controller connected: {} extended={} micro={}",
                vendor(controller),
                !msg0::<Id>(controller, "extendedGamepad").is_null() as i32,
                !msg0::<Id>(controller, "microGamepad").is_null() as i32
            ));
            pad_attach(controller);
        });
        rt::observe_main(&rt::ns_string(GCControllerDidDisconnectNotification), |note| unsafe {
            let controller: Id = msg0(note, "object");
            if controller.is_null() || !pear_objc::is_kind_of(controller, "GCController") {
                return;
            }
            {
                let k = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
                msg1::<Id, ()>(k.controllers as Id, "removeObject:", controller);
                msg1::<Id, ()>(k.connect_dates as Id, "removeObjectForKey:", date_key(controller));
            }
            rt::log(&format!("[PearRuntime][input] game controller disconnected: {}", vendor(controller)));
            let p = pad();
            if p.current == controller as usize {
                p.current = 0;
            }
        });
        RUNTIME_REGISTERING.store(false, Ordering::Release);

        let existing = runtime_controllers();
        for i in 0..pear_objc::nsarray_count(existing) {
            note_known(pear_objc::nsarray_object_at(existing, i));
        }
        rt::log(&format!("[PearRuntime][input] game controllers after launch: {}", pear_objc::nsarray_count(existing)));
        install_controller_compatibility();
        for i in 0..pear_objc::nsarray_count(existing) {
            let c = pear_objc::nsarray_object_at(existing, i);
            rt::log(&format!(
                "[PearRuntime][input] controller present: {} extended={} micro={}",
                vendor(c),
                !msg0::<Id>(c, "extendedGamepad").is_null() as i32,
                !msg0::<Id>(c, "microGamepad").is_null() as i32
            ));
        }
        let gc = cls("GCController");
        let m = rt::class_getClassMethod(gc, pear_objc::sel("controllers"));
        if !m.is_null() {
            ALLOW_WAIT.store(!rt::env_bool("PEAR_CONTROLLER_NATIVE"), Ordering::Release);
            ORIGINAL_CONTROLLERS.store(rt::method_getImplementation(m) as usize, Ordering::Release);
            rt::method_setImplementation(m, controllers_hook as Imp);
        }
        let m = rt::class_getClassMethod(gc, pear_objc::sel("current"));
        if !m.is_null() {
            ORIGINAL_CURRENT.store(rt::method_getImplementation(m) as usize, Ordering::Release);
            rt::method_setImplementation(m, current_hook as Imp);
        }
        // Opt-in diagnostics for apps with their own gamepad layer.
        if rt::env_set("PEAR_DEBUG_GC") {
            let g = rt::class_named("MHYGameController");
            rt::log(&format!("[PearRuntime][input] MHYGameController class={}", if g.is_null() { "nil" } else { rt::class_name(g) }));
            if !g.is_null() {
                let m = rt::class_getClassMethod(g, pear_objc::sel("instance"));
                if !m.is_null() {
                    ORIGINAL_MHY_INSTANCE.store(rt::method_getImplementation(m) as usize, Ordering::Release);
                    rt::method_setImplementation(m, mhy_instance as Imp);
                }
                let m = rt::class_getInstanceMethod(g, pear_objc::sel("init"));
                if !m.is_null() {
                    ORIGINAL_MHY_INIT.store(rt::method_getImplementation(m) as usize, Ordering::Release);
                    rt::method_setImplementation(m, mhy_init as Imp);
                }
            }
        }
        // Legacy broadcast replay (opt-in): duplicates the connect for engines
        // that enumerate pads at launch.
        if std::env::var("PEAR_CONTROLLER_REPLAY").is_ok_and(|v| v.to_lowercase() == "broadcast") {
            rt::after_main(3.0, || unsafe {
                pear_objc::autorelease_pool(|| {
                    let all = runtime_controllers();
                    let center: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
                    for i in 0..pear_objc::nsarray_count(all) {
                        let c = pear_objc::nsarray_object_at(all, i);
                        rt::log(&format!("[PearRuntime][input] broadcast replay of controller connect: {}", vendor(c)));
                        msg2::<Id, Id, ()>(center, "postNotificationName:object:", GCControllerDidConnectNotification, c);
                    }
                })
            });
        }
        install_gamepad_keymapper();
    });
}
