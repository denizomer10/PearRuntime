//! Geometry and touch bridge for the ANGLE renderer: fits the app's drawable
//! into the Mac window, crops design-resolution viewports, maps window-space
//! touches back into the space the engine laid its controls out in, and
//! counts presented frames. System OpenGLES is never queried.

use super::eagl;
use super::routing::{game_symbol, routing_enabled};
use crate::graphics::screen::{clear_screen_override, set_screen_override};
use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsstring, objc_release, objc_retain, CGPoint, CGRect, CGSize, Id, Imp, Sel};
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const NIL: Id = std::ptr::null_mut();

fn debug() -> bool {
    static D: OnceLock<bool> = OnceLock::new();
    *D.get_or_init(|| rt::env_set("PEAR_DEBUG_GL"))
}

extern "C" {
    fn NSStringFromCGRect(r: CGRect) -> Id;
    fn NSStringFromCGSize(s: CGSize) -> Id;
    fn NSStringFromCGPoint(p: CGPoint) -> Id;
    fn CFAbsoluteTimeGetCurrent() -> f64;
}

unsafe fn rect_s(r: CGRect) -> String {
    rt::ns_string(NSStringFromCGRect(r))
}
unsafe fn size_s(s: CGSize) -> String {
    rt::ns_string(NSStringFromCGSize(s))
}
unsafe fn point_s(p: CGPoint) -> String {
    rt::ns_string(NSStringFromCGPoint(p))
}

/// A fixed-orientation app can create its first drawable with UIKit's stale
/// opposite orientation: respect an unambiguous declaration.
fn orient_drawable_size(size: CGSize, landscape: bool, portrait: bool) -> CGSize {
    if size.width <= 0.0 || size.height <= 0.0 || landscape == portrait {
        return size;
    }
    if (landscape && size.width < size.height) || (portrait && size.width > size.height) {
        return CGSize { width: size.height, height: size.width };
    }
    size
}

/// Centers content in a container without cropping or changing its aspect.
fn aspect_fit_rect(content: CGSize, container: CGRect) -> CGRect {
    if content.width <= 0.0 || content.height <= 0.0 || container.size.width <= 0.0 || container.size.height <= 0.0 {
        return CGRect::default();
    }
    let scale = (container.size.width / content.width).min(container.size.height / content.height);
    let fitted = CGSize { width: content.width * scale, height: content.height * scale };
    CGRect {
        origin: CGPoint {
            x: container.origin.x + container.size.width / 2.0 - fitted.width * 0.5,
            y: container.origin.y + container.size.height / 2.0 - fitted.height * 0.5,
        },
        size: fitted,
    }
}

unsafe fn declared_drawable_size(size: CGSize) -> CGSize {
    static ORIENTATIONS: OnceLock<(bool, bool)> = OnceLock::new();
    let (landscape, portrait) = *ORIENTATIONS.get_or_init(|| unsafe {
        let info: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "infoDictionary");
        let (mut landscape, mut portrait) = (false, false);
        // A conflicting phone/tablet declaration is deliberately ambiguous.
        for key in ["UISupportedInterfaceOrientations", "UISupportedInterfaceOrientations~ipad"] {
            let list: Id = if info.is_null() { NIL } else { msg1(info, "objectForKey:", nsstring(key)) };
            if list.is_null() || !pear_objc::is_kind_of(list, "NSArray") {
                continue;
            }
            for i in 0..pear_objc::nsarray_count(list) {
                let v = pear_objc::nsarray_object_at(list, i);
                if !pear_objc::is_kind_of(v, "NSString") {
                    continue;
                }
                match rt::ns_string(v).as_str() {
                    "UIInterfaceOrientationLandscapeLeft" | "UIInterfaceOrientationLandscapeRight" => landscape = true,
                    "UIInterfaceOrientationPortrait" | "UIInterfaceOrientationPortraitUpsideDown" => portrait = true,
                    _ => {}
                }
            }
        }
        (landscape, portrait)
    });
    orient_drawable_size(size, landscape, portrait)
}

/// Shared geometry state (render thread and main queue).
struct Geo {
    gl_drawable: CGSize,
    native: CGSize,
    last_crop: [i32; 4],
    last_present: [i32; 4],
    present_count: u64,
    crop_applied: bool,
    seen_generation: u32,
    settle_frames: i32,
    fit_applied: bool,
    last_fit_window: CGSize,
    last_fit_layer: CGSize,
    last_fit_scale: f64,
    reference_window: CGSize,
    pending_window: CGSize,
    pending_since: f64,
}

