//! Keeps an app's visible CAMetalLayer renderable, and makes sure a window
//! that renders has a key window to receive input.
//!
//! Engines that create their CAMetalLayer before the scene attaches it get a
//! layer with no device and a zero drawable size and present into nothing.
//! Platform defaults are filled in (at nextDrawable and in a startup scan),
//! zero-sized texture descriptors are clamped so Metal validation does not
//! abort, and the first visible window becomes key if none is. Rendering,
//! shaders and app memory are untouched. PEAR_NO_METAL_FIX disables it.

use crate::rt;
use pear_objc::{cls, msg0, msg1, nsarray_count, nsarray_object_at, objc_getAssociatedObject, objc_setAssociatedObject, sel, CGRect, CGSize, Class, Id, Imp, Sel, NIL};
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

#[link(name = "Metal", kind = "framework")]
extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}
extern "C" {
    fn CACurrentMediaTime() -> f64;
    fn NSStringFromCGRect(r: CGRect) -> Id;
    fn NSStringFromCGSize(s: CGSize) -> Id;
}

static SEEDED_BOUNDS: u8 = 0;
static SEEDED_DRAWABLE: u8 = 0;
const SEED: CGSize = CGSize { width: 1920.0, height: 1080.0 };

static TEXTURE_GUARD: Mutex<bool> = Mutex::new(false);
static NEXT_DRAWABLE_FIX: Mutex<bool> = Mutex::new(false);
static TEX: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static TEX_OFFSET: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static TEX_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static NEXT_DRAWABLE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static TEXTURE_FAILURES: AtomicU32 = AtomicU32::new(0);

fn debug() -> bool {
    rt::env_set("PEAR_DEBUG_METAL")
}

fn perf_trace() -> bool {
    static T: OnceLock<bool> = OnceLock::new();
    *T.get_or_init(|| rt::env_set("PEAR_PERF_TRACE"))
}

fn default_device() -> Id {
    static D: OnceLock<usize> = OnceLock::new();
    *D.get_or_init(|| unsafe { MTLCreateSystemDefaultDevice() as usize }) as Id
}

/// Zero-sized backbuffer textures (created before the window exists) abort
/// Metal validation; the real size replaces them once it is known.
unsafe fn clamp_descriptor(descriptor: Id) {
    if descriptor.is_null() {
        return;
    }
    if msg0::<usize>(descriptor, "width") == 0 {
        msg1::<usize, ()>(descriptor, "setWidth:", 1);
        if debug() {
            rt::log("[PearRuntime][metal] clamped zero texture width");
        }
    }
    if msg0::<usize>(descriptor, "height") == 0 {
        msg1::<usize, ()>(descriptor, "setHeight:", 1);
        if debug() {
            rt::log("[PearRuntime][metal] clamped zero texture height");
        }
    }
}

unsafe extern "C" fn new_texture(s: Id, c: Sel, descriptor: Id) -> Id {
    clamp_descriptor(descriptor);
    let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(TEX.load(Ordering::Acquire));
    let texture = f(s, c, descriptor);
    if texture.is_null() && debug() && TEXTURE_FAILURES.fetch_add(1, Ordering::Relaxed) < 32 {
        rt::log(&format!(
            "[PearRuntime][metal] texture allocation failed format={} size={}x{} mipmaps={} usage={} storage={}",
            msg0::<usize>(descriptor, "pixelFormat"),
            msg0::<usize>(descriptor, "width"),
            msg0::<usize>(descriptor, "height"),
            msg0::<usize>(descriptor, "mipmapLevelCount"),
            msg0::<usize>(descriptor, "usage"),
            msg0::<usize>(descriptor, "storageMode")
        ));
    }
    texture
}

unsafe extern "C" fn new_texture_offset(s: Id, c: Sel, descriptor: Id, offset: usize, bytes_per_row: usize) -> Id {
    clamp_descriptor(descriptor);
    let f: unsafe extern "C" fn(Id, Sel, Id, usize, usize) -> Id = std::mem::transmute(TEX_OFFSET.load(Ordering::Acquire));
    f(s, c, descriptor, offset, bytes_per_row)
}

unsafe extern "C" fn new_texture_surface(s: Id, c: Sel, descriptor: Id, surface: *mut c_void, plane: usize) -> Id {
    clamp_descriptor(descriptor);
    let f: unsafe extern "C" fn(Id, Sel, Id, *mut c_void, usize) -> Id = std::mem::transmute(TEX_SURFACE.load(Ordering::Acquire));
    f(s, c, descriptor, surface, plane)
}

