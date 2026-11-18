//! Original PearSilicon input adapter using public GameController APIs.
//! Opt-in only (PEAR_VIRTUAL_GAMEPAD). No touch synthesis.
//!
//! The game sees a hidden GCVirtualController. Keyboard/mouse events map to
//! controller elements from PEAR_VIRTUAL_GAMEPAD_MAP (built by the app), so
//! nothing depends on a particular keyboard layout or game.

use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsstring, CGPoint, Id, Sel};
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};

const NIL: Id = std::ptr::null_mut();

extern "C" {
    fn CGAssociateMouseAndMouseCursorPosition(connected: u32) -> i32;
    fn CGDisplayHideCursor(display: u32) -> i32;
    fn CGDisplayShowCursor(display: u32) -> i32;
    fn CGMainDisplayID() -> u32;
    fn atexit(f: extern "C" fn()) -> i32;
    static GCInputButtonA: Id;
    static GCInputButtonB: Id;
    static GCInputButtonX: Id;
    static GCInputButtonY: Id;
    static GCInputLeftShoulder: Id;
    static GCInputRightShoulder: Id;
    static GCInputLeftTrigger: Id;
    static GCInputRightTrigger: Id;
    static GCInputButtonMenu: Id;
    static GCInputButtonOptions: Id;
    static GCInputLeftThumbstickButton: Id;
    static GCInputRightThumbstickButton: Id;
    static GCInputDirectionPad: Id;
    static GCInputLeftThumbstick: Id;
    static GCInputRightThumbstick: Id;
}

/// User intent survives focus changes: losing focus suspends capture, Escape
/// only releases, and a click re-arms.
#[derive(Default)]
struct Capture {
    requested: bool,
    suspended: bool,
    escape_down: bool,
}
impl Capture {
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
struct Pad {
    pad: usize, // retained GCVirtualController
    ready: bool,
    elements: HashSet<usize>, // configured element names (NSString pointers)
    warned: HashSet<usize>,
    capture: Capture,
    pressed_keys: HashSet<String>,
    pressed_mouse: HashSet<String>,
    monitor: bool,
    last_capture_state: i64,
    captured: bool,
    click_passthrough: bool,
    cursor_disassociated: bool,
    cursor_hidden: bool,
    cursor_api_failed: bool,
    mouse_x: f32,
    mouse_y: f32,
    sensitivity: f64,
    invert_y: bool,
    last_mouse_time: f64,
    last_gc_mouse_time: f64,
    mouse_received: u64,
    mouse_accepted: u64,
    mouse_event_samples: u64,
    last_mouse_diagnostic: f64,
    key_to_control: HashMap<String, Vec<String>>,
    mouse_to_control: HashMap<String, Vec<String>>,
    stick: HashMap<String, String>,
}

/// Main-queue-only state (every entry point hops to the main queue).
struct MainCell(UnsafeCell<Option<Pad>>);
unsafe impl Sync for MainCell {}
static STATE: MainCell = MainCell(UnsafeCell::new(None));

fn st() -> &'static mut Pad {
    unsafe {
        let s = &mut *STATE.0.get();
        s.get_or_insert_with(|| Pad { last_capture_state: -1, sensitivity: 1.0, capture: Capture { requested: true, ..Default::default() }, ..Default::default() })
    }
}

#[export_name = "_Z25PearVirtualGamepadEnabledv"]
pub extern "C" fn enabled() -> bool {
    rt::env_bool("PEAR_VIRTUAL_GAMEPAD")
}

unsafe fn foreground() -> bool {
    let c = rt::class_named("NSApplication");
    let app: Id = if !c.is_null() && pear_objc::responds_to(c, "sharedApplication") { msg0(c, "sharedApplication") } else { NIL };
    let active = !app.is_null() && pear_objc::responds_to(app, "isActive") && msg0::<bool>(app, "isActive");
    if !active {
        return false;
    }
    let ui: Id = msg0(cls("UIApplication"), "sharedApplication");
    ui.is_null() || msg0::<isize>(ui, "applicationState") == 0
}