static GEO: Mutex<Geo> = Mutex::new(Geo {
    gl_drawable: CGSize { width: 0.0, height: 0.0 },
    native: CGSize { width: 0.0, height: 0.0 },
    last_crop: [-1; 4],
    last_present: [-1; 4],
    present_count: 0,
    crop_applied: false,
    seen_generation: 0,
    settle_frames: 0,
    fit_applied: false,
    last_fit_window: CGSize { width: 0.0, height: 0.0 },
    last_fit_layer: CGSize { width: 0.0, height: 0.0 },
    last_fit_scale: 0.0,
    reference_window: CGSize { width: 0.0, height: 0.0 },
    pending_window: CGSize { width: 0.0, height: 0.0 },
    pending_since: 0.0,
});

fn geo() -> std::sync::MutexGuard<'static, Geo> {
    GEO.lock().unwrap_or_else(|e| e.into_inner())
}

/// The drawable layer, held weakly (`__weak CALayer *`).
static LAYER_WEAK: AtomicUsize = AtomicUsize::new(0);

fn layer_cell() -> *mut Id {
    let p = LAYER_WEAK.load(Ordering::Acquire);
    if p != 0 {
        return p as *mut Id;
    }
    let cell = Box::into_raw(Box::new(NIL));
    match LAYER_WEAK.compare_exchange(0, cell as usize, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => cell,
        Err(existing) => existing as *mut Id,
    }
}

unsafe fn drawable_layer() -> Id {
    let o = pear_objc::objc_loadWeakRetained(layer_cell());
    if o.is_null() { o } else { pear_objc::autoreleased(o) }
}

unsafe fn set_drawable_layer(layer: Id) {
    pear_objc::objc_storeWeak(layer_cell(), layer);
}

static CLIENT_VERSION: AtomicUsize = AtomicUsize::new(2);
static FIXED_LOGICAL: Mutex<CGSize> = Mutex::new(CGSize { width: 0.0, height: 0.0 });

#[export_name = "_Z26PearGLCopyFixedLogicalSizeP6CGSize"]
pub unsafe extern "C" fn copy_fixed_logical_size(size: *mut CGSize) -> bool {
    let value = *FIXED_LOGICAL.lock().unwrap_or_else(|e| e.into_inner());
    if value.width < 1.0 || value.height < 1.0 {
        return false;
    }
    *size = value;
    true
}

/// Bumped (from any thread) whenever the window geometry may have changed: the
/// present path re-fits only after a bump and for a short settling period.
static GEOMETRY_GENERATION: AtomicU32 = AtomicU32::new(1);

fn geometry_changed() {
    GEOMETRY_GENERATION.fetch_add(1, Ordering::Relaxed);
}

fn observe_geometry() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let center: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        for name in [
            "NSWindowDidResizeNotification",
            "NSWindowDidEnterFullScreenNotification",
            "NSWindowDidExitFullScreenNotification",
            "NSWindowDidChangeBackingPropertiesNotification",
            "UIWindowDidBecomeKeyNotification",
            "UISceneDidActivateNotification",
        ] {
            let block = rt::block1(|_| geometry_changed());
            let token: Id = pear_objc::msg4(center, "addObserverForName:object:queue:usingBlock:", nsstring(name), NIL, NIL, block);
            objc_retain(token);
            rt::release_block(block);
        }
    });
}

unsafe fn gl_proc<T: Copy>(name: &std::ffi::CStr) -> Option<T> {
    let p = game_symbol(name.as_ptr());
    if p.is_null() { None } else { Some(std::mem::transmute_copy(&p)) }
}

