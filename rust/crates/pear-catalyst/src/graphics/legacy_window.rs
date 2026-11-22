//! Scene-less UIKit window compatibility for legacy iPhone apps.
//!
//! Apps without a scene manifest (or built against an SDK older than iOS 15)
//! can hit UINSWindow's "should have proxy before assigning wc" assertion; the
//! pre-proxy minimum-size pass is skipped and UIKit attaches the window. An
//! empty (0x0) host window left by that workaround is repaired. With
//! PEAR_LEGACY_ATTACH_LAYER the UIKit layer tree is attached to the AppKit
//! host and fitted (letterboxed at the launch aspect) into the visible area.

use crate::geometry::{aspect_fit_rect, rect_is_empty};
use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsarray_count, nsarray_object_at, nsstring, objc_getAssociatedObject, objc_setAssociatedObject, sel, CGPoint, CGRect, CGSize, Id, Imp, Sel, NIL};
use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU64, Ordering};
use std::sync::Mutex;

extern "C" {
    #[link_name = "_Z26PearGLCopyFixedLogicalSizeP6CGSize"]
    fn PearGLCopyFixedLogicalSize(size: *mut CGSize) -> bool;
    fn method_getTypeEncoding(m: rt::Method) -> *const std::ffi::c_char;
    fn CATransform3DMakeScale(sx: f64, sy: f64, sz: f64) -> Transform3D;
    fn CATransform3DIsIdentity(t: Transform3D) -> bool;
    fn NSStringFromCGRect(r: CGRect) -> Id;
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Transform3D {
    m: [f64; 16],
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_UPDATE_SUBTREE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ATTACHED_ROOT_KEY: u8 = 0;
static DESIGN_ASPECT: AtomicU64 = AtomicU64::new(0); // f64 bits
static BUTTON_LOGS: AtomicI32 = AtomicI32::new(0);
static DRAG_CLASSES: Mutex<Option<HashSet<usize>>> = Mutex::new(None);

unsafe fn info_value(key: &str) -> Id {
    let info: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "infoDictionary");
    if info.is_null() { NIL } else { msg1(info, "objectForKey:", nsstring(key)) }
}

unsafe fn is_legacy_app() -> bool {
    if rt::env_bool("PEAR_LEGACY_WINDOW_FIX") {
        return true;
    }
    if !info_value("UIApplicationSceneManifest").is_null() {
        return false;
    }
    let sdk = info_value("DTSDKName");
    let sdk = if sdk.is_null() { String::new() } else { rt::ns_string(sdk).to_lowercase() };
    let version = sdk.strip_prefix("iphoneos").unwrap_or(&sdk);
    let major = rt::ns_string_double(version);
    major > 0.0 && major < 15.0
}

/// The part of a window's content view below the title bar, in the content
/// view's coordinates (the whole view in full screen).
#[export_name = "_Z28PearWindowVisibleContentRectP11objc_objectS0_"]
pub unsafe extern "C" fn window_visible_content_rect(window: Id, content: Id) -> CGRect {
    if content.is_null() {
        return CGRect::default();
    }
    let bounds: CGRect = msg0(content, "bounds");
    if window.is_null() || !pear_objc::responds_to(window, "contentLayoutRect") {
        return bounds;
    }
    let mask: usize = msg0(window, "styleMask");
    if mask & (1 << 14) != 0 {
        return bounds;
    }
    let layout: CGRect = msg0(window, "contentLayoutRect");
    let converted: CGRect = msg2(content, "convertRect:fromView:", layout, NIL);
    let visible = intersection(converted, bounds);
    if visible.size.width >= 1.0 && visible.size.height >= 1.0 { visible } else { bounds }
}

fn intersection(a: CGRect, b: CGRect) -> CGRect {
    let x0 = a.origin.x.max(b.origin.x);
    let y0 = a.origin.y.max(b.origin.y);
    let x1 = (a.origin.x + a.size.width).min(b.origin.x + b.size.width);
    let y1 = (a.origin.y + a.size.height).min(b.origin.y + b.size.height);
    if x1 <= x0 || y1 <= y0 {
        return CGRect::default();
    }
    CGRect { origin: CGPoint { x: x0, y: y0 }, size: CGSize { width: x1 - x0, height: y1 - y0 } }
}

unsafe extern "C" fn skip_min_size(_s: Id, _c: Sel) {
    rt::log("[PearRuntime][window] skipped pre-proxy content min size");
}
unsafe extern "C" fn skip_container_constraints(_s: Id, _c: Sel) {
    rt::log("[PearRuntime][window] skipped scene container constraints");
}

/// AppKit alert panels and their glass host need normal layout; everything
/// else in a legacy window skips subtree constraint passes.
unsafe extern "C" fn update_subtree(s: Id, c: Sel) {
    let window: Id = if pear_objc::responds_to(s, "window") { msg0(s, "window") } else { NIL };
    let window_class = if window.is_null() { "" } else { rt::class_name(rt::object_getClass(window)) };
    if window_class.contains("NSAlertPanel") || window_class.contains("NSGlassEffectWindow") {
        let f: unsafe extern "C" fn(Id, Sel) = std::mem::transmute(ORIGINAL_UPDATE_SUBTREE.load(Ordering::Acquire));
        f(s, c);
    } else if rt::env_set("PEAR_DEBUG_VIEWS") {
        rt::log(&format!("[PearRuntime][window] skipped legacy subtree constraints on {}", rt::class_name(rt::object_getClass(s))));
    }
}

unsafe fn install_now() {
    if INSTALLED.load(Ordering::Acquire) {
        return;
    }
    let window = cls("UINSWindow");
    let m = if window.is_null() { std::ptr::null_mut() } else { rt::class_getInstanceMethod(window, sel("_setContentMinSizeFromScene")) };
    if m.is_null() {
        return;
    }
    rt::method_setImplementation(m, skip_min_size as Imp);
    let container = cls("UINSSceneContainerView");
    let constraints = if container.is_null() { std::ptr::null_mut() } else { rt::class_getInstanceMethod(container, sel("updateConstraints")) };
    if !constraints.is_null() && !rt::env_bool("PEAR_LEGACY_CONTAINER_LAYOUT") {
        rt::method_setImplementation(constraints, skip_container_constraints as Imp);
    }
    let ns_view = cls("NSView");
    let update = if ns_view.is_null() { std::ptr::null_mut() } else { rt::class_getInstanceMethod(ns_view, sel("updateConstraintsForSubtreeIfNeeded")) };
    if !update.is_null() {
        ORIGINAL_UPDATE_SUBTREE.store(rt::method_getImplementation(update) as *mut c_void, Ordering::Release);
        rt::method_setImplementation(update, update_subtree as Imp);
    }
    INSTALLED.store(true, Ordering::Release);
    rt::log(&format!(
        "[PearRuntime][window] legacy window fix installed (constraints={}, subtree={})",
        (!constraints.is_null()) as i32,
        (!update.is_null()) as i32
    ));
}

unsafe extern "C" fn no_drag(_s: Id, _c: Sel) -> bool {
    false
}

unsafe fn log_buttons(window: Id) {
    if BUTTON_LOGS.fetch_add(1, Ordering::Relaxed) % 300 != 0 || !pear_objc::responds_to(window, "standardWindowButton:") {
        return;
    }
    for kind in 0..3isize {
        let button: Id = msg1(window, "standardWindowButton:", kind);
        if button.is_null() {
            continue;
        }
        let frame: CGRect = msg0(button, "frame");
        let enabled: bool = msg0(button, "isEnabled");
        let superview: Id = msg0(button, "superview");
        let super_frame: CGRect = if superview.is_null() { CGRect::default() } else { msg0(superview, "frame") };
        rt::log(&format!(
            "[PearRuntime][window] button {kind} enabled={} frame={} in {} {}",
            enabled as i32,
            rt::ns_string(NSStringFromCGRect(frame)),
            if superview.is_null() { "(null)" } else { rt::class_name(rt::object_getClass(superview)) },
            rt::ns_string(NSStringFromCGRect(super_frame))
        ));
    }
}

/// Logical size override "WxH" (100…8192), as PEAR_LEGACY_LOGICAL_SIZE.
fn logical_override() -> Option<Option<CGSize>> {
    let v = std::env::var("PEAR_LEGACY_LOGICAL_SIZE").ok()?;
    let lower = v.to_lowercase();
    let parts: Vec<&str> = lower.split('x').collect();
    if parts.len() != 2 {
        return None;
    }
    let (w, h) = (rt::ns_string_double(parts[0]), rt::ns_string_double(parts[1]));
    let valid = w.is_finite() && h.is_finite() && (100.0..=8192.0).contains(&w) && (100.0..=8192.0).contains(&h);
    Some(valid.then_some(CGSize { width: w, height: h }))
}

unsafe fn apply_logical_size(ui_window: Id, logical: CGSize) {
    rt::without_actions(|| {
        let b: CGRect = msg0(ui_window, "bounds");
        if b.size != logical {
            msg1::<CGRect, ()>(ui_window, "setBounds:", CGRect { origin: Default::default(), size: logical });
        }
        let root: Id = msg0(ui_window, "rootViewController");
        let content: Id = if !root.is_null() && msg0::<bool>(root, "isViewLoaded") { msg0(root, "view") } else { NIL };
        if !content.is_null() && msg0::<CGRect>(content, "bounds").size != logical {
            msg1::<CGRect, ()>(content, "setFrame:", CGRect { origin: Default::default(), size: logical });
        }
    });
}

unsafe fn attach_layer(window: Id, ui_window: Id) {
    let view: Id = if pear_objc::responds_to(window, "contentView") { msg0(window, "contentView") } else { NIL };
    if pear_objc::responds_to(view, "setWantsLayer:") {
        msg1::<bool, ()>(view, "setWantsLayer:", true);
    }
    // Once the scene carries game content, drags belong to it.
    if pear_objc::responds_to(window, "setMovableByWindowBackground:") {
        msg1::<bool, ()>(window, "setMovableByWindowBackground:", false);
    }
    if !view.is_null() {
        let view_class = rt::object_getClass(view);
        let mut set = DRAG_CLASSES.lock().unwrap_or_else(|e| e.into_inner());
        let set = set.get_or_insert_with(HashSet::new);
        if !set.contains(&(view_class as usize)) {
            let m = rt::class_getInstanceMethod(view_class, sel("mouseDownCanMoveWindow"));
            if !m.is_null() {
                if !rt::class_addMethod(view_class, sel("mouseDownCanMoveWindow"), no_drag as Imp, method_getTypeEncoding(m)) {
                    rt::method_setImplementation(m, no_drag as Imp);
                }
                set.insert(view_class as usize);
            }
        }
    }
    let host: Id = if pear_objc::responds_to(view, "layer") { msg0(view, "layer") } else { NIL };
    let root: Id = msg0(ui_window, "layer");
    if host.is_null() || root.is_null() {
        return;
    }
    let key = &ATTACHED_ROOT_KEY as *const u8 as *const c_void;
    let mut top: Id = objc_getAssociatedObject(host, key);
    if top.is_null() {
        top = root;
        if !rt::env_bool("PEAR_LEGACY_WINDOW_LAYER_ONLY") {
            loop {
                let up: Id = msg0(top, "superlayer");
                if up.is_null() {
                    break;
                }
                top = up;
            }
        }
        if top == host || nsarray_count(msg0(host, "sublayers")) != 0 {
            return;
        }
        msg1::<Id, ()>(host, "addSublayer:", top);
        objc_setAssociatedObject(host, key, top, 1);
        rt::log(&format!("[PearRuntime][window] attached UIKit root layer {} to legacy host", rt::class_name(rt::object_getClass(top))));
    }
    // The logical size is the launch aspect fitted into the visible window
    // area (never read back from the scaled UIWindow, which drifts), applied
    // through UIKit; the layer tree is scaled and centered, not resized.
    let host_bounds: CGRect = msg0(host, "bounds");
    if f64::from_bits(DESIGN_ASPECT.load(Ordering::Acquire)) <= 0.0 && host_bounds.size.width >= 1.0 && host_bounds.size.height >= 1.0 {
        let ui_bounds: CGRect = msg0(ui_window, "bounds");
        let identity = CATransform3DIsIdentity(msg0::<Transform3D>(top, "transform"));
        let initial = if identity && ui_bounds.size.height >= 1.0 { ui_bounds.size } else { host_bounds.size };
        let aspect = initial.width / initial.height;
        DESIGN_ASPECT.store(aspect.to_bits(), Ordering::Release);
        rt::log(&format!("[PearRuntime][window] legacy design aspect {aspect:.4} ({:.0}x{:.0})", initial.width, initial.height));
    }
    let mut area = window_visible_content_rect(window, view);
    if area.size.width < 1.0 || area.size.height < 1.0 {
        area = host_bounds;
    }
    let aspect = f64::from_bits(DESIGN_ASPECT.load(Ordering::Acquire));
    let design_fit = aspect_fit_rect(CGSize { width: aspect * 1000.0, height: 1000.0 }, area);
    let mut logical = CGSize { width: design_fit.size.width.round(), height: design_fit.size.height.round() };
    // Once a GL drawable exists its size owns the coordinate space.
    PearGLCopyFixedLogicalSize(&mut logical);
    if logical.width < 1.0 || logical.height < 1.0 {
        return;
    }
    match logical_override() {
        Some(Some(size)) => {
            logical = size;
            apply_logical_size(ui_window, logical);
        }
        Some(None) => {}
        None => apply_logical_size(ui_window, logical),
    }
    let fitted = aspect_fit_rect(logical, area);
    if rect_is_empty(fitted) {
        return;
    }
    let scale = fitted.size.width / logical.width;
    let center = CGPoint { x: fitted.origin.x + fitted.size.width / 2.0, y: fitted.origin.y + fitted.size.height / 2.0 };
    let t: Transform3D = msg0(top, "transform");
    let top_bounds: CGRect = msg0(top, "bounds");
    let position: CGPoint = msg0(top, "position");
    if top_bounds.size == logical && position == center && (t.m[0] - scale).abs() < 0.0001 && (t.m[5] - scale).abs() < 0.0001 {
        return;
    }
    rt::without_actions(|| {
        // The detached top layer may have host-sized bounds while its UIKit
        // descendants still use the logical space: restore it, then fit.
        msg1::<CGRect, ()>(top, "setBounds:", CGRect { origin: Default::default(), size: logical });
        msg1::<CGPoint, ()>(top, "setPosition:", center);
        msg1::<Transform3D, ()>(top, "setTransform:", CATransform3DMakeScale(scale, scale, 1.0));
    });
    if rt::env_set("PEAR_DEBUG_VIEWS") {
        rt::log(&format!(
            "[PearRuntime][window] legacy fit bounds={} host={} scale={scale:.4}",
            rt::ns_string(NSStringFromCGRect(msg0(top, "bounds"))),
            rt::ns_string(NSStringFromCGRect(host_bounds))
        ));
    }
}

/// Repairs only empty UINSWindows, never one the user or app already sized.
unsafe fn repair_empty_legacy_windows() {
    let window_class = cls("UINSWindow");
    let app_class = cls("NSApplication");
    let app: Id = if !app_class.is_null() && pear_objc::responds_to(app_class, "sharedApplication") { msg0(app_class, "sharedApplication") } else { NIL };
    if window_class.is_null() || app.is_null() {
        return;
    }
    let ui_window = rt::primary_window();
    let mut preferred = if ui_window.is_null() { CGSize::default() } else { msg0::<CGRect>(ui_window, "bounds").size };
    if preferred.width < 100.0 || preferred.height < 100.0 {
        preferred = CGSize { width: 1024.0, height: 768.0 };
    }
    let windows: Id = msg0(app, "windows");
    for i in 0..nsarray_count(windows) {
        let window = nsarray_object_at(windows, i);
        if !msg1::<pear_objc::Class, bool>(window, "isKindOfClass:", window_class) || !pear_objc::responds_to(window, "frame") {
            continue;
        }
        let frame: CGRect = msg0(window, "frame");
        let empty = frame.size.width <= 1.0 || frame.size.height <= 1.0;
        if empty && pear_objc::responds_to(window, "setContentSize:") {
            msg1::<CGSize, ()>(window, "setContentSize:", preferred);
        }
        let mut after: CGRect = msg0(window, "frame");
        if empty && (after.size.width <= 1.0 || after.size.height <= 1.0) && pear_objc::responds_to(window, "setFrame:display:") {
            after.size = preferred;
            msg2::<CGRect, bool, ()>(window, "setFrame:display:", after, true);
        }
        if empty {
            rt::log(&format!("[PearRuntime][window] repaired empty legacy host window to {:.0}x{:.0}", preferred.width, preferred.height));
        }
        if rt::env_set("PEAR_DEBUG_VIEWS") {
            log_buttons(window);
        }
        // Diagnostic experiment only (see module docs).
        if rt::env_bool("PEAR_LEGACY_ATTACH_LAYER") && !ui_window.is_null() {
            attach_layer(window, ui_window);
        }
    }
}

extern "C" fn geometry_tick(_: *mut c_void) {
    unsafe { repair_empty_legacy_windows() }
}

#[export_name = "_Z26PearInstallLegacyWindowFixv"]
pub unsafe extern "C" fn install_legacy_window_fix() {
    if !is_legacy_app() {
        return;
    }
    rt::log("[PearRuntime][window] legacy window fix requested");
    // Before UIApplicationMain can create a window; retried on the main queue
    // in case the class is not loaded yet.
    if msg0::<bool>(cls("NSThread"), "isMainThread") {
        install_now();
    }
    rt::async_main(|| unsafe { install_now() });
    rt::observe_main("NSWindowDidBecomeKeyNotification", |_| unsafe { repair_empty_legacy_windows() });
    rt::observe_main("NSWindowDidResizeNotification", |_| unsafe { repair_empty_legacy_windows() });
    // UIKit can settle its logical surface after the native resize callback:
    // follow it through live resizes and full-screen transitions.
    if rt::env_bool("PEAR_LEGACY_ATTACH_LAYER") {
        static TIMER: AtomicBool = AtomicBool::new(false);
        if !TIMER.swap(true, Ordering::AcqRel) {
            rt::main_timer(1.0, 1.0 / 30.0, 1.0 / 300.0, geometry_tick);
        }
    }
    for delay in [1.0, 3.0, 8.0, 15.0, 30.0] {
        rt::after_main(delay, || unsafe { repair_empty_legacy_windows() });
    }
    for delay in [0.05, 0.5, 2.0] {
        rt::after_main(delay, || unsafe { install_now() });
    }
}