// ---------------------------------------------------------------------------
// Mapping

const KNOWN: [&str; 16] = ["a", "b", "x", "y", "l1", "r1", "l2", "r2", "dpadUp", "dpadDown", "dpadLeft", "dpadRight", "menu", "options", "l3", "r3"];

fn is_dpad(control: &str) -> bool {
    matches!(control, "dpadUp" | "dpadDown" | "dpadLeft" | "dpadRight")
}

/// Accepts old single-control profiles and multi-control lists (deduplicated, in order).
unsafe fn controls(value: Id) -> Vec<String> {
    let items: Vec<Id> = if value.is_null() {
        vec![]
    } else if pear_objc::is_kind_of(value, "NSString") {
        vec![value]
    } else if pear_objc::is_kind_of(value, "NSArray") {
        (0..pear_objc::nsarray_count(value)).map(|i| pear_objc::nsarray_object_at(value, i)).collect()
    } else {
        vec![]
    };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if !pear_objc::is_kind_of(item, "NSString") {
            continue;
        }
        let s = rt::ns_string(item);
        if KNOWN.contains(&s.as_str()) && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

unsafe fn lower(s: Id) -> String {
    rt::ns_string(msg0(s, "lowercaseString"))
}

unsafe fn dict_entries(d: Id) -> Vec<(Id, Id)> {
    let keys: Id = msg0(d, "allKeys");
    (0..pear_objc::nsarray_count(keys))
        .map(|i| {
            let k = pear_objc::nsarray_object_at(keys, i);
            (k, msg1(d, "objectForKey:", k))
        })
        .collect()
}

unsafe fn load_mapping(p: &mut Pad) {
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
        let parsed: Id = if data.is_null() { NIL } else { pear_objc::msg3(cls("NSJSONSerialization"), "JSONObjectWithData:options:error:", data, 0usize, &mut error as *mut Id) };
        if !parsed.is_null() && pear_objc::is_kind_of(parsed, "NSDictionary") {
            root = parsed;
        } else {
            let reason = if error.is_null() { "not JSON".to_string() } else { rt::ns_string(msg0(error, "localizedDescription")) };
            rt::log(&format!("[PearRuntime][virtual-pad] invalid PEAR_VIRTUAL_GAMEPAD_MAP: {reason}"));
        }
    }
    if !root.is_null() {
        let get = |k: &str| -> Id { msg1(root, "objectForKey:", nsstring(k)) };
        let buttons = get("buttons");
        if !buttons.is_null() && pear_objc::is_kind_of(buttons, "NSDictionary") {
            for (k, v) in dict_entries(buttons) {
                if pear_objc::is_kind_of(k, "NSString") && msg0::<usize>(k, "length") > 0 {
                    keys.insert(lower(k), controls(v));
                }
            }
        }
        let mouse_buttons = get("mouseButtons");
        if !mouse_buttons.is_null() && pear_objc::is_kind_of(mouse_buttons, "NSDictionary") {
            let non_digits: Id = msg0(msg0::<Id>(cls("NSCharacterSet"), "decimalDigitCharacterSet"), "invertedSet");
            for (k, v) in dict_entries(mouse_buttons) {
                if !pear_objc::is_kind_of(k, "NSString") || msg0::<usize>(k, "length") == 0 {
                    continue;
                }
                let r: pear_objc::NSRange = msg1(k, "rangeOfCharacterFromSet:", non_digits);
                if r.location == isize::MAX as usize {
                    mouse.insert(msg0::<isize>(k, "integerValue").to_string(), controls(v));
                }
            }
        }
        let left = get("leftStick");
        if !left.is_null() && pear_objc::is_kind_of(left, "NSDictionary") {
            for (direction, key) in dict_entries(left) {
                if !pear_objc::is_kind_of(direction, "NSString") || key.is_null() || !pear_objc::is_kind_of(key, "NSString") || msg0::<usize>(key, "length") == 0 {
                    continue;
                }
                let d = rt::ns_string(direction);
                if matches!(d.as_str(), "up" | "down" | "left" | "right") {
                    stick.insert(d, lower(key));
                }
            }
        }
        let sensitivity = get("sensitivity");
        if !sensitivity.is_null() && pear_objc::is_kind_of(sensitivity, "NSNumber") {
            p.sensitivity = msg0::<f64>(sensitivity, "doubleValue").max(0.2).min(3.0);
        }
        let invert = get("invertY");
        if !invert.is_null() && pear_objc::is_kind_of(invert, "NSNumber") {
            p.invert_y = msg0(invert, "boolValue");
        }
    } else if raw.is_empty() {
        for (k, c) in [("space", "a"), ("shift", "b"), ("e", "x"), ("r", "y"), ("q", "l1"), ("f", "r1"), ("up", "dpadUp"), ("down", "dpadDown"), ("left", "dpadLeft"), ("right", "dpadRight"), ("enter", "menu"), ("tab", "options")] {
            keys.insert(k.into(), vec![c.into()]);
        }
        for (k, c) in [("1", "l2"), ("0", "r2")] {
            mouse.insert(k.into(), vec![c.into()]);
        }
        for (d, k) in [("up", "w"), ("down", "s"), ("left", "a"), ("right", "d")] {
            stick.insert(d.into(), k.into());
        }
    }
    // A valid-but-empty map is intentional (the user cleared every binding).
    rt::log(&format!(
        "[PearRuntime][virtual-pad] mapping keys={} mouse={} stick={} sensitivity={:.2} invertY={}",
        keys.len(),
        mouse.len(),
        stick.len(),
        p.sensitivity,
        p.invert_y as i32
    ));
    p.key_to_control = keys;
    p.mouse_to_control = mouse;
    p.stick = stick;
}