/// Shows only the region the engine rendered, stretched to the drawable:
/// legacy titles keep a design-resolution viewport in a corner of a
/// window-sized framebuffer. Opt out with PEAR_NO_GL_VIEWPORT_FIT=1.
unsafe fn update_drawable_crop() {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    if *DISABLED.get_or_init(|| rt::env_set("PEAR_NO_GL_VIEWPORT_FIT")) {
        return;
    }
    let layer = drawable_layer();
    if layer.is_null() {
        return;
    }
    type GetIntegerv = unsafe extern "C" fn(u32, *mut i32);
    type ReadPixels = unsafe extern "C" fn(i32, i32, i32, i32, u32, u32, *mut c_void);
    let count = {
        let mut g = geo();
        g.present_count += 1;
        g.present_count
    };
    if debug() && count % 120 == 0 {
        let mut vp = [0i32; 4];
        let mut center = [0u8; 4];
        let mut corner = [0u8; 4];
        if let (Some(get), Some(read)) = (gl_proc::<GetIntegerv>(c"glGetIntegerv"), gl_proc::<ReadPixels>(c"glReadPixels")) {
            get(0x0BA2, vp.as_mut_ptr());
            if vp[2] > 0 && vp[3] > 0 {
                read(vp[0] + vp[2] / 2, vp[1] + vp[3] / 2, 1, 1, 0x1908, 0x1401, center.as_mut_ptr() as *mut c_void);
                read(vp[0] + 4, vp[1] + vp[3] - 4, 1, 1, 0x1908, 0x1401, corner.as_mut_ptr() as *mut c_void);
            }
        }
        rt::log(&format!(
            "[PearRuntime][gl] present #{count} center={:02x}{:02x}{:02x}{:02x} corner={:02x}{:02x}{:02x}{:02x}",
            center[0], center[1], center[2], center[3], corner[0], corner[1], corner[2], corner[3]
        ));
    }
    // Re-fit after every geometry change and briefly afterwards, while UIKit
    // settles; a slow periodic re-fit covers changes without a notification.
    let generation = GEOMETRY_GENERATION.load(Ordering::Relaxed);
    let refit = {
        let mut g = geo();
        if generation != g.seen_generation {
            g.seen_generation = generation;
            g.settle_frames = 45;
        }
        if g.settle_frames > 0 {
            g.settle_frames -= 1;
            true
        } else {
            count % 120 == 0
        }
    };
    if refit {
        fit_drawable(layer);
    }
    let Some(get) = gl_proc::<GetIntegerv>(c"glGetIntegerv") else { return };
    let mut viewport = [0i32; 4];
    get(0x0BA2, viewport.as_mut_ptr());
    if viewport[2] <= 0 || viewport[3] <= 0 {
        return;
    }
    let mut g = geo();
    let dw = g.gl_drawable.width.round() as i32;
    let dh = g.gl_drawable.height.round() as i32;
    if dw <= 0 || dh <= 0 {
        return;
    }
    if debug() && viewport != g.last_present {
        g.last_present = viewport;
        rt::log(&format!(
            "[PearRuntime][gl] present viewport=({},{},{},{}) drawable={dw}x{dh}",
            viewport[0], viewport[1], viewport[2], viewport[3]
        ));
    }
    let small = dw as f64 >= viewport[2] as f64 * 1.4 && dh as f64 >= viewport[3] as f64 * 1.4;
    if !small {
        if !g.crop_applied {
            return;
        }
        g.crop_applied = false;
        g.last_crop[0] = -1;
        drop(g);
        rt::async_main(|| unsafe {
            let current = drawable_layer();
            if !current.is_null() {
                msg1::<CGRect, ()>(current, "setContentsRect:", CGRect { origin: CGPoint::default(), size: CGSize { width: 1.0, height: 1.0 } });
            }
        });
        return;
    }
    if viewport == g.last_crop {
        return;
    }
    g.last_crop = viewport;
    g.crop_applied = true;
    drop(g);
    let top = dh - (viewport[1] + viewport[3]);
    let rect = CGRect {
        origin: CGPoint { x: viewport[0] as f64 / dw as f64, y: top as f64 / dh as f64 },
        size: CGSize { width: viewport[2] as f64 / dw as f64, height: viewport[3] as f64 / dh as f64 },
    };
    if debug() {
        rt::log(&format!(
            "[PearRuntime][gl] crop {}x{} at ({},{}) from drawable {dw}x{dh}",
            viewport[2], viewport[3], viewport[0], viewport[1]
        ));
    }
    rt::async_main(move || unsafe {
        let current = drawable_layer();
        if current.is_null() {
            return;
        }
        msg1::<CGRect, ()>(current, "setContentsRect:", rect);
        msg1::<Id, ()>(current, "setContentsGravity:", nsstring("resize"));
        msg1::<Id, ()>(current, "setMagnificationFilter:", nsstring("linear"));
    });
}

#[no_mangle]
pub unsafe extern "C" fn EAGLGetProcAddress(name: *const c_char) -> *mut c_void {
    game_symbol(name)
}

unsafe fn window_for_layer(layer: Id) -> Id {
    let delegate: Id = msg0(layer, "delegate");
    if !delegate.is_null() && pear_objc::is_kind_of(delegate, "UIView") { msg0(delegate, "window") } else { NIL }
}

