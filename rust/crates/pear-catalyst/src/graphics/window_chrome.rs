//! Keeps an iOS game's content out from under the Mac title bar.
//!
//! Catalyst hosts the UIKit scene in an AppKit window whose content view uses
//! the full window frame, so the top ~28 points are drawn behind the title
//! bar, where games put HUD elements and buttons. The scene view is placed in
//! the window's content layout rect instead; UIKit then reports a window that
//! is exactly the visible area. Full screen gets the whole window. Disable
//! with PEAR_KEEP_TITLEBAR_OVERLAP=1. Closing the last game window ends the
//! process, as the launcher expects.

use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsarray_count, nsarray_object_at, nsstring, sel, CGRect, Id, Imp, Sel, NIL};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

const FULL_SCREEN_MASK: usize = 1 << 14; // NSWindowStyleMaskFullScreen
const FULL_SIZE_CONTENT_MASK: usize = 1 << 15; // NSWindowStyleMaskFullSizeContentView

static ORIGINAL_SET_FRAME: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static CLAMP_INSTALLED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn method_getTypeEncoding(m: rt::Method) -> *const std::ffi::c_char;
    fn kill(pid: i32, sig: i32) -> i32;
    fn getpid() -> i32;
    fn NSStringFromCGRect(r: CGRect) -> Id;
}

unsafe fn is_scene_window(window: Id) -> bool {
    if window.is_null() {
        return false;
    }
    let mut c = rt::object_getClass(window);
    while !c.is_null() {
        if rt::class_name(c) == "UINSWindow" {
            return true;
        }
        c = rt::class_getSuperclass(c);
    }
    false
}

/// The AppKit view that hosts the UIKit scene (UINSSceneView).
unsafe fn scene_view(content: Id) -> Id {
    let scene = cls("UINSSceneView");
    if content.is_null() || scene.is_null() {
        return NIL;
    }
    if msg1::<Id, bool>(content, "isKindOfClass:", scene) {
        return content;
    }
    let children: Id = msg0(content, "subviews");
    for i in 0..nsarray_count(children) {
        let child = nsarray_object_at(children, i);
        if msg1::<Id, bool>(child, "isKindOfClass:", scene) {
            return child;
        }
    }
    NIL
}

/// The content layout rect (below the title bar) in content-view coordinates,
/// or None when no title bar overlaps (full screen, no full-size content).
unsafe fn target_frame(window: Id, content: Id) -> Option<CGRect> {
    if !is_scene_window(window) {
        return None;
    }
    let mask: usize = msg0(window, "styleMask");
    if mask & FULL_SCREEN_MASK != 0 || mask & FULL_SIZE_CONTENT_MASK == 0 {
        return None;
    }
    if content.is_null() || !pear_objc::responds_to(window, "contentLayoutRect") {
        return None;
    }
    let visible: CGRect = msg0(window, "contentLayoutRect");
    let container: CGRect = msg0(content, "bounds");
    if visible.size.width < 1.0 || visible.size.height < 1.0 || visible.size.height >= container.size.height - 0.5 {
        return None;
    }
    // contentLayoutRect is in window coordinates (bottom-left origin); only a
    // flipped content view needs converting.
    let mut target = visible;
    if msg0::<bool>(content, "isFlipped") {
        target.origin.y = container.size.height - (visible.origin.y + visible.size.height);
    }
    Some(target)
}

/// Catalyst lays the scene view out over the whole window on every pass, so
/// the frame is clamped where it is set rather than fixed once.
unsafe extern "C" fn scene_set_frame(s: Id, c: Sel, frame: CGRect) {
    let window: Id = if pear_objc::responds_to(s, "window") { msg0(s, "window") } else { NIL };
    let superview: Id = msg0(s, "superview");
    let frame = target_frame(window, superview).unwrap_or(frame);
    let f: unsafe extern "C" fn(Id, Sel, CGRect) = std::mem::transmute(ORIGINAL_SET_FRAME.load(Ordering::Acquire));
    f(s, c, frame);
}

unsafe fn install_scene_frame_clamp() {
    if CLAMP_INSTALLED.load(Ordering::Acquire) {
        return;
    }
    let scene = cls("UINSSceneView");
    if scene.is_null() {
        return;
    }
    let m = rt::class_getInstanceMethod(scene, sel("setFrame:"));
    if m.is_null() {
        return;
    }
    CLAMP_INSTALLED.store(true, Ordering::Release);
    ORIGINAL_SET_FRAME.store(rt::method_getImplementation(m) as *mut c_void, Ordering::Release);
    // On the class itself, so a superclass implementation is not replaced for
    // unrelated views.
    if !rt::class_addMethod(scene, sel("setFrame:"), scene_set_frame as Imp, method_getTypeEncoding(m)) {
        rt::method_setImplementation(m, scene_set_frame as Imp);
    }
    rt::log("[PearRuntime][window] scene view kept below the title bar");
}

