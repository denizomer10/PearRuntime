//! Pan and swipe recognition for the host's single-pointer touch stream.

use super::*;

static TRACKING_KEY: u8 = 0;
static MIN_TOUCHES_KEY: u8 = 0;
static MAX_TOUCHES_KEY: u8 = 0;
static SCROLL_MASK_KEY: u8 = 0;
static DIRECTION_KEY: u8 = 0;

const MOVEMENT_THRESHOLD: f64 = 10.0;
const SWIPE_DISTANCE: f64 = 30.0;
const SWIPE_DURATION: f64 = 0.5;

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Tracking {
    origin: CGPoint,
    offset: CGPoint,
    previous: CGPoint,
    velocity: CGPoint,
    started: f64,
    timestamp: f64,
}

unsafe fn tracking(gesture: Id) -> Option<Tracking> {
    let bytes = nsdata_to_vec(assoc_get(gesture, &TRACKING_KEY))?;
    if bytes.len() != std::mem::size_of::<Tracking>() {
        return None;
    }
    Some(std::ptr::read_unaligned(bytes.as_ptr().cast()))
}

unsafe fn save_tracking(gesture: Id, state: &Tracking) {
    let bytes = std::slice::from_raw_parts(
        (state as *const Tracking).cast::<u8>(),
        std::mem::size_of::<Tracking>(),
    );
    assoc_set(gesture, &TRACKING_KEY, nsdata_from_bytes(bytes));
}

unsafe extern "C" fn min_touches(gesture: Id, _cmd: Sel) -> usize {
    pear_objc::assoc_i64(gesture, &MIN_TOUCHES_KEY, 1) as usize
}

unsafe extern "C" fn set_min_touches(gesture: Id, _cmd: Sel, count: usize) {
    pear_objc::assoc_set_i64(gesture, &MIN_TOUCHES_KEY, count as i64);
}

unsafe extern "C" fn max_touches(gesture: Id, _cmd: Sel) -> usize {
    pear_objc::assoc_i64(gesture, &MAX_TOUCHES_KEY, usize::MAX as i64) as usize
}

unsafe extern "C" fn set_max_touches(gesture: Id, _cmd: Sel, count: usize) {
    pear_objc::assoc_set_i64(gesture, &MAX_TOUCHES_KEY, count as i64);
}

unsafe extern "C" fn scroll_mask(gesture: Id, _cmd: Sel) -> usize {
    pear_objc::assoc_i64(gesture, &SCROLL_MASK_KEY, 0) as usize
}

unsafe extern "C" fn set_scroll_mask(gesture: Id, _cmd: Sel, mask: usize) {
    pear_objc::assoc_set_i64(gesture, &SCROLL_MASK_KEY, mask as i64);
}

unsafe extern "C" fn direction(gesture: Id, _cmd: Sel) -> usize {
    pear_objc::assoc_i64(gesture, &DIRECTION_KEY, 1) as usize
}

unsafe extern "C" fn set_direction(gesture: Id, _cmd: Sel, mask: usize) {
    pear_objc::assoc_set_i64(gesture, &DIRECTION_KEY, mask as i64);
}