/// Whether a large (maximised / full-screen) window keeps the drawable at the
/// size the engine first created (legacy profile) or re-renders at window
/// size. PEAR_GL_FREEZE_LARGE_WINDOW=1/0 forces either.
fn freezes_large_windows() -> bool {
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| {
        let freeze = match std::env::var("PEAR_GL_FREEZE_LARGE_WINDOW") {
            Ok(v) if !v.is_empty() => v != "0",
            _ => rt::env_set("PEAR_LEGACY_ATTACH_LAYER"),
        };
        rt::log(&format!(
            "[PearRuntime][gl] large windows: {}",
            if freeze { "keep the launch drawable (legacy profile)" } else { "re-render at window size" }
        ));
        freeze
    })
}

static TOUCH_REMAP_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Legacy GL titles ask for `locationInView:window` (or nil) and divide by
/// their design scale; after a Mac window resize the view is fitted but that
/// window point is not. Only the window-space answer is rewritten.
#[export_name = "_Z19PearGLMapTouchPointP7UITouchP6UIView7CGPoint"]
pub unsafe extern "C" fn map_touch_point(touch: Id, view: Id, point: CGPoint) -> CGPoint {
    #[cfg(feature = "smoke")]
    if let Some(f) = crate::smoke::hook::<unsafe extern "C" fn(Id, Id, CGPoint) -> CGPoint>(&crate::smoke::GL_MAP_TOUCH_POINT) {
        return f(touch, view, point);
    }
    if !geo().fit_applied || !msg0::<bool>(cls("NSThread"), "isMainThread") {
        return point;
    }
    let drawable = drawable_layer();
    let delegate: Id = if drawable.is_null() { NIL } else { msg0(drawable, "delegate") };
    let render_view = if !delegate.is_null() && pear_objc::is_kind_of(delegate, "UIView") { delegate } else { NIL };
    let target: Id = msg0(touch, "view");
    if render_view.is_null() || target.is_null() || !msg1::<Id, bool>(target, "isDescendantOfView:", render_view) {
        return point;
    }
    let mut window: Id = msg0(render_view, "window");
    if window.is_null() {
        window = rt::primary_window();
    }
    if window.is_null() {
        return point;
    }
    let window_space = view.is_null()
        || pear_objc::is_kind_of(view, "UIWindow")
        || (msg0::<Id>(view, "layer") != drawable
            && msg0::<Id>(view, "window") == window
            && msg0::<CGRect>(view, "bounds").size == msg0::<CGRect>(window, "bounds").size);
    if !window_space {
        return point;
    }
    // Root-view coordinates may include an offset inside the window.
    let window_layer: Id = msg0(window, "layer");
    let window_point = if !view.is_null() && view != window { msg2(msg0::<Id>(view, "layer"), "convertPoint:toLayer:", point, window_layer) } else { point };
    let remapped: CGPoint = msg2(drawable, "convertPoint:fromLayer:", window_point, window_layer);
    if rt::env_set("PEAR_DEBUG_TOUCH") && msg0::<isize>(touch, "phase") != 1 && ((remapped.x - point.x).abs() > 0.5 || (remapped.y - point.y).abs() > 0.5) {
        rt::log(&format!(
            "[PearRuntime][touch] mapped {}: {} -> {}",
            if view.is_null() { "nil".into() } else { pear_objc::class_name(view).to_string() },
            point_s(point),
            point_s(remapped)
        ));
    }
    remapped
}

static ORIGINAL_LOCATION: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_PREVIOUS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn location_in_view(touch: Id, cmd: Sel, view: Id) -> CGPoint {
    let f: unsafe extern "C" fn(Id, Sel, Id) -> CGPoint = std::mem::transmute(ORIGINAL_LOCATION.load(Ordering::Acquire));
    map_touch_point(touch, view, f(touch, cmd, view))
}
unsafe extern "C" fn previous_location_in_view(touch: Id, cmd: Sel, view: Id) -> CGPoint {
    let f: unsafe extern "C" fn(Id, Sel, Id) -> CGPoint = std::mem::transmute(ORIGINAL_PREVIOUS.load(Ordering::Acquire));
    map_touch_point(touch, view, f(touch, cmd, view))
}

