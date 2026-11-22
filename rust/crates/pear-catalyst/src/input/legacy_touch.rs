//! Local touch bridge for scene-less apps whose visual layer has been
//! reattached but whose system digitizer cannot route back to the UIKit
//! scene. Handles only actual user mouse gestures delivered to this process
//! (from the mouse monitor, on the main queue); it posts no system events.

use crate::rt::{self, ClassBuilder};
use pear_objc::{
    cls, msg0, msg1, msg2, objc_loadWeakRetained, objc_release, objc_retain, objc_storeWeak, objc_destroyWeak,
    nsstring, sel, CGPoint, CGRect, Class, Id, Imp, Sel, NIL,
};
use std::sync::atomic::{AtomicPtr, Ordering};

extern "C" {
    #[link_name = "_Z17PearPrimaryWindowv"]
    fn PearPrimaryWindow() -> Id;
    #[link_name = "_Z19PearGLMapTouchPointP7UITouchP6UIView7CGPoint"]
    fn PearGLMapTouchPoint(touch: Id, view: Id, point: CGPoint) -> CGPoint;
}

const PHASE_BEGAN: isize = 0;
const PHASE_MOVED: isize = 1;
const PHASE_ENDED: isize = 3;
const PHASE_CANCELLED: isize = 4;

#[repr(C)]
struct TouchState {
    window: Id, // weak
    view: Id,   // weak
    point: CGPoint,
    previous: CGPoint,
    phase: isize,
    timestamp: f64,
}

#[repr(C)]
struct EventState {
    touch: Id, // strong
}

static TOUCH_CLASS: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static EVENT_CLASS: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static DIRECT_TOUCH: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static DIRECT_EVENT: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe fn touch(obj: Id) -> &'static mut TouchState {
    rt::ivar_state::<TouchState>(obj)
}

unsafe fn weak_get(slot: *mut Id) -> Id {
    rt::autoreleased(objc_loadWeakRetained(slot))
}

unsafe fn uptime() -> f64 {
    msg0(msg0::<Id>(cls("NSProcessInfo"), "processInfo"), "systemUptime")
}

// ---- PearLegacyTouch : UITouch ----
unsafe extern "C" fn t_window(s: Id, _c: Sel) -> Id { weak_get(&mut touch(s).window) }
unsafe extern "C" fn t_view(s: Id, _c: Sel) -> Id { weak_get(&mut touch(s).view) }
unsafe extern "C" fn t_phase(s: Id, _c: Sel) -> isize { touch(s).phase }
unsafe extern "C" fn t_type(_s: Id, _c: Sel) -> isize { 0 } // UITouchTypeDirect
unsafe extern "C" fn t_tap_count(_s: Id, _c: Sel) -> usize { 1 }
unsafe extern "C" fn t_timestamp(s: Id, _c: Sel) -> f64 { touch(s).timestamp }
unsafe extern "C" fn t_force(s: Id, _c: Sel) -> f64 {
    let p = touch(s).phase;
    if p == PHASE_ENDED || p == PHASE_CANCELLED { 0.0 } else { 1.0 }
}
unsafe extern "C" fn t_max_force(_s: Id, _c: Sel) -> f64 { 1.0 }
unsafe fn located(s: Id, view: Id, point: CGPoint) -> CGPoint {
    let p = if view.is_null() {
        point
    } else {
        let window = t_window(s, sel("window"));
        msg2(window, "convertPoint:toView:", point, view)
    };
    PearGLMapTouchPoint(s, view, p)
}
unsafe extern "C" fn t_location(s: Id, _c: Sel, view: Id) -> CGPoint { located(s, view, touch(s).point) }
unsafe extern "C" fn t_previous(s: Id, _c: Sel, view: Id) -> CGPoint { located(s, view, touch(s).previous) }
unsafe extern "C" fn t_dealloc(s: Id, _c: Sel) {
    let st = touch(s);
    objc_destroyWeak(&mut st.window);
    objc_destroyWeak(&mut st.view);
    pear_objc::msg_super0::<()>(s, "PearLegacyTouch", "dealloc");
}

