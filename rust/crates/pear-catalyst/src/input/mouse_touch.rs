//! Mouse-to-touch fallback for games without native mouse touches
//! (PEAR_MOUSE_TOUCH=1). A native UIKit touch and a synthetic finger are
//! never both generated: a short grace period lets UIKit deliver a native
//! touch first, which keeps normal mouse handling intact.

use crate::geometry::{aspect_fit_rect, point_in_rect, rect_is_empty};
use crate::rt;
use pear_objc::{cls, msg0, msg2, nsarray_count, nsarray_object_at, nsstring, objc_release, objc_retain, sel, CGPoint, CGRect, Class, Id, Imp, Sel, NIL};
use std::cell::Cell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

extern "C" {
    #[link_name = "_Z27PearSynthesizeVirtualFingerbP8NSStringdd"]
    fn PearSynthesizeVirtualFinger(down: bool, key: Id, nx: f64, ny: f64);
    #[link_name = "_Z28PearWindowVisibleContentRectP11objc_objectS0_"]
    fn PearWindowVisibleContentRect(window: Id, content: Id) -> CGRect;
    fn class_getMethodImplementation(c: Class, s: Sel) -> Imp;
}

use crate::input::legacy_touch::{cancel_legacy_direct_touch, run_legacy_direct_touch_self_test, send_legacy_direct_touch};

static ADAPTIVE_DIRECT: AtomicBool = AtomicBool::new(false);
static NATIVE_GENERATION: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_SEND_EVENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static MONITOR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

const LEFT_DOWN: isize = 1;
const LEFT_UP: isize = 2;
const LEFT_DRAGGED: isize = 6;
const APP_ACTIVE: isize = 0;

/// Gesture state; only touched on the main thread (event monitor, timers).
struct Gesture {
    pending: Cell<bool>,
    held: Cell<bool>,
    synthetic: Cell<bool>,
    serial: Cell<u64>,
    native_generation: Cell<u64>,
    start: Cell<CGPoint>,
    last: Cell<CGPoint>,
    window: Cell<usize>, // retained
}

thread_local! {
    static G: Gesture = const { Gesture {
        pending: Cell::new(false), held: Cell::new(false), synthetic: Cell::new(false),
        serial: Cell::new(0), native_generation: Cell::new(0),
        start: Cell::new(CGPoint { x: 0.0, y: 0.0 }), last: Cell::new(CGPoint { x: 0.0, y: 0.0 }),
        window: Cell::new(0),
    } };
}

fn debug() -> bool {
    rt::env_set("PEAR_DEBUG_TOUCH")
}

fn direct_legacy() -> bool {
    rt::env_bool("PEAR_DIRECT_LEGACY_TOUCH")
}

unsafe fn set_gesture_window(w: Id) {
    G.with(|g| {
        let old = g.window.replace(if w.is_null() { 0 } else { objc_retain(w) as usize });
        if old != 0 {
            objc_release(old as Id);
        }
    });
}

/// Only engine-owned rendering views use responder delivery; UIKit controls,
/// scroll views and gesture-recognizer surfaces keep the system event path.
unsafe fn can_deliver_engine_touch(n: CGPoint) -> bool {
    let window = rt::primary_window();
    if window.is_null() {
        return false;
    }
    let b: CGRect = msg0(window, "bounds");
    let point = CGPoint { x: b.origin.x + n.x * b.size.width, y: b.origin.y + n.y * b.size.height };
    let view: Id = msg2(window, "hitTest:withEvent:", point, NIL);
    if view.is_null()
        || pear_objc::is_kind_of(view, "UIControl")
        || pear_objc::is_kind_of(view, "UIScrollView")
        || nsarray_count(msg0(view, "gestureRecognizers")) > 0
    {
        return false;
    }
    let layer: Id = msg0(view, "layer");
    if !(pear_objc::is_kind_of(layer, "CAMetalLayer") || pear_objc::is_kind_of(layer, "CAEAGLLayer")) {
        return false;
    }
    let began = sel("touchesBegan:withEvent:");
    class_getMethodImplementation(rt::object_getClass(view), began) != class_getMethodImplementation(cls("UIView"), began)
}

unsafe fn send_mouse_touch(down: bool, nx: f64, ny: f64) {
    if ADAPTIVE_DIRECT.load(Ordering::Relaxed) || direct_legacy() {
        send_legacy_direct_touch(down, nx, ny);
    } else {
        PearSynthesizeVirtualFinger(down, nsstring("f:mouseclick"), nx, ny);
    }
}