unsafe fn install_touch_remap() {
    if TOUCH_REMAP_INSTALLED.load(Ordering::Acquire) {
        return;
    }
    let touch_class = rt::class_named("UITouch");
    if touch_class.is_null() {
        return;
    }
    TOUCH_REMAP_INSTALLED.store(true, Ordering::Release);
    for (selector, imp, slot) in [("locationInView:", location_in_view as Imp, &ORIGINAL_LOCATION), ("previousLocationInView:", previous_location_in_view as Imp, &ORIGINAL_PREVIOUS)] {
        let m = rt::class_getInstanceMethod(touch_class, pear_objc::sel(selector));
        if m.is_null() {
            continue;
        }
        slot.store(rt::method_getImplementation(m) as usize, Ordering::Release);
        rt::override_instance(touch_class, selector, imp);
    }
    rt::log("[PearRuntime][touch] current/previous window-space touch mapping installed");
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Transform3D {
    m: [f64; 16],
}

extern "C" {
    fn CATransform3DMakeScale(sx: f64, sy: f64, sz: f64) -> Transform3D;
}

/// Fits the drawable into the key window while keeping the engine's
/// coordinate space: the visible size changes with a layer transform, the
/// layer's bounds stay at the size the renderbuffer was allocated at.
unsafe fn fit_drawable(layer: Id) {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    if layer.is_null() || *DISABLED.get_or_init(|| rt::env_set("PEAR_NO_GL_FIT")) {
        return;
    }
    let native = geo().native;
    if native.width < 1.0 || native.height < 1.0 {
        return;
    }
    let fixed_function = CLIENT_VERSION.load(Ordering::Relaxed) == 1 || rt::env_set("PEAR_LEGACY_ATTACH_LAYER");
    rt::async_main(move || unsafe { pear_objc::autorelease_pool(|| fit_on_main(native, fixed_function)) });
}

unsafe fn fit_on_main(native: CGSize, fixed_function: bool) {
    let current = drawable_layer();
    let window = rt::primary_window();
    if current.is_null() || window.is_null() {
        return;
    }
    let window_bounds: CGRect = msg0(window, "bounds");
    if window_bounds.size.width < 1.0 || window_bounds.size.height < 1.0 {
        return;
    }
    let parent: Id = msg0(current, "superlayer");
    if parent.is_null() {
        return;
    }
    let window_layer: Id = msg0(window, "layer");
    let container: CGRect = msg2(parent, "convertRect:fromLayer:", window_bounds, window_layer);
    if container.size.width <= 0.0 || container.size.height <= 0.0 || !container.origin.x.is_finite() || !container.size.width.is_finite() {
        return;
    }
    rt::without_actions(|| unsafe {
        // The last window the app was laid out in; a window much larger than
        // the drawable in both axes is the full-screen form.
        let full_screen_like = freezes_large_windows() && window_bounds.size.width > native.width * 1.4 && window_bounds.size.height > native.height * 1.4;
        let now = CFAbsoluteTimeGetCurrent();
        let reference = {
            let mut g = geo();
            if !full_screen_like {
                if g.reference_window.width < 1.0 {
                    g.reference_window = window_bounds.size;
                }
                if g.pending_window != window_bounds.size {
                    g.pending_window = window_bounds.size;
                    g.pending_since = now;
                } else if now - g.pending_since > 0.4 {
                    g.reference_window = window_bounds.size;
                }
            }
            g.reference_window
        };
        if fixed_function {
            // GLES1 engines lay controls out in the drawable's logical space.
            set_screen_override(native);
        } else if full_screen_like && reference.width > 0.0 {
            set_screen_override(reference);
        } else {
            clear_screen_override();
        }
        let mut basis = native;
        let bounds: CGRect = msg0(current, "bounds");
        if full_screen_like {
            if (bounds.size.width - native.width).abs() > 0.5 || (bounds.size.height - native.height).abs() > 0.5 {
                msg1::<CGRect, ()>(current, "setBounds:", CGRect { origin: CGPoint::default(), size: native });
            }
        } else {
            // Autoresizing changes bounds through the fit transform; only a
            // renderbuffer allocation establishes a new coordinate space.
            let cs: f64 = msg0(current, "contentsScale");
            let pixel_scale = if cs > 0.0 { cs } else { 1.0 };
            let drawable = geo().gl_drawable;
            basis = CGSize { width: drawable.width / pixel_scale, height: drawable.height / pixel_scale };
            if basis.width < 1.0 || basis.height < 1.0 {
                return;
            }
            if bounds.size != basis {
                msg1::<CGRect, ()>(current, "setBounds:", CGRect { origin: CGPoint::default(), size: basis });
            }
        }
        let fitted = aspect_fit_rect(basis, container);
        let scale = fitted.size.width / basis.width;
        if scale <= 0.0 {
            return;
        }
        // Re-apply the position even with an unchanged scale.
        let anchor: CGPoint = msg0(current, "anchorPoint");
        msg1::<CGPoint, ()>(current, "setPosition:", CGPoint {
            x: fitted.origin.x + anchor.x * fitted.size.width,
            y: fitted.origin.y + anchor.y * fitted.size.height,
        });
        let transform: Transform3D = msg0(current, "transform");
        {
            let g = geo();
            if g.fit_applied
                && g.last_fit_window == window_bounds.size
                && g.last_fit_layer == basis
                && (g.last_fit_scale - scale).abs() < 0.001
                && (transform.m[0] - scale).abs() < 0.001
                && (transform.m[5] - scale).abs() < 0.001
            {
                return;
            }
        }
        {
            let mut g = geo();
            g.fit_applied = true;
            g.last_fit_window = window_bounds.size;
            g.last_fit_layer = basis;
            g.last_fit_scale = scale;
        }
        msg1::<Transform3D, ()>(current, "setTransform:", CATransform3DMakeScale(scale, scale, 1.0));
        let black: Id = msg0(cls("UIColor"), "blackColor");
        let superlayer: Id = msg0(current, "superlayer");
        if !superlayer.is_null() {
            msg1::<Id, ()>(superlayer, "setBackgroundColor:", msg0::<Id>(black, "CGColor"));
        }
        msg1::<Id, ()>(window, "setBackgroundColor:", black);
        if debug() {
            rt::log(&format!("[PearRuntime][gl] fit drawable {} x{scale:.3} into window {}", size_s(native), rect_s(window_bounds)));
        }
    });
}

// ---------------------------------------------------------------------------
// EAGL hook: renderbufferStorage:fromDrawable: takes the size from the layer;
// fixing the layer first makes the renderbuffer match the window.

static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_STORAGE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_PRESENT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn hooked_storage(this: Id, cmd: Sel, target: usize, drawable_obj: Id) -> bool {
    let is_layer = !drawable_obj.is_null() && pear_objc::is_kind_of(drawable_obj, "CALayer");
    let main = msg0::<bool>(cls("NSThread"), "isMainThread");
    if is_layer {
        let layer = drawable_obj;
        let bounds: CGRect = msg0(layer, "bounds");
        let oriented = declared_drawable_size(bounds.size);
        if main && oriented != bounds.size {
            if debug() {
                rt::log(&format!("[PearRuntime][gl] corrected fixed-orientation drawable {} -> {}", size_s(bounds.size), size_s(oriented)));
            }
            msg1::<CGRect, ()>(layer, "setBounds:", CGRect { origin: bounds.origin, size: oriented });
        }
        if debug() {
            rt::log(&format!(
                "[PearRuntime][gl] renderbufferStorage fromDrawable bounds={} pos={} contentsScale={:.2}",
                rect_s(msg0(layer, "bounds")),
                point_s(msg0(layer, "position")),
                msg0::<f64>(layer, "contentsScale")
            ));
        }
        let size = msg0::<CGRect>(layer, "bounds").size;
        let native = geo().native;
        if native.width < 1.0 || native.height < 1.0 {
            geo().native = size;
            install_touch_remap();
        } else if main && ((size.width - native.width).abs() > 0.5 || (size.height - native.height).abs() > 0.5) {
            // A view inflated for full screen allocating a new drawable: keep
            // the engine's native size; the transform does the visible size.
            let window = window_for_layer(layer);
            let fixed_legacy = rt::env_set("PEAR_LEGACY_ATTACH_LAYER");
            let full_screen_like = fixed_legacy
                || (!window.is_null() && freezes_large_windows() && size.width > native.width * 1.4 && size.height > native.height * 1.4);
            if full_screen_like {
                msg1::<CGRect, ()>(layer, "setBounds:", CGRect { origin: CGPoint::default(), size: native });
            }
        }
        let size = msg0::<CGRect>(layer, "bounds").size;
        let scale: f64 = msg0(layer, "contentsScale");
        geo().gl_drawable = CGSize { width: size.width * scale, height: size.height * scale };
        set_drawable_layer(layer);
        observe_geometry();
        geometry_changed();
        fit_drawable(layer);
    } else if debug() {
        rt::log(&format!(
            "[PearRuntime][gl] renderbufferStorage fromDrawable {}",
            if drawable_obj.is_null() { "(null)".into() } else { pear_objc::class_name(drawable_obj).to_string() }
        ));
    }
    let f: unsafe extern "C" fn(Id, Sel, usize, Id) -> bool = std::mem::transmute(ORIGINAL_STORAGE.load(Ordering::Acquire));
    let result = f(this, cmd, target, drawable_obj);
    if result && is_layer {
        let version = eagl::api_of(this);
        CLIENT_VERSION.store(version, Ordering::Relaxed);
        if version == 1 || rt::env_set("PEAR_LEGACY_ATTACH_LAYER") {
            let logical = msg0::<CGRect>(drawable_obj, "bounds").size;
            *FIXED_LOGICAL.lock().unwrap_or_else(|e| e.into_inner()) = logical;
            set_screen_override(logical);
        }
        if let Some(query) = gl_proc::<unsafe extern "C" fn(u32, u32, *mut i32)>(c"glGetRenderbufferParameteriv") {
            let (mut w, mut h) = (0, 0);
            query(target as u32, 0x8D42, &mut w);
            query(target as u32, 0x8D43, &mut h);
            if w > 0 && h > 0 {
                geo().gl_drawable = CGSize { width: w as f64, height: h as f64 };
                if debug() {
                    rt::log(&format!("[PearRuntime][gl] allocated renderbuffer={w}x{h}"));
                }
            }
        }
    }
    result
}

/// The viewport is final only while the engine presents: read it there.
unsafe extern "C" fn hooked_present(this: Id, cmd: Sel, target: usize) -> bool {
    update_drawable_crop();
    let f: unsafe extern "C" fn(Id, Sel, usize) -> bool = std::mem::transmute(ORIGINAL_PRESENT.load(Ordering::Acquire));
    let presented = f(this, cmd, target);
    if presented {
        crate::graphics::fps_hud::record_frame_presented();
    }
    presented
}

unsafe fn install_eagl_hook() {
    if HOOK_INSTALLED.load(Ordering::Acquire) || !routing_enabled() {
        return;
    }
    let context = eagl::context_class();
    let m = rt::class_getInstanceMethod(context, pear_objc::sel("renderbufferStorage:fromDrawable:"));
    if m.is_null() {
        return;
    }
    if HOOK_INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    ORIGINAL_STORAGE.store(rt::method_getImplementation(m) as usize, Ordering::Release);
    rt::method_setImplementation(m, hooked_storage as Imp);
    rt::log("[PearRuntime][gl] EAGL drawable fit installed");
    let p = rt::class_getInstanceMethod(context, pear_objc::sel("presentRenderbuffer:"));
    if !p.is_null() {
        ORIGINAL_PRESENT.store(rt::method_getImplementation(p) as usize, Ordering::Release);
        rt::method_setImplementation(p, hooked_present as Imp);
        rt::log("[PearRuntime][gl] EAGL present hook installed");
    }
}

/// Installed by the router before the game allocates its first drawable.
#[no_mangle]
pub extern "C" fn PearANGLEInstallGeometryHooks() {
    unsafe { install_eagl_hook() };
}

pub fn install_geometry_hooks() {
    PearANGLEInstallGeometryHooks();
}

// ---------------------------------------------------------------------------
// Legacy touch diagnostics (PEAR_DEBUG_TOUCH)

static EAGL_TOUCH_ORIGINALS: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];