// ---- PearLegacyEvent : UIEvent ----
unsafe fn event(obj: Id) -> &'static mut EventState {
    rt::ivar_state::<EventState>(obj)
}
unsafe extern "C" fn e_type(_s: Id, _c: Sel) -> isize { 0 } // UIEventTypeTouches
unsafe extern "C" fn e_timestamp(s: Id, _c: Sel) -> f64 {
    let t = event(s).touch;
    if t.is_null() { 0.0 } else { touch(t).timestamp }
}
unsafe extern "C" fn e_all(s: Id, _c: Sel) -> Id {
    let t = event(s).touch;
    if t.is_null() { msg0(cls("NSSet"), "set") } else { msg1(cls("NSSet"), "setWithObject:", t) }
}
unsafe extern "C" fn e_for_window(s: Id, c: Sel, window: Id) -> Id {
    let t = event(s).touch;
    if !t.is_null() && window == t_window(t, c) { e_all(s, c) } else { msg0(cls("NSSet"), "set") }
}
unsafe extern "C" fn e_for_view(s: Id, c: Sel, view: Id) -> Id {
    let t = event(s).touch;
    if !t.is_null() && view == t_view(t, c) { e_all(s, c) } else { msg0(cls("NSSet"), "set") }
}
unsafe extern "C" fn e_dealloc(s: Id, _c: Sel) {
    let st = event(s);
    if !st.touch.is_null() {
        objc_release(st.touch);
        st.touch = NIL;
    }
    pear_objc::msg_super0::<()>(s, "PearLegacyEvent", "dealloc");
}

const PT: &str = "{CGPoint=dd}";

pub fn init() {
    unsafe {
        if let Some(b) = ClassBuilder::new::<TouchState>("PearLegacyTouch", cls("UITouch")) {
            b.method("window", t_window as Imp, "@@:")
                .method("view", t_view as Imp, "@@:")
                .method("phase", t_phase as Imp, "q@:")
                .method("type", t_type as Imp, "q@:")
                .method("tapCount", t_tap_count as Imp, "Q@:")
                .method("timestamp", t_timestamp as Imp, "d@:")
                .method("force", t_force as Imp, "d@:")
                .method("maximumPossibleForce", t_max_force as Imp, "d@:")
                .method("locationInView:", t_location as Imp, &format!("{PT}@:@"))
                .method("previousLocationInView:", t_previous as Imp, &format!("{PT}@:@"))
                .method("preciseLocationInView:", t_location as Imp, &format!("{PT}@:@"))
                .method("precisePreviousLocationInView:", t_previous as Imp, &format!("{PT}@:@"))
                .method("dealloc", t_dealloc as Imp, "v@:");
            TOUCH_CLASS.store(b.register() as *mut _, Ordering::Release);
        }
        if let Some(b) = ClassBuilder::new::<EventState>("PearLegacyEvent", cls("UIEvent")) {
            b.method("type", e_type as Imp, "q@:")
                .method("timestamp", e_timestamp as Imp, "d@:")
                .method("allTouches", e_all as Imp, "@@:")
                .method("touchesForWindow:", e_for_window as Imp, "@@:@")
                .method("touchesForView:", e_for_view as Imp, "@@:@")
                .method("dealloc", e_dealloc as Imp, "v@:");
            EVENT_CLASS.store(b.register() as *mut _, Ordering::Release);
        }
    }
}

fn debug() -> bool {
    rt::env_set("PEAR_DEBUG_TOUCH")
}

unsafe fn log_target(view: Id) {
    if !debug() || view.is_null() {
        return;
    }
    extern "C" {
        fn class_getMethodImplementation(c: Class, s: Sel) -> Imp;
    }
    let c = rt::object_getClass(view);
    let began = sel("touchesBegan:withEvent:");
    rt::log(&format!(
        "[PearRuntime][mouse-touch] direct target={} interaction={} began={} impl={:p}",
        rt::class_name(c),
        msg0::<bool>(view, "isUserInteractionEnabled") as i32,
        pear_objc::responds_to(view, "touchesBegan:withEvent:") as i32,
        class_getMethodImplementation(c, began)
    ));
}

unsafe fn clear_direct() {
    let e = DIRECT_EVENT.swap(std::ptr::null_mut(), Ordering::AcqRel) as Id;
    let t = DIRECT_TOUCH.swap(std::ptr::null_mut(), Ordering::AcqRel) as Id;
    if !e.is_null() {
        objc_release(e);
    }
    if !t.is_null() {
        objc_release(t);
    }
}