unsafe fn cancel() {
    G.with(|g| {
        if g.synthetic.get() {
            let l = g.last.get();
            send_mouse_touch(false, l.x, l.y);
        }
        ADAPTIVE_DIRECT.store(false, Ordering::Relaxed);
        g.pending.set(false);
        g.held.set(false);
        g.synthetic.set(false);
        g.serial.set(g.serial.get() + 1);
    });
    set_gesture_window(NIL);
}

/// UIApplication sendEvent: notes every native touch that begins (pointer
/// hover/move events also carry touches; they do not count).
unsafe extern "C" fn send_event(s: Id, c: Sel, event: Id) {
    if !event.is_null() && pear_objc::responds_to(event, "allTouches") {
        let touches: Id = msg0(msg0::<Id>(event, "allTouches"), "allObjects");
        let began = (0..nsarray_count(touches)).any(|i| msg0::<isize>(nsarray_object_at(touches, i), "phase") == 0);
        if began {
            let generation = NATIVE_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
            if debug() {
                rt::log(&format!("[PearRuntime][mouse-touch] UIKit touches={} generation={generation}", nsarray_count(touches)));
            }
        }
    }
    let f: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(ORIGINAL_SEND_EVENT.load(Ordering::Acquire));
    f(s, c, event);
}

unsafe fn install_native_touch_tracking() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let m = rt::class_getInstanceMethod(cls("UIApplication"), sel("sendEvent:"));
        if !m.is_null() {
            ORIGINAL_SEND_EVENT.store(rt::method_getImplementation(m) as *mut c_void, Ordering::Release);
            rt::method_setImplementation(m, send_event as Imp);
        }
    });
}

unsafe fn app_active() -> bool {
    msg0::<isize>(msg0::<Id>(cls("UIApplication"), "sharedApplication"), "applicationState") == APP_ACTIVE
}