unsafe fn log_eagl_touch(this: Id, cmd: Sel, touches: Id, event: Id, index: usize) {
    let touch: Id = msg0(touches, "anyObject");
    let point: CGPoint = msg1(touch, "locationInView:", this);
    let bounds: CGRect = msg0(this, "bounds");
    rt::log(&format!(
        "[PearRuntime][touch] {} point=({:.1},{:.1}) bounds={} scale={:.2}",
        rt::cstr(rt::sel_getName(cmd)),
        point.x,
        point.y,
        rect_s(bounds),
        msg0::<f64>(this, "contentScaleFactor")
    ));
    let f: unsafe extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(EAGL_TOUCH_ORIGINALS[index].load(Ordering::Acquire));
    f(this, cmd, touches, event);
}
unsafe extern "C" fn eagl_touches_began(this: Id, cmd: Sel, touches: Id, event: Id) {
    log_eagl_touch(this, cmd, touches, event, 0)
}
unsafe extern "C" fn eagl_touches_ended(this: Id, cmd: Sel, touches: Id, event: Id) {
    log_eagl_touch(this, cmd, touches, event, 1)
}

unsafe fn install_eagl_touch_logging() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    let view_class = rt::class_named("EAGLView");
    if INSTALLED.load(Ordering::Acquire) || view_class.is_null() {
        return;
    }
    INSTALLED.store(true, Ordering::Release);
    for (i, (selector, imp)) in [("touchesBegan:withEvent:", eagl_touches_began as Imp), ("touchesEnded:withEvent:", eagl_touches_ended as Imp)].into_iter().enumerate() {
        let m = rt::class_getInstanceMethod(view_class, pear_objc::sel(selector));
        if m.is_null() {
            continue;
        }
        EAGL_TOUCH_ORIGINALS[i].store(rt::method_getImplementation(m) as usize, Ordering::Release);
        rt::override_instance(view_class, selector, imp);
    }
    rt::log("[PearRuntime][touch] EAGLView touch logging installed");
}