fn difference(a: CGPoint, b: CGPoint) -> CGPoint {
    CGPoint {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

unsafe fn vector_in_view(gesture: Id, vector: CGPoint, view: Id) -> CGPoint {
    let source = gr_view(gesture, sel("view"));
    if source.is_null() || view == source {
        return vector;
    }
    let end: CGPoint = msg2(source, "convertPoint:toView:", vector, view);
    let start: CGPoint = msg2(source, "convertPoint:toView:", CGPoint::default(), view);
    difference(end, start)
}

unsafe extern "C" fn translation(gesture: Id, _cmd: Sel, view: Id) -> CGPoint {
    tracking(gesture).map_or(CGPoint::default(), |state| {
        let travel = difference(state.previous, state.origin);
        vector_in_view(gesture, difference(travel, state.offset), view)
    })
}

unsafe extern "C" fn set_translation(gesture: Id, _cmd: Sel, translation: CGPoint, view: Id) {
    let Some(mut state) = tracking(gesture) else {
        return;
    };
    let source = gr_view(gesture, sel("view"));
    let end: CGPoint = msg2(source, "convertPoint:fromView:", translation, view);
    let start: CGPoint = msg2(source, "convertPoint:fromView:", CGPoint::default(), view);
    state.offset = difference(
        difference(state.previous, state.origin),
        difference(end, start),
    );
    state.velocity = CGPoint::default();
    save_tracking(gesture, &state);
}

unsafe extern "C" fn velocity(gesture: Id, _cmd: Sel, view: Id) -> CGPoint {
    tracking(gesture).map_or(CGPoint::default(), |state| {
        vector_in_view(gesture, state.velocity, view)
    })
}

unsafe extern "C" fn location(gesture: Id, _cmd: Sel, view: Id) -> CGPoint {
    let point = gr_location(gesture, sel("locationInView:"), NIL);
    let source = gr_view(gesture, sel("view"));
    if source.is_null() || view == source {
        point
    } else {
        msg2(source, "convertPoint:toView:", point, view)
    }
}

unsafe extern "C" fn number_of_touches(gesture: Id, _cmd: Sel) -> usize {
    usize::from(tracking(gesture).is_some() && matches!(gr_state(gesture, sel("state")), 0..=2))
}

unsafe fn send_action(gesture: Id) {
    let targets = get_gr_targets(gesture);
    for entry in &targets {
        objc_retain(entry.target as Id);
    }
    let send: unsafe extern "C" fn(Id, Sel, Id) =
        std::mem::transmute(objc_msgSend as *const c_void);
    for entry in targets {
        let (target, action) = (entry.target as Id, entry.action as Sel);
        if msg1::<Sel, bool>(target, "respondsToSelector:", action) {
            send(target, action, gesture);
        }
        objc_release(target);
    }
}

unsafe fn update(gesture: Id, touch: Id, phase: isize, pan: bool) -> bool {
    let mut properties = get_gr_state(gesture);
    if !properties.enabled {
        assoc_set(gesture, &TRACKING_KEY, NIL);
        return false;
    }
    let view = gr_view(gesture, sel("view"));
    let point: CGPoint = msg1(touch, "locationInView:", view);
    let timestamp: f64 = msg0(touch, "timestamp");
    properties.location = point;

    if phase == 0 {
        assoc_set(gesture, &TRACKING_KEY, NIL);
        let allowed = if pan {
            min_touches(gesture, sel("minimumNumberOfTouches")) <= 1
                && max_touches(gesture, sel("maximumNumberOfTouches")) >= 1
        } else {
            properties.touches_required == 1
        };
        properties.state = if allowed { 0 } else { 5 };
        if allowed {
            save_tracking(
                gesture,
                &Tracking {
                    origin: point,
                    previous: point,
                    started: timestamp,
                    timestamp,
                    ..Tracking::default()
                },
            );
        }
        set_gr_state(gesture, properties);
        return false;
    }

    let Some(mut state) = tracking(gesture) else {
        return false;
    };
    let previous_state = properties.state;
    let delta = difference(point, state.previous);
    let elapsed = timestamp - state.timestamp;
    if elapsed > 0.0 && (phase == 1 || delta.x != 0.0 || delta.y != 0.0) {
        state.velocity = CGPoint {
            x: delta.x / elapsed,
            y: delta.y / elapsed,
        };
    }
    state.previous = point;
    state.timestamp = timestamp;
    save_tracking(gesture, &state);
    let travel = difference(point, state.origin);

    properties.state = if phase == 4 {
        if matches!(previous_state, 1 | 2) {
            4
        } else {
            5
        }
    } else if pan {
        match (phase, previous_state) {
            (1, 0) if travel.x.hypot(travel.y) >= MOVEMENT_THRESHOLD => 1,
            (1, 1 | 2) => 2,
            (3, 1 | 2) => 3,
            (3, _) => 5,
            _ => previous_state,
        }
    } else if phase == 3 {
        let swipe = if travel.x.abs() >= travel.y.abs() {
            if travel.x > 0.0 {
                1
            } else {
                2
            }
        } else if travel.y < 0.0 {
            4
        } else {
            8
        };
        if travel.x.hypot(travel.y) >= SWIPE_DISTANCE
            && timestamp - state.started <= SWIPE_DURATION
            && direction(gesture, sel("direction")) & swipe != 0
        {
            3
        } else {
            5
        }
    } else {
        previous_state
    };

    let delegate = gr_delegate(gesture, sel("delegate"));
    if matches!(properties.state, 1 | 3)
        && previous_state == 0
        && responds_to(delegate, "gestureRecognizerShouldBegin:")
        && !msg1::<Id, bool>(delegate, "gestureRecognizerShouldBegin:", gesture)
    {
        properties.state = 5;
    }
    set_gr_state(gesture, properties);
    if matches!(properties.state, 1..=4) {
        send_action(gesture);
    }
    if matches!(phase, 3 | 4) {
        assoc_set(gesture, &TRACKING_KEY, NIL);
    }
    properties.cancels_touches && matches!(properties.state, 1..=3)
}

pub(crate) unsafe fn deliver_pointer_gestures(mut view: Id, touch: Id, phase: isize) -> bool {
    let mut cancel = false;
    objc_retain(view);
    while !view.is_null() {
        let parent: Id = msg0(view, "superview");
        if !parent.is_null() {
            objc_retain(parent);
        }
        let attached = assoc_get(view, &VIEW_GESTURES_KEY);
        if !attached.is_null() && nsarray_count(attached) > 0 {
            let recognizers: Id = msg1(cls("NSArray"), "arrayWithArray:", attached);
            for index in 0..nsarray_count(recognizers) {
                let gesture = nsarray_object_at(recognizers, index);
                let pan = is_kind_of(gesture, "UIPanGestureRecognizer");
                if pan || is_kind_of(gesture, "UISwipeGestureRecognizer") {
                    cancel |= update(gesture, touch, phase, pan);
                }
            }
        }
        objc_release(view);
        view = parent;
    }
    cancel
}

pub(super) unsafe fn register() {
    let pan = cls("UIPanGestureRecognizer");
    if !pan.is_null() {
        add_method(pan, "minimumNumberOfTouches", min_touches as Imp, "Q@:");
        add_method(
            pan,
            "setMinimumNumberOfTouches:",
            set_min_touches as Imp,
            "v@:Q",
        );
        add_method(pan, "maximumNumberOfTouches", max_touches as Imp, "Q@:");
        add_method(
            pan,
            "setMaximumNumberOfTouches:",
            set_max_touches as Imp,
            "v@:Q",
        );
        add_method(pan, "allowedScrollTypesMask", scroll_mask as Imp, "Q@:");
        add_method(
            pan,
            "setAllowedScrollTypesMask:",
            set_scroll_mask as Imp,
            "v@:Q",
        );
        add_method(
            pan,
            "translationInView:",
            translation as Imp,
            "{CGPoint=dd}@:@",
        );
        add_method(
            pan,
            "setTranslation:inView:",
            set_translation as Imp,
            "v@:{CGPoint=dd}@",
        );
        add_method(pan, "velocityInView:", velocity as Imp, "{CGPoint=dd}@:@");
        add_method(pan, "locationInView:", location as Imp, "{CGPoint=dd}@:@");
        add_method(pan, "numberOfTouches", number_of_touches as Imp, "Q@:");
    }

    let swipe = cls("UISwipeGestureRecognizer");
    if !swipe.is_null() {
        add_method(swipe, "direction", direction as Imp, "Q@:");
        add_method(swipe, "setDirection:", set_direction as Imp, "v@:Q");
        add_method(
            swipe,
            "numberOfTouchesRequired",
            gr_touches_required as Imp,
            "Q@:",
        );
        add_method(
            swipe,
            "setNumberOfTouchesRequired:",
            gr_set_touches_required as Imp,
            "v@:Q",
        );
        add_method(swipe, "locationInView:", location as Imp, "{CGPoint=dd}@:@");
        add_method(swipe, "numberOfTouches", number_of_touches as Imp, "Q@:");
    }
}