unsafe fn handle(event: Id) -> Id {
    let kind: isize = msg0(event, "type");
    let direct_gesture = G.with(|g| g.pending.get()) && direct_legacy();
    let window: Id = msg0(event, "window");
    if kind == LEFT_DOWN {
        cancel();
        if !app_active() || window.is_null() || !msg0::<bool>(window, "isKeyWindow") {
            return event;
        }
    } else if !G.with(|g| g.pending.get()) || window as usize != G.with(|g| g.window.get()) {
        if G.with(|g| g.pending.get()) && kind == LEFT_UP {
            cancel();
        }
        return event;
    }
    let content: Id = if window.is_null() { NIL } else { msg0(window, "contentView") };
    if content.is_null() {
        cancel();
        return event;
    }
    // Only the area below the title bar is game content; clicks on the title
    // bar must reach the window's buttons.
    let mut bounds = PearWindowVisibleContentRect(window, content);
    let in_window: CGPoint = msg0(event, "locationInWindow");
    let point: CGPoint = msg2(content, "convertPoint:fromView:", in_window, NIL);
    if rt::env_bool("PEAR_LEGACY_ATTACH_LAYER") {
        let ui = rt::primary_window();
        let fitted = if ui.is_null() { CGRect::default() } else { aspect_fit_rect(msg0::<CGRect>(ui, "bounds").size, bounds) };
        if !rect_is_empty(fitted) {
            bounds = fitted;
        }
    }
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 || (kind == LEFT_DOWN && !point_in_rect(point, bounds)) {
        return event;
    }
    let nx = ((point.x - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0);
    let y = ((point.y - bounds.origin.y) / bounds.size.height).clamp(0.0, 1.0);
    let flipped: bool = msg0(content, "isFlipped");
    let last = CGPoint { x: nx, y: if flipped { y } else { 1.0 - y } };
    G.with(|g| g.last.set(last));
    if kind == LEFT_DOWN {
        let this_gesture = G.with(|g| {
            g.pending.set(true);
            g.held.set(true);
            g.start.set(last);
            g.native_generation.set(NATIVE_GENERATION.load(Ordering::Relaxed));
            g.serial.set(g.serial.get() + 1);
            g.serial.get()
        });
        set_gesture_window(window);
        if debug() {
            rt::log(&format!(
                "[PearRuntime][mouse-touch] down normalized=({:.3},{:.3}) generation={}",
                last.x, last.y, NATIVE_GENERATION.load(Ordering::Relaxed)
            ));
        }
        // The explicitly selected direct path owns input for the reattached
        // legacy window: keep the timing of real gestures.
        if direct_legacy() {
            G.with(|g| g.synthetic.set(true));
            send_mouse_touch(true, last.x, last.y);
            return NIL;
        }
        rt::after_main(0.22, move || unsafe { begin_fallback(this_gesture) });
    } else if kind == LEFT_DRAGGED {
        let synthetic = G.with(|g| g.synthetic.get());
        if debug() {
            rt::log(&format!("[PearRuntime][mouse-touch] drag normalized=({:.3},{:.3}) synthetic={}", last.x, last.y, synthetic as i32));
        }
        if synthetic {
            send_mouse_touch(true, last.x, last.y);
        }
    } else if kind == LEFT_UP {
        let synthetic = G.with(|g| g.synthetic.get());
        if debug() {
            rt::log(&format!("[PearRuntime][mouse-touch] up normalized=({:.3},{:.3}) synthetic={}", last.x, last.y, synthetic as i32));
        }
        G.with(|g| g.held.set(false));
        if synthetic {
            cancel();
        }
    }
    if direct_gesture { NIL } else { event }
}

unsafe fn begin_fallback(this_gesture: u64) {
    let (pending, serial, native, window) =
        G.with(|g| (g.pending.get(), g.serial.get(), g.native_generation.get(), g.window.get() as Id));
    if !pending || serial != this_gesture {
        return;
    }
    let key = !window.is_null() && msg0::<bool>(window, "isKeyWindow");
    let current = NATIVE_GENERATION.load(Ordering::Relaxed);
    if native != current || !app_active() || !key {
        if debug() {
            rt::log(&format!(
                "[PearRuntime][mouse-touch] fallback cancelled native={} active={} key={}",
                (native != current) as i32, app_active() as i32, key as i32
            ));
        }
        cancel();
        return;
    }
    let (start, last) = G.with(|g| {
        g.synthetic.set(true);
        (g.start.get(), g.last.get())
    });
    ADAPTIVE_DIRECT.store(can_deliver_engine_touch(start), Ordering::Relaxed);
    if debug() {
        rt::log(&format!("[PearRuntime][mouse-touch] engine responder fallback={}", ADAPTIVE_DIRECT.load(Ordering::Relaxed) as i32));
        rt::log(&format!("[PearRuntime][mouse-touch] fallback begin ({:.3},{:.3})", start.x, start.y));
    }
    send_mouse_touch(true, start.x, start.y);
    if start != last {
        send_mouse_touch(true, last.x, last.y);
    }
    if !G.with(|g| g.held.get()) {
        rt::after_main(0.06, move || unsafe {
            if G.with(|g| g.pending.get() && g.serial.get() == this_gesture && !g.held.get()) {
                cancel();
            }
        });
    }
}

#[export_name = "_Z31PearInstallMouseToTouchFallbackv"]
pub unsafe extern "C" fn install_mouse_to_touch_fallback() {
    if !rt::env_bool("PEAR_MOUSE_TOUCH") {
        return;
    }
    install_native_touch_tracking();
    let event_class = cls("NSEvent");
    if event_class.is_null() || !pear_objc::responds_to(event_class, "addLocalMonitorForEventsMatchingMask:handler:") {
        return;
    }
    let mask: u64 = (1 << 1) | (1 << 2) | (1 << 6); // left down/up/dragged
    let handler = rt::block1_ret(|event| unsafe { handle(event) });
    let monitor: Id = msg2(event_class, "addLocalMonitorForEventsMatchingMask:handler:", mask, handler);
    rt::release_block(handler);
    if !monitor.is_null() {
        MONITOR.store(objc_retain(monitor) as *mut c_void, Ordering::Release);
    }
    rt::observe_main("UIApplicationWillResignActiveNotification", |_| unsafe {
        if G.with(|g| g.synthetic.get()) && (ADAPTIVE_DIRECT.load(Ordering::Relaxed) || direct_legacy()) {
            cancel_legacy_direct_touch();
            G.with(|g| g.synthetic.set(false));
        }
        cancel();
    });
    rt::log(&format!(
        "[PearRuntime][touch] mouse-to-touch fallback {}",
        if monitor.is_null() { "unavailable" } else { "installed" }
    ));
    run_legacy_direct_touch_self_test();
}