static LEGACY_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static LEGACY_IVARS: OnceLock<[isize; 3]> = OnceLock::new();

extern "C" {
    fn class_getInstanceVariable(c: Id, name: *const c_char) -> *mut c_void;
    fn ivar_getOffset(ivar: *mut c_void) -> isize;
}

/// The engine's `createZUITouch:withIndex:` input and output: tells a UIKit
/// delivery problem from the engine's own coordinate conversion.
unsafe extern "C" fn legacy_create_touch(this: Id, cmd: Sel, touch: Id, index: i32) -> *mut c_void {
    let point: CGPoint = msg1(touch, "locationInView:", this);
    let bounds: CGRect = msg0(this, "bounds");
    let offsets = LEGACY_IVARS.get().copied().unwrap_or([-1; 3]);
    let read_i32 = |o: isize| if o >= 0 { *((this as *mut u8).offset(o) as *const i32) } else { 0 };
    let (width, height) = (read_i32(offsets[0]), read_i32(offsets[1]));
    let content_scale = if offsets[2] >= 0 { *((this as *mut u8).offset(offsets[2]) as *const f32) } else { 0.0 };
    let f: unsafe extern "C" fn(Id, Sel, Id, i32) -> *mut c_void = std::mem::transmute(LEGACY_ORIGINAL.load(Ordering::Acquire));
    let z = f(this, cmd, touch, index);
    let (zx, zy) = if z.is_null() { (0.0f32, 0.0f32) } else { (*((z as *mut u8).add(0xc) as *const f32), *((z as *mut u8).add(0x10) as *const f32)) };
    rt::log(&format!(
        "[PearRuntime][legacytouch] view={} bounds={} point=({:.1},{:.1}) m={width}x{height} scale={content_scale:.2} index={index} -> z=({zx:.4},{zy:.4}) phase={}",
        pear_objc::class_name(this),
        rect_s(bounds),
        point.x,
        point.y,
        msg0::<isize>(touch, "phase")
    ));
    z
}