unsafe fn fix_titlebar_overlap(window: Id) {
    if !is_scene_window(window) {
        return;
    }
    install_scene_frame_clamp();
    let content: Id = msg0(window, "contentView");
    let scene = scene_view(content);
    match (scene.is_null(), target_frame(window, content)) {
        (false, Some(target)) => {
            let current: CGRect = msg0(scene, "frame");
            if current == target {
                return;
            }
            msg1::<CGRect, ()>(scene, "setFrame:", target);
            if rt::env_set("PEAR_DEBUG_VIEWS") {
                rt::log(&format!(
                    "[PearRuntime][window] scene placed below the title bar: {}",
                    rt::ns_string(NSStringFromCGRect(target))
                ));
            }
        }
        (false, None) if !content.is_null() => {
            // Full screen: give the scene the whole window back.
            let bounds: CGRect = msg0(content, "bounds");
            let current: CGRect = msg0(scene, "frame");
            if current != bounds {
                msg1::<CGRect, ()>(scene, "setFrame:", bounds);
            }
        }
        _ => {}
    }
}

unsafe fn app_windows() -> Id {
    let app_class = cls("NSApplication");
    if app_class.is_null() || !pear_objc::responds_to(app_class, "sharedApplication") {
        return NIL;
    }
    let app: Id = msg0(app_class, "sharedApplication");
    if !pear_objc::responds_to(app, "windows") {
        return NIL;
    }
    msg0(app, "windows")
}

unsafe fn fix_all_windows() {
    let windows = app_windows();
    for i in 0..nsarray_count(windows) {
        fix_titlebar_overlap(nsarray_object_at(windows, i));
    }
}

unsafe fn on_window_will_close(note: Id) {
    let closing: Id = msg0(note, "object");
    if !is_scene_window(closing) {
        return;
    }
    let closing = closing as usize; // identity only
    rt::after_main(0.5, move || unsafe {
        let app: Id = msg0(cls("NSApplication"), "sharedApplication");
        let windows: Id = msg0(app, "windows");
        for i in 0..nsarray_count(windows) {
            let w = nsarray_object_at(windows, i);
            if w as usize == closing || !is_scene_window(w) {
                continue;
            }
            if msg0::<bool>(w, "isVisible") || msg0::<bool>(w, "isMiniaturized") {
                return;
            }
        }
        let center: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        msg2::<Id, Id, ()>(center, "postNotificationName:object:", nsstring("NSApplicationWillTerminateNotification"), app);
        rt::log("[PearRuntime][window] last game window closed; terminating owned process");
        kill(getpid(), 9);
    });
}

pub fn init() {
    if !rt::module_enabled("graphics") {
        return;
    }
    unsafe {
        rt::observe_main("NSWindowWillCloseNotification", |note| on_window_will_close(note));
        if rt::env_set("PEAR_KEEP_TITLEBAR_OVERLAP") {
            return;
        }
        // Resizable UIKit apps already lay out safe areas in their native
        // Catalyst host; the game clamp would shift their content twice.
        let info: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "infoDictionary");
        let full: Id = msg1(info, "objectForKey:", nsstring("UIRequiresFullScreen"));
        if !full.is_null()
            && pear_objc::is_kind_of(full, "NSNumber")
            && !msg0::<bool>(full, "boolValue")
            && !rt::env_set("PEAR_LEGACY_ATTACH_LAYER")
        {
            return;
        }
        install_scene_frame_clamp();
        // Catalyst can restore the style when the scene updates its title bar
        // or leaves full screen, so re-apply on those events.
        for name in [
            "NSWindowDidBecomeKeyNotification",
            "NSWindowDidResizeNotification",
            "NSWindowDidEndLiveResizeNotification",
            "NSWindowDidEnterFullScreenNotification",
            "NSWindowDidExitFullScreenNotification",
        ] {
            rt::observe_main(name, |note| {
                let window: Id = msg0(note, "object");
                fix_titlebar_overlap(window);
                let w = pear_objc::objc_retain(window) as usize;
                rt::async_main(move || {
                    fix_titlebar_overlap(w as Id);
                    pear_objc::objc_release(w as Id);
                });
            });
        }
    }
    for delay in [0.2, 1.0, 3.0] {
        rt::after_main(delay, || unsafe { fix_all_windows() });
    }
}
