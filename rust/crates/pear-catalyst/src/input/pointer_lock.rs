//! Disables iPadOS pointer lock for the loaded app. Opt out with
//! PEAR_ALLOW_POINTER_LOCK=1; PEAR_DEBUG_POINTER=1 logs the classes touched.
//!
//! Never walks objc_copyClassList: some engines' class metadata can only be
//! realized after their own launch path. The live view-controller chain is the
//! only place the preference matters.

use crate::rt;
use pear_objc::{cls, msg0, msg1, nsarray_count, nsarray_object_at, sel, Class, Id, Imp, Sel};
use std::collections::HashSet;
use std::sync::Mutex;

static HANDLED: Mutex<Option<HashSet<usize>>> = Mutex::new(None);

unsafe extern "C" fn prefers_not_locked(_s: Id, _c: Sel) -> bool {
    false
}

extern "C" {
    fn method_getTypeEncoding(m: rt::Method) -> *const std::ffi::c_char;
}

fn allowed() -> bool {
    std::env::var("PEAR_ALLOW_POINTER_LOCK").map(|v| {
        // NSString.boolValue: leading Y/y/T/t or a non-zero integer.
        let t = v.trim_start();
        t.starts_with(['Y', 'y', 'T', 't']) || t.trim_start_matches(['+', '-', '0']).starts_with(|c: char| c.is_ascii_digit())
    }).unwrap_or(false)
}

/// Every view controller of every window scene, depth first.
unsafe fn visit_controllers(mut f: impl FnMut(Id)) {
    unsafe fn walk(vc: Id, f: &mut dyn FnMut(Id)) {
        if vc.is_null() {
            return;
        }
        f(vc);
        let children: Id = msg0(vc, "childViewControllers");
        for i in 0..nsarray_count(children) {
            walk(nsarray_object_at(children, i), f);
        }
        let presented: Id = msg0(vc, "presentedViewController");
        if !presented.is_null() {
            walk(presented, f);
        }
    }
    let app: Id = msg0(cls("UIApplication"), "sharedApplication");
    let scenes: Id = msg0(app, "connectedScenes");
    let all: Id = msg0(scenes, "allObjects");
    let window_scene: Class = cls("UIWindowScene");
    for i in 0..nsarray_count(all) {
        let scene = nsarray_object_at(all, i);
        if !msg1::<Class, bool>(scene, "isKindOfClass:", window_scene) {
            continue;
        }
        let windows: Id = msg0(scene, "windows");
        for j in 0..nsarray_count(windows) {
            walk(msg0(nsarray_object_at(windows, j), "rootViewController"), &mut f);
        }
    }
}

unsafe fn disable_pointer_lock() -> usize {
    if allowed() {
        return 0;
    }
    let selector = sel("prefersPointerLocked");
    let debug = rt::env_set("PEAR_DEBUG_POINTER");
    let mut swizzled = 0usize;
    let mut handled = HANDLED.lock().unwrap_or_else(|e| e.into_inner());
    let handled = handled.get_or_insert_with(HashSet::new);
    visit_controllers(|vc| {
        let c = rt::object_getClass(vc);
        let m = rt::class_getInstanceMethod(c, selector);
        if m.is_null() || !handled.insert(c as usize) {
            return;
        }
        // Override on the concrete controller instead of mutating an inherited
        // Method on UIKit's superclass.
        if !rt::class_addMethod(c, selector, prefers_not_locked as Imp, method_getTypeEncoding(m)) {
            rt::method_setImplementation(rt::class_getInstanceMethod(c, selector), prefers_not_locked as Imp);
        }
        swizzled += 1;
        if debug {
            rt::log(&format!("[PearRuntime][pointer] disabled prefersPointerLocked on {}", rt::class_name(c)));
        }
    });
    if swizzled > 0 {
        rt::log(&format!(
            "[PearRuntime][pointer] pointer lock disabled ({swizzled} method{})",
            if swizzled == 1 { "" } else { "s" }
        ));
    }
    swizzled
}

/// Asks UIKit to read the preference again on the live controller chain.
unsafe fn refresh_pointer_preference() {
    let mut updated = 0usize;
    visit_controllers(|vc| {
        if pear_objc::responds_to(vc, "setNeedsUpdateOfPrefersPointerLocked") {
            msg0::<()>(vc, "setNeedsUpdateOfPrefersPointerLocked");
            updated += 1;
        }
    });
    if rt::env_set("PEAR_DEBUG_POINTER") {
        rt::log(&format!(
            "[PearRuntime][pointer] refresh requested on {updated} controller{}",
            if updated == 1 { "" } else { "s" }
        ));
    }
}

fn pass() {
    unsafe {
        pear_objc::autorelease_pool(|| {
            disable_pointer_lock();
            refresh_pointer_preference();
        })
    }
}

pub fn init() {
    if !rt::module_enabled("input") || allowed() {
        return;
    }
    // Not from the dyld initializer itself: Swift classes may not be realized
    // yet. The first launch callbacks are early enough for a user click.
    for i in 1..=4 {
        rt::after_main(i as f64 * 0.75, pass);
    }
    unsafe {
        rt::observe_main("UIApplicationDidBecomeActiveNotification", |_| pass());
    }
}