unsafe fn install_legacy_touch_logging() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    let view_class = rt::class_named("OpenGLView");
    if INSTALLED.load(Ordering::Acquire) || view_class.is_null() {
        return;
    }
    let m = rt::class_getInstanceMethod(view_class, pear_objc::sel("createZUITouch:withIndex:"));
    if m.is_null() {
        return;
    }
    INSTALLED.store(true, Ordering::Release);
    let offset = |n: &std::ffi::CStr| {
        let ivar = class_getInstanceVariable(view_class, n.as_ptr());
        if ivar.is_null() { -1 } else { ivar_getOffset(ivar) }
    };
    let _ = LEGACY_IVARS.set([offset(c"m_width"), offset(c"m_height"), offset(c"m_contentScale")]);
    LEGACY_ORIGINAL.store(rt::method_getImplementation(m) as usize, Ordering::Release);
    rt::method_setImplementation(m, legacy_create_touch as Imp);
    rt::log("[PearRuntime][legacytouch] diagnostics installed");
}

pub fn init() {
    if !rt::module_enabled("graphics-gl") || !routing_enabled() {
        return;
    }
    // Retry while the application connects its own drawable/window.
    for i in 0..40 {
        rt::after_main(i as f64 * 0.5, || {
            extern "C" {
                fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut c_void;
                fn dispatch_async_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
            }
            extern "C" fn work(_: *mut c_void) {
                unsafe { pear_objc::autorelease_pool(|| install_eagl_hook()) };
            }
            unsafe { dispatch_async_f(dispatch_get_global_queue(0x11, 0), std::ptr::null_mut(), work) };
        });
    }
    if rt::env_set("PEAR_DEBUG_TOUCH") {
        rt::after_main(1.0, || unsafe {
            install_eagl_touch_logging();
            install_legacy_touch_logging();
        });
    }
    let _ = objc_release;
}