unsafe fn swap(c: Class, selector: &str, imp: Imp, slot: &AtomicPtr<c_void>) -> bool {
    let m = rt::class_getInstanceMethod(c, sel(selector));
    if m.is_null() {
        return false;
    }
    slot.store(rt::method_getImplementation(m) as *mut c_void, Ordering::Release);
    rt::method_setImplementation(m, imp);
    true
}

unsafe fn install_texture_guard(device: Id) {
    if device.is_null() {
        return;
    }
    let mut installed = TEXTURE_GUARD.lock().unwrap_or_else(|e| e.into_inner());
    if *installed {
        return;
    }
    let c = rt::object_getClass(device);
    if rt::class_getInstanceMethod(c, sel("newTextureWithDescriptor:")).is_null() {
        return;
    }
    *installed = true;
    swap(c, "newTextureWithDescriptor:", new_texture as Imp, &TEX);
    swap(c, "newTextureWithDescriptor:offset:bytesPerRow:", new_texture_offset as Imp, &TEX_OFFSET);
    swap(c, "newTextureWithDescriptor:iosurface:plane:", new_texture_surface as Imp, &TEX_SURFACE);
    if debug() {
        rt::log("[PearRuntime][metal] zero-size texture guard installed");
    }
}

unsafe fn connected_windows(mut f: impl FnMut(Id)) {
    let app: Id = msg0(cls("UIApplication"), "sharedApplication");
    let scenes: Id = msg0(msg0::<Id>(app, "connectedScenes"), "allObjects");
    let window_scene = cls("UIWindowScene");
    for i in 0..nsarray_count(scenes) {
        let scene = nsarray_object_at(scenes, i);
        if !msg1::<Class, bool>(scene, "isKindOfClass:", window_scene) {
            continue;
        }
        let windows: Id = msg0(scene, "windows");
        for j in 0..nsarray_count(windows) {
            f(nsarray_object_at(windows, j));
        }
    }
}

/// UIKit delivers no touches until a window is key: make the first visible
/// one key, the state a normal iOS launch produces.
unsafe fn ensure_key_window() {
    let app: Id = msg0(cls("UIApplication"), "sharedApplication");
    if msg0::<isize>(app, "applicationState") != 0 {
        return;
    }
    let ns_app_class = cls("NSApplication");
    let ns_app: Id = if !ns_app_class.is_null() && pear_objc::responds_to(ns_app_class, "sharedApplication") { msg0(ns_app_class, "sharedApplication") } else { NIL };
    if ns_app.is_null() || !pear_objc::responds_to(ns_app, "isActive") || !msg0::<bool>(ns_app, "isActive") {
        return;
    }
    let mut has_key = false;
    let mut first_visible: Id = NIL;
    connected_windows(|w| {
        if msg0::<bool>(w, "isKeyWindow") {
            has_key = true;
        }
        if first_visible.is_null() && !msg0::<bool>(w, "isHidden") && !msg0::<Id>(w, "rootViewController").is_null() {
            first_visible = w;
        }
    });
    if has_key || first_visible.is_null() {
        return;
    }
    msg0::<()>(first_visible, "makeKeyAndVisible");
    rt::log(&format!("[PearRuntime][window] promoted {} to key window", rt::class_name(rt::object_getClass(first_visible))));
}

unsafe fn flag(layer: Id, key: &u8) -> bool {
    let v = objc_getAssociatedObject(layer, key as *const u8 as *const c_void);
    !v.is_null() && msg0::<bool>(v, "boolValue")
}

unsafe fn set_flag(layer: Id, key: &u8, on: bool) {
    objc_setAssociatedObject(layer, key as *const u8 as *const c_void, if on { pear_objc::nsnumber_bool(true) } else { NIL }, 1);
}