#[export_name = "_Z27PearCancelLegacyDirectTouchv"]
pub unsafe extern "C" fn cancel_legacy_direct_touch() {
    let t = DIRECT_TOUCH.load(Ordering::Acquire) as Id;
    if t.is_null() {
        return;
    }
    let e = DIRECT_EVENT.load(Ordering::Acquire) as Id;
    let st = touch(t);
    st.phase = PHASE_CANCELLED;
    st.timestamp = uptime();
    let view = t_view(t, sel("view"));
    msg2::<Id, Id, ()>(view, "touchesCancelled:withEvent:", e_all(e, sel("allTouches")), e);
    clear_direct();
}

#[export_name = "_Z25PearSendLegacyDirectTouchbdd"]
pub unsafe extern "C" fn send_legacy_direct_touch(down: bool, nx: f64, ny: f64) {
    let window = PearPrimaryWindow();
    if window.is_null() {
        return;
    }
    let b: CGRect = msg0(window, "bounds");
    if b.size.width < 1.0 || b.size.height < 1.0 {
        return;
    }
    let point = CGPoint { x: b.origin.x + nx * b.size.width, y: b.origin.y + ny * b.size.height };
    let current = DIRECT_TOUCH.load(Ordering::Acquire) as Id;
    if down && current.is_null() {
        let view: Id = msg2(window, "hitTest:withEvent:", point, NIL);
        if view.is_null() {
            rt::log("[PearRuntime][mouse-touch] direct hit test returned no view");
            return;
        }
        let t: Id = msg0(TOUCH_CLASS.load(Ordering::Acquire) as Id, "new");
        let e: Id = msg0(EVENT_CLASS.load(Ordering::Acquire) as Id, "new");
        if t.is_null() || e.is_null() {
            return;
        }
        let st = touch(t);
        objc_storeWeak(&mut st.window, window);
        objc_storeWeak(&mut st.view, view);
        st.point = point;
        st.previous = point;
        st.phase = PHASE_BEGAN;
        st.timestamp = uptime();
        event(e).touch = objc_retain(t);
        DIRECT_TOUCH.store(t as *mut _, Ordering::Release);
        DIRECT_EVENT.store(e as *mut _, Ordering::Release);
        if debug() {
            let p: Id = {
                extern "C" {
                    fn NSStringFromCGPoint(p: CGPoint) -> Id;
                }
                NSStringFromCGPoint(point)
            };
            rt::log(&format!(
                "[PearRuntime][mouse-touch] direct began view={} point={}",
                rt::class_name(rt::object_getClass(view)),
                rt::ns_string(p)
            ));
        }
        log_target(view);
        msg2::<Id, Id, ()>(view, "touchesBegan:withEvent:", e_all(e, sel("allTouches")), e);
        return;
    }
    if current.is_null() {
        return;
    }
    let e = DIRECT_EVENT.load(Ordering::Acquire) as Id;
    let st = touch(current);
    let view = t_view(current, sel("view"));
    st.previous = st.point;
    st.point = point;
    st.timestamp = uptime();
    st.phase = if down { PHASE_MOVED } else { PHASE_ENDED };
    let all = e_all(e, sel("allTouches"));
    if down {
        msg2::<Id, Id, ()>(view, "touchesMoved:withEvent:", all, e);
    } else {
        msg2::<Id, Id, ()>(view, "touchesEnded:withEvent:", all, e);
        clear_direct();
    }
    let _ = nsstring;
}

/// Opt-in smoke-test diagnostic (PEAR_TEST_LEGACY_TOUCH): one tap at the
/// centre of the reattached legacy window, without posting system events.
#[export_name = "_Z32PearRunLegacyDirectTouchSelfTestv"]
pub extern "C" fn run_legacy_direct_touch_self_test() {
    if !rt::env_set("PEAR_TEST_LEGACY_TOUCH") {
        return;
    }
    rt::after_main(8.0, || {
        rt::log("[PearRuntime][mouse-touch] self-test tap begin");
        unsafe { send_legacy_direct_touch(true, 0.5, 0.5) };
        rt::after_main(0.2, || {
            unsafe { send_legacy_direct_touch(false, 0.5, 0.5) };
            rt::log("[PearRuntime][mouse-touch] self-test tap end");
        });
    });
}