unsafe fn element_for(control: &str) -> Id {
    match control {
        "a" => GCInputButtonA,
        "b" => GCInputButtonB,
        "x" => GCInputButtonX,
        "y" => GCInputButtonY,
        "l1" => GCInputLeftShoulder,
        "r1" => GCInputRightShoulder,
        "l2" => GCInputLeftTrigger,
        "r2" => GCInputRightTrigger,
        "menu" => GCInputButtonMenu,
        "options" => GCInputButtonOptions,
        "l3" => GCInputLeftThumbstickButton,
        "r3" => GCInputRightThumbstickButton,
        _ => NIL,
    }
}

unsafe fn configuration_elements(p: &Pad) -> Vec<Id> {
    let mut elements = vec![
        GCInputButtonA, GCInputButtonB, GCInputButtonX, GCInputButtonY, GCInputLeftShoulder, GCInputRightShoulder, GCInputLeftTrigger,
        GCInputRightTrigger, GCInputButtonMenu, GCInputDirectionPad, GCInputLeftThumbstick, GCInputRightThumbstick,
    ];
    for c in p.key_to_control.values().chain(p.mouse_to_control.values()).flatten() {
        let e = element_for(c);
        if !e.is_null() && !elements.contains(&e) {
            elements.push(e);
        }
    }
    elements
}

// ---------------------------------------------------------------------------
// Cursor capture (disassociated from the mouse instead of the game's pointer
// lock, which Catalyst often refuses for mouse events without a UIKit gesture)

extern "C" fn restore_cursor() {
    restore(st());
}