/// Repairs the window tree's Metal layers; returns (changed, seen).
unsafe fn fix_metal_layers() -> (i32, i32) {
    install_next_drawable_fix();
    let device = default_device();
    if device.is_null() {
        if debug() {
            rt::log("[PearRuntime][metal] MTLCreateSystemDefaultDevice returned nil");
        }
        return (0, 0);
    }
    ensure_key_window();
    let mut stack: Vec<Id> = Vec::new();
    let mut window_count = 0usize;
    connected_windows(|w| {
        let l: Id = msg0(w, "layer");
        if !l.is_null() {
            stack.push(l);
        }
        window_count += 1;
    });
    // Legacy apps may own a delegate window without a connected scene.
    if window_count == 0 {
        let w = rt::primary_window();
        let l: Id = if w.is_null() { NIL } else { msg0(w, "layer") };
        if !l.is_null() {
            stack.push(l);
            window_count += 1;
        }
    }
    let metal_class = cls("CAMetalLayer");
    let (mut fixed, mut seen) = (0, 0);
    while let Some(layer) = stack.pop() {
        if msg1::<Class, bool>(layer, "isKindOfClass:", metal_class) {
            seen += 1;
            let mut changed = false;
            if msg0::<Id>(layer, "device").is_null() {
                msg1::<Id, ()>(layer, "setDevice:", device);
                changed = true;
            }
            let mut bounds: CGRect = msg0(layer, "bounds");
            let host: Id = msg0(layer, "superlayer");
            let host_bounds: CGRect = if host.is_null() { CGRect::default() } else { msg0(host, "bounds") };
            let mut seeded_bounds = flag(layer, &SEEDED_BOUNDS);
            if seeded_bounds && bounds.size != SEED {
                set_flag(layer, &SEEDED_BOUNDS, false);
                seeded_bounds = false;
            }
            if seeded_bounds && host_bounds.size.width > 0.0 && host_bounds.size.height > 0.0 {
                bounds = host_bounds;
                msg1::<CGRect, ()>(layer, "setBounds:", bounds);
                set_flag(layer, &SEEDED_BOUNDS, false);
                changed = true;
            }
            if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
                bounds = host_bounds;
                if bounds.size.width > 0.0 && bounds.size.height > 0.0 {
                    msg1::<CGRect, ()>(layer, "setBounds:", bounds);
                    changed = true;
                }
            }
            let contents: f64 = msg0(layer, "contentsScale");
            let scale = if contents > 0.0 { contents } else { 1.0 };
            let wanted = CGSize { width: bounds.size.width * scale, height: bounds.size.height * scale };
            let mut seeded_drawable = flag(layer, &SEEDED_DRAWABLE);
            if seeded_drawable && msg0::<CGSize>(layer, "drawableSize") != SEED {
                set_flag(layer, &SEEDED_DRAWABLE, false);
                seeded_drawable = false;
            }
            if seeded_drawable && host_bounds.size.width > 0.0 && host_bounds.size.height > 0.0 && wanted.width > 0.0 && wanted.height > 0.0 {
                msg1::<CGSize, ()>(layer, "setDrawableSize:", wanted);
                set_flag(layer, &SEEDED_DRAWABLE, false);
                changed = true;
            }
            let drawable: CGSize = msg0(layer, "drawableSize");
            if (drawable.width <= 0.0 || drawable.height <= 0.0) && wanted.width > 0.0 && wanted.height > 0.0 {
                msg1::<CGSize, ()>(layer, "setDrawableSize:", wanted);
                changed = true;
            }
            if changed {
                fixed += 1;
                if debug() {
                    rt::log(&format!(
                        "[PearRuntime][metal] normalized {} bounds={} drawable={} scale={scale:.2} device={}",
                        rt::class_name(rt::object_getClass(layer)),
                        rt::ns_string(NSStringFromCGRect(msg0(layer, "bounds"))),
                        rt::ns_string(NSStringFromCGSize(msg0(layer, "drawableSize"))),
                        if msg0::<Id>(layer, "device").is_null() { "nil" } else { "yes" }
                    ));
                }
            }
        }
        let subs: Id = msg0(layer, "sublayers");
        for i in 0..nsarray_count(subs) {
            stack.push(nsarray_object_at(subs, i));
        }
    }
    if debug() {
        rt::log(&format!("[PearRuntime][metal] scan: {window_count} window(s), {fixed} layer(s) normalized"));
    }
    (fixed, seen)
}

struct Timing {
    count: u64,
    slow: u64,
    empty: u64,
    total_ms: f64,
    longest_ms: f64,
    reported_at: f64,
}
static TIMING: Mutex<Timing> = Mutex::new(Timing { count: 0, slow: 0, empty: 0, total_ms: 0.0, longest_ms: 0.0, reported_at: 0.0 });

/// A layer asked for a drawable before it reached a window gets its device
/// (and a seed size, replaced by the scan once real bounds exist) here.
unsafe extern "C" fn next_drawable(s: Id, c: Sel) -> Id {
    crate::graphics::fps_hud::note_drawable(s);
    let original: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(NEXT_DRAWABLE.load(Ordering::Acquire));
    let trace = perf_trace();
    // Hot path: nothing to repair once device and drawable size exist.
    if !trace {
        let d: CGSize = msg0(s, "drawableSize");
        if d.width > 0.0 && d.height > 0.0 && !msg0::<Id>(s, "device").is_null() {
            return original(s, c);
        }
    }
    if msg0::<Id>(s, "device").is_null() {
        let device = MTLCreateSystemDefaultDevice();
        if !device.is_null() {
            msg1::<Id, ()>(s, "setDevice:", device);
            install_texture_guard(device);
            pear_objc::objc_release(device);
            if debug() {
                rt::log("[PearRuntime][metal] assigned device before nextDrawable");
            }
        }
    }
    let b: CGRect = msg0(s, "bounds");
    if b.size.width <= 0.0 || b.size.height <= 0.0 {
        msg1::<CGRect, ()>(s, "setBounds:", CGRect { origin: Default::default(), size: SEED });
        set_flag(s, &SEEDED_BOUNDS, true);
    }
    let d: CGSize = msg0(s, "drawableSize");
    if d.width <= 0.0 || d.height <= 0.0 {
        msg1::<CGSize, ()>(s, "setDrawableSize:", SEED);
        set_flag(s, &SEEDED_DRAWABLE, true);
    }
    if !trace {
        return original(s, c);
    }
    let start = CACurrentMediaTime();
    let drawable = original(s, c);
    let elapsed = (CACurrentMediaTime() - start) * 1000.0;
    let mut t = TIMING.lock().unwrap_or_else(|e| e.into_inner());
    t.count += 1;
    t.total_ms += elapsed;
    t.longest_ms = t.longest_ms.max(elapsed);
    if elapsed > 16.667 {
        t.slow += 1;
    }
    if drawable.is_null() {
        t.empty += 1;
    }
    let now = CACurrentMediaTime();
    if t.reported_at == 0.0 {
        t.reported_at = now;
    }
    if now - t.reported_at >= 5.0 {
        let line = format!(
            "[PearRuntime][perf] drawable acquisition calls={} avg={:.3}ms max={:.3}ms over16ms={} nil={} (not FPS or GPU duration)",
            t.count, t.total_ms / t.count as f64, t.longest_ms, t.slow, t.empty
        );
        *t = Timing { count: 0, slow: 0, empty: 0, total_ms: 0.0, longest_ms: 0.0, reported_at: now };
        std::thread::spawn(move || rt::log(&line));
    }
    drawable
}

unsafe fn install_next_drawable_fix() -> bool {
    let layer_class = cls("CAMetalLayer");
    if layer_class.is_null() {
        if debug() {
            rt::log("[PearRuntime][metal] CAMetalLayer not registered yet");
        }
        return false;
    }
    let mut installed = NEXT_DRAWABLE_FIX.lock().unwrap_or_else(|e| e.into_inner());
    if *installed {
        return true;
    }
    if !swap(layer_class, "nextDrawable", next_drawable as Imp, &NEXT_DRAWABLE) {
        if debug() {
            rt::log("[PearRuntime][metal] CAMetalLayer has no nextDrawable");
        }
        return false;
    }
    *installed = true;
    if debug() {
        rt::log("[PearRuntime][metal] nextDrawable fix installed");
    }
    true
}

pub fn init() {
    if !rt::module_enabled("graphics-metal") || rt::env_set("PEAR_NO_METAL_FIX") {
        return;
    }
    unsafe {
        install_texture_guard(default_device());
        if !install_next_drawable_fix() {
            // One bounded retry source, stopped as soon as the class is ready.
            let mut attempts = 0;
            rt::main_ticker(0.05, 0.05, 0.005, move || unsafe {
                attempts += 1;
                install_next_drawable_fix() || attempts >= 400
            });
        }
    }
    // Layers and the key window can appear well after the first frame: scan
    // once a second for up to a minute, stopping after five healthy scans.
    let (mut scans, mut healthy) = (0, 0);
    rt::main_ticker(1.0, 1.0, 0.1, move || unsafe {
        pear_objc::autorelease_pool(|| {
            let (fixed, seen) = fix_metal_layers();
            healthy = if seen > 0 && fixed == 0 { healthy + 1 } else { 0 };
            scans += 1;
            if scans >= 60 || healthy >= 5 {
                if debug() {
                    rt::log(&format!("[PearRuntime][metal] layer scan finished after {scans} scan(s)"));
                }
                return true;
            }
            false
        })
    });
}