fn restore(p: &mut Pad) {
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

fn set_cursor_captured(p: &mut Pad, captured: bool) {
    if captured {
        if !p.cursor_disassociated && !p.cursor_api_failed {
            let error = unsafe { CGAssociateMouseAndMouseCursorPosition(0) };
            p.cursor_disassociated = error == 0;
            if !p.cursor_disassociated {
                p.cursor_api_failed = true;
                rt::log(&format!("[PearRuntime][virtual-pad] cursor capture API failed ({error}); using window-local mouse deltas"));
            } else {
                rt::log("[PearRuntime][virtual-pad] cursor disassociated");
            }
        }
        if p.cursor_disassociated && !p.cursor_hidden {
            unsafe { CGDisplayHideCursor(CGMainDisplayID()) };
            p.cursor_hidden = true;
        }
        p.captured = true;
    } else {
        let was = p.captured;
        restore(p);
        if was {
            rt::log("[PearRuntime][virtual-pad] cursor released");
        }
    }
}

// ---------------------------------------------------------------------------
// Controller output

unsafe fn set_control(p: &mut Pad, control: &str, value: f64) {
    if !p.ready {
        return;
    }
    let element = element_for(control);
    if element.is_null() {
        return;
    }
    // GameController raises for an element the configuration does not have.
    if !p.elements.contains(&(element as usize)) {
        if p.warned.insert(element as usize) {
            rt::log(&format!("[PearRuntime][virtual-pad] element {} rejected: not in the controller configuration", rt::ns_string(element)));
        }
        return;
    }
    msg2::<f64, Id, ()>(p.pad as Id, "setValue:forButtonElement:", value, element);
}

unsafe fn set_position(p: &Pad, x: f32, y: f32, element: Id) {
    msg2::<CGPoint, Id, ()>(p.pad as Id, "setPosition:forDirectionPadElement:", CGPoint { x: x as f64, y: y as f64 }, element);
}

fn key_pressed(p: &Pad, key: Option<&String>) -> bool {
    key.is_some_and(|k| !k.is_empty() && p.pressed_keys.contains(&k.to_lowercase()))
}

unsafe fn update_left_stick(p: &Pad) {
    if !p.ready {
        return;
    }
    let f = |d: &str| if key_pressed(p, p.stick.get(d)) { 1.0f32 } else { 0.0 };
    let (mut x, mut y) = (f("right") - f("left"), f("up") - f("down"));
    let length = x.hypot(y);
    if length > 1.0 {
        x /= length;
        y /= length;
    }
    set_position(p, x, y, GCInputLeftThumbstick);
}

fn pressed_controls(p: &Pad) -> HashSet<String> {
    let mut set = HashSet::new();
    for k in &p.pressed_keys {
        set.extend(p.key_to_control.get(k).cloned().unwrap_or_default());
    }
    for b in &p.pressed_mouse {
        set.extend(p.mouse_to_control.get(b).cloned().unwrap_or_default());
    }
    set
}

unsafe fn update_dpad(p: &Pad) {
    if !p.ready {
        return;
    }
    let c = pressed_controls(p);
    let f = |n: &str| if c.contains(n) { 1.0f32 } else { 0.0 };
    set_position(p, f("dpadRight") - f("dpadLeft"), f("dpadUp") - f("dpadDown"), GCInputDirectionPad);
}

unsafe fn neutral(p: &mut Pad) {
    p.pressed_keys.clear();
    p.pressed_mouse.clear();
    p.mouse_x = 0.0;
    p.mouse_y = 0.0;
    if !p.ready {
        return;
    }
    let mapped: HashSet<String> = p.key_to_control.values().chain(p.mouse_to_control.values()).flatten().cloned().collect();
    for c in mapped {
        if !is_dpad(&c) {
            set_control(p, &c, 0.0);
        }
    }
    for e in [GCInputDirectionPad, GCInputLeftThumbstick, GCInputRightThumbstick] {
        set_position(p, 0.0, 0.0, e);
    }
}

unsafe fn set_active(p: &mut Pad, active: bool) {
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

unsafe fn controls_description(list: Option<&Vec<String>>) -> String {
    let ids: Vec<Id> = list.map(|l| l.iter().map(|s| nsstring(s)).collect()).unwrap_or_default();
    rt::description(pear_objc::nsarray_from_ids(&ids))
}

unsafe fn apply_controls(p: &mut Pad, list: Option<Vec<String>>) {
    let active = pressed_controls(p);
    for c in list.unwrap_or_default() {
        if !is_dpad(&c) {
            set_control(p, &c, if active.contains(&c) { 1.0 } else { 0.0 });
        }
    }
    update_dpad(p);
}

// ---------------------------------------------------------------------------
// Event input

#[repr(C)]
#[derive(Clone, Copy)]
struct NSRect {
    origin: CGPoint,
    size: pear_objc::CGSize,
}

unsafe fn click_in_content(event: Id) -> bool {
    let window: Id = msg0(event, "window");
    if window.is_null() || !pear_objc::responds_to(window, "isKeyWindow") || !pear_objc::responds_to(window, "contentView") {
        return false;
    }
    if !msg0::<bool>(window, "isKeyWindow") {
        return false;
    }
    let view: Id = msg0(window, "contentView");
    if view.is_null() || !pear_objc::responds_to(view, "convertPoint:fromView:") || !pear_objc::responds_to(view, "bounds") {
        return false;
    }
    let location: CGPoint = msg0(event, "locationInWindow");
    let point: CGPoint = msg2(view, "convertPoint:fromView:", location, NIL);
    let b: NSRect = msg0(view, "bounds");
    // NSPointInRect for an unflipped view: [min, max) on both axes.
    point.x >= b.origin.x && point.x < b.origin.x + b.size.width && point.y >= b.origin.y && point.y < b.origin.y + b.size.height
}

unsafe fn mouse_button(p: &mut Pad, button: isize, pressed: bool) {
    let name = button.to_string();
    let list = p.mouse_to_control.get(&name).cloned();
    if pressed {
        p.pressed_mouse.insert(name);
    } else {
        p.pressed_mouse.remove(&name);
    }
    if rt::env_set("PEAR_DEBUG_POINTER") {
        rt::log(&format!(
            "[PearRuntime][virtual-pad] mouse button={button} pressed={} control={}",
            pressed as i32,
            if list.is_none() { "(null)".to_string() } else { controls_description(list.as_ref()) }
        ));
    }
    apply_controls(p, list);
}

unsafe fn pad_key(key: &str, pressed: bool) {
    if !enabled() || key.is_empty() {
        return;
    }
    let key = key.to_string();
    rt::async_main(move || unsafe {
        let p = st();
        if !p.ready || !foreground() {
            return;
        }
        let name = lower(nsstring(&key));
        if name == "escape" {
            if p.capture.escape(pressed) {
                neutral(p);
                set_cursor_captured(p, false);
            }
            return;
        }
        // Ignore key repeat / duplicate delivery.
        if p.pressed_keys.contains(&name) == pressed {
            return;
        }
        if pressed {
            p.pressed_keys.insert(name.clone());
        } else {
            p.pressed_keys.remove(&name);
        }
        let list = p.key_to_control.get(&name).cloned();
        let debug = rt::env_set("PEAR_DEBUG_POINTER").then(|| controls_description(Some(list.as_ref().unwrap_or(&vec![]))));
        apply_controls(p, list);
        update_left_stick(p);
        if let Some(d) = debug {
            rt::log(&format!("[PearRuntime][virtual-pad] key={name} pressed={} control={d}", pressed as i32));
        }
    });
}

#[export_name = "_Z21PearVirtualGamepadKeyP8NSStringb"]
pub unsafe extern "C" fn virtual_gamepad_key(key: Id, pressed: bool) {
    // Prefer local native events when available, so GCKeyboard and AppKit
    // cannot release each other's held keys.
    if !st().monitor && !key.is_null() {
        pad_key(&rt::ns_string(key), pressed);
    }
}

fn uptime() -> f64 {
    unsafe { msg0(msg0::<Id>(cls("NSProcessInfo"), "processInfo"), "systemUptime") }
}

fn accumulate(p: &mut Pad, dx: f32, dy: f32) {
    let scale = 0.035f32 * p.sensitivity as f32;
    let vertical = if p.invert_y { -dy } else { dy };
    p.mouse_x = (p.mouse_x + dx * scale).min(1.0).max(-1.0);
    p.mouse_y = (p.mouse_y + vertical * scale).min(1.0).max(-1.0);
}

#[export_name = "_Z23PearVirtualGamepadMouseff"]
pub extern "C" fn virtual_gamepad_mouse(dx: f32, dy: f32) {
    if !enabled() {
        return;
    }
    rt::async_main(move || unsafe {
        let p = st();
        p.mouse_received += 1;
        if !p.ready || !p.captured || !foreground() || !dx.is_finite() || !dy.is_finite() {
            return;
        }
        p.mouse_accepted += 1;
        p.last_gc_mouse_time = uptime();
        accumulate(p, dx, dy);
        p.last_mouse_time = p.last_gc_mouse_time;
    });
}

/// NSEvent fallback for hosts where GCMouse does not deliver while the cursor
/// is disassociated; ignored for a quarter second after a GCMouse sample.
#[export_name = "_Z28PearVirtualGamepadMouseEventff"]
pub extern "C" fn virtual_gamepad_mouse_event(dx: f32, dy: f32) {
    if !enabled() {
        return;
    }
    rt::async_main(move || unsafe {
        let p = st();
        p.mouse_event_samples += 1;
        if !p.ready || !p.captured || !foreground() || !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let now = uptime();
        if now - p.last_gc_mouse_time < 0.25 {
            return;
        }
        p.mouse_accepted += 1;
        accumulate(p, dx, dy);
        p.last_mouse_time = now;
    });
}

// ---------------------------------------------------------------------------
// Install

unsafe fn connect(elements: Vec<Id>, allow_fallback: bool) {
    let set: Id = msg1(cls("NSSet"), "setWithArray:", pear_objc::nsarray_from_ids(&elements));
    let configuration: Id = msg0(msg0::<Id>(cls("GCVirtualControllerConfiguration"), "alloc"), "init");
    msg1::<Id, ()>(configuration, "setElements:", set);
    msg1::<bool, ()>(configuration, "setHidden:", true);
    let pad: Id = msg1(msg0::<Id>(cls("GCVirtualController"), "alloc"), "initWithConfiguration:", configuration);
    pear_objc::objc_release(configuration);
    {
        let p = st();
        if p.pad != 0 {
            pear_objc::objc_release(p.pad as Id);
        }
        p.pad = pad as usize;
        p.elements = elements.iter().map(|e| *e as usize).collect();
    }
    if pad.is_null() {
        return;
    }
    let count = elements.len();
    let elements_keep: Vec<usize> = elements.iter().map(|e| *e as usize).collect();
    let handler = rt::block1(move |error| unsafe {
        let error = if error.is_null() { 0 } else { pear_objc::objc_retain(error) as usize };
        let elements_keep = elements_keep.clone();
        rt::async_main(move || unsafe {
            let p = st();
            if error != 0 {
                let error = error as Id;
                let description = rt::ns_string(msg0(error, "localizedDescription"));
                pear_objc::objc_release(error);
                let optional = [GCInputButtonOptions, GCInputLeftThumbstickButton, GCInputRightThumbstickButton];
                let has_optional = optional.iter().any(|o| elements_keep.contains(&(*o as usize)));
                if allow_fallback && has_optional {
                    let reduced: Vec<Id> = elements_keep.iter().map(|e| *e as Id).filter(|e| !optional.contains(e)).collect();
                    rt::log(&format!("[PearRuntime][virtual-pad] retry without optional elements: {description}"));
                    if p.pad != 0 {
                        pear_objc::objc_release(p.pad as Id);
                        p.pad = 0;
                    }
                    connect(reduced, false);
                    return;
                }
                rt::log(&format!("[PearRuntime][virtual-pad] connection failed: {description}"));
                return;
            }
            p.ready = p.pad != 0 && !msg0::<Id>(p.pad as Id, "controller").is_null();
            rt::log(&format!("[PearRuntime][virtual-pad] connected={} elements={count}", p.ready as i32));
            if p.ready {
                set_active(p, foreground());
            }
        });
    });
    msg1::<Id, ()>(pad, "connectWithReplyHandler:", handler);
    rt::release_block(handler);
}

fn tick() -> bool {
    unsafe {
        let p = st();
        if !p.ready {
            return false;
        }
        let fg = foreground();
        if p.capture.suspended == fg {
            set_active(p, fg);
        }
        let state = fg as i64 | (p.captured as i64) << 1 | (p.capture.requested as i64) << 2;
        let debug = rt::env_set("PEAR_DEBUG_POINTER");
        if state != p.last_capture_state && debug {
            rt::log(&format!(
                "[PearRuntime][virtual-pad] capture requested={} foreground={} captured={} cursorAssoc={}",
                p.capture.requested as i32, fg as i32, p.captured as i32, p.cursor_disassociated as i32
            ));
        }
        p.last_capture_state = state;
        if !fg {
            return false;
        }
        let now = uptime();
        if !p.captured || now - p.last_mouse_time > 0.1 {
            p.mouse_x = 0.0;
            p.mouse_y = 0.0;
        }
        set_position(p, p.mouse_x, p.mouse_y, GCInputRightThumbstick);
        if debug && p.mouse_received + p.mouse_event_samples > 0 && now - p.last_mouse_diagnostic >= 1.0 {
            let stick: Id = msg0(msg0::<Id>(msg0::<Id>(p.pad as Id, "controller"), "extendedGamepad"), "rightThumbstick");
            let axis = |n: &str| -> f32 { if stick.is_null() { 0.0 } else { msg0(msg0::<Id>(stick, n), "value") } };
            rt::log(&format!(
                "[PearRuntime][virtual-pad] mouse gc={} event={} accepted={} right=({:.3},{:.3}) readback=({:.3},{:.3}) captured={}",
                p.mouse_received, p.mouse_event_samples, p.mouse_accepted, p.mouse_x, p.mouse_y, axis("xAxis"), axis("yAxis"), p.captured as i32
            ));
            p.mouse_received = 0;
            p.mouse_accepted = 0;
            p.mouse_event_samples = 0;
            p.last_mouse_diagnostic = now;
        }
        p.mouse_x *= 0.7;
        p.mouse_y *= 0.7;
        false
    }
}

const MODIFIERS: [(&str, usize); 5] = [("shift", 1 << 17), ("control", 1 << 18), ("option", 1 << 19), ("command", 1 << 20), ("capslock", 1 << 16)];

unsafe fn handle_event(event: Id) -> Id {
    let p = st();
    if !p.ready || !foreground() {
        return event;
    }
    let kind: isize = msg0(event, "type");
    if matches!(kind, 10..=12) {
        let code: u16 = msg0(event, "keyCode");
        let name = crate::input::key_names::host_key_name(code);
        let flags: usize = msg0(event, "modifierFlags");
        if kind == 12 {
            // AppKit supplies aggregate left/right modifier state.
            if let Some((n, mask)) = MODIFIERS.iter().find(|(n, _)| Some(*n) == name) {
                pad_key(n, flags & mask != 0);
            }
        } else if let Some(n) = name {
            pad_key(n, kind == 10);
        }
        // Preserve system shortcuts such as Command-Tab.
        let n = name.unwrap_or("");
        let mapped = p.key_to_control.get(n).is_some_and(|c| !c.is_empty()) || p.stick.values().any(|v| v == n);
        return if mapped && flags & (1 << 20) == 0 && kind != 12 { NIL } else { event };
    }
    if matches!(kind, 5 | 6 | 7 | 27) {
        // Native movement and drag.
        if p.captured {
            let dx: f64 = msg0(event, "deltaX");
            let dy: f64 = msg0(event, "deltaY");
            virtual_gamepad_mouse_event(dx as f32, dy as f32);
        }
        return event;
    }
    let down = matches!(kind, 1 | 3 | 25);
    let up = matches!(kind, 2 | 4 | 26);
    if !down && !up {
        return event;
    }
    if kind == 2 && p.click_passthrough {
        // Left up of the arming click.
        p.click_passthrough = false;
        return event;
    }
    if !p.captured {
        if kind == 1 && click_in_content(event) {
            p.capture.requested = true;
            set_active(p, true);
            p.click_passthrough = true;
            // UIKit sees the activating click, so it also reaches the game's menu.
        }
        return event;
    }
    let button: isize = match kind {
        3 | 4 => 1,
        25 | 26 => msg0(event, "buttonNumber"),
        _ => 0,
    };
    mouse_button(p, button, down);
    NIL
}

#[export_name = "_Z25PearInstallVirtualGamepadv"]
pub extern "C" fn install() {
    if !enabled() {
        return;
    }
    rt::async_main(|| unsafe {
        #[repr(C)]
        struct Version {
            major: isize,
            minor: isize,
            patch: isize,
        }
        // Mac Catalyst 17 runs on macOS 14.
        let info: Id = msg0(cls("NSProcessInfo"), "processInfo");
        if !msg1::<Version, bool>(info, "isOperatingSystemAtLeastVersion:", Version { major: 14, minor: 0, patch: 0 }) {
            rt::log("[PearRuntime][virtual-pad] requires Mac Catalyst 17 or newer");
            return;
        }
        let configuration = rt::class_named("GCVirtualControllerConfiguration");
        let controller = rt::class_named("GCVirtualController");
        let responds = |c, s: &str| !c.is_null() && msg1::<Sel, bool>(c, "instancesRespondToSelector:", pear_objc::sel(s));
        if !responds(configuration, "setHidden:") || !responds(controller, "setPosition:forDirectionPadElement:") {
            rt::log("[PearRuntime][virtual-pad] hidden virtual-controller API unavailable");
            return;
        }
        let p = st();
        p.pressed_keys.clear();
        p.pressed_mouse.clear();
        p.capture.requested = false;
        load_mapping(p);
        atexit(restore_cursor);
        let elements = configuration_elements(p);
        connect(elements, true);
        rt::main_ticker(1.0, 1.0 / 60.0, 1.0 / 600.0, tick);

        let event_class = rt::class_named("NSEvent");
        if !event_class.is_null() && pear_objc::responds_to(event_class, "addLocalMonitorForEventsMatchingMask:handler:") {
            // left/right/other mouse down+up, moved/dragged, key down/up, flags changed.
            let mask: usize = (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 27) | (1 << 10) | (1 << 11) | (1 << 12) | (1 << 25) | (1 << 26);
            let handler = rt::block1_ret(|event| unsafe { handle_event(event) });
            let monitor: Id = msg2(event_class, "addLocalMonitorForEventsMatchingMask:handler:", mask, handler);
            rt::release_block(handler);
            if !monitor.is_null() {
                pear_objc::objc_retain(monitor);
                st().monitor = true;
            }
        }
        for name in ["UIApplicationWillResignActiveNotification", "NSApplicationWillResignActiveNotification"] {
            rt::observe_main(name, |_| unsafe { set_active(st(), false) });
        }
        for name in ["UIApplicationDidBecomeActiveNotification", "NSApplicationDidBecomeActiveNotification"] {
            rt::observe_main(name, |_| unsafe { set_active(st(), true) });
        }
        for name in ["UIApplicationWillTerminateNotification", "NSApplicationWillTerminateNotification"] {
            rt::observe_main(name, |_| unsafe {
                let p = st();
                set_active(p, false);
                restore(p);
            });
        }
    });
}
