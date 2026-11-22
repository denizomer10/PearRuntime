//! Renderbuffer / native-surface adapter (the `PearANGLEContext (Drawable)`
//! methods). No system GL entry points.

use super::backend;
use super::context::{self, ctx, current, NIL};
use crate::rt;
use pear_objc::{msg0, objc_release, objc_retain, CGPoint, CGRect, CGSize, Id, Sel};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// `PearANGLEDrawableGeometry`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Geometry {
    pub logical_bounds: CGRect,
    pub pixel_size: CGSize,
    pub backing_scale: f64,
    pub generation: u64,
}

pub struct Storage {
    layer: Id, // retained
    geometry: Geometry,
    retained: bool,
    /// Resized from a thread other than the context's: the owning thread
    /// reallocates at its next present.
    needs_reallocation: AtomicBool,
}
unsafe impl Send for Storage {}
unsafe impl Sync for Storage {}
impl Drop for Storage {
    fn drop(&mut self) {
        if !self.layer.is_null() {
            unsafe { objc_release(self.layer) };
        }
    }
}

#[derive(Default)]
struct Inner {
    storage: HashMap<u32, Arc<Storage>>,
    framebuffer: u32,
    generation: u64,
}

/// Per-context drawable state (the private FBO lives with the context and is
/// freed by eglDestroyContext).
#[derive(Default)]
pub struct State(Mutex<Inner>);

impl State {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Clone, Copy)]
struct Gl {
    get: unsafe extern "C" fn(u32, *mut i32),
    parameter: unsafe extern "C" fn(u32, u32, *mut i32),
    storage: unsafe extern "C" fn(u32, u32, i32, i32),
    is_renderbuffer: unsafe extern "C" fn(u32) -> u8,
    gen_framebuffer: unsafe extern "C" fn(i32, *mut u32),
    bind_framebuffer: unsafe extern "C" fn(u32, u32),
    attach: unsafe extern "C" fn(u32, u32, u32, u32),
    bind_renderbuffer: Option<unsafe extern "C" fn(u32, u32)>,
    flush: Option<unsafe extern "C" fn()>,
}

/// GLES1 contexts use the OES framebuffer entry points.
fn api() -> Option<Gl> {
    static CORE: OnceLock<Option<Gl>> = OnceLock::new();
    static OES: OnceLock<Option<Gl>> = OnceLock::new();
    let v1 = unsafe { ctx(current()).is_some_and(|c| c.client_version == 1) };
    let load = |oes: bool| unsafe {
        macro_rules! p {
            ($core:literal, $ext:literal) => {
                backend::proc(if oes { $ext } else { $core })
            };
        }
        Some(Gl {
            get: backend::proc(c"glGetIntegerv")?,
            parameter: p!(c"glGetRenderbufferParameteriv", c"glGetRenderbufferParameterivOES")?,
            storage: p!(c"glRenderbufferStorage", c"glRenderbufferStorageOES")?,
            is_renderbuffer: p!(c"glIsRenderbuffer", c"glIsRenderbufferOES")?,
            gen_framebuffer: p!(c"glGenFramebuffers", c"glGenFramebuffersOES")?,
            bind_framebuffer: p!(c"glBindFramebuffer", c"glBindFramebufferOES")?,
            attach: p!(c"glFramebufferRenderbuffer", c"glFramebufferRenderbufferOES")?,
            bind_renderbuffer: p!(c"glBindRenderbuffer", c"glBindRenderbufferOES"),
            flush: backend::proc(c"glFlush"),
        })
    };
    if v1 { *OES.get_or_init(|| load(true)) } else { *CORE.get_or_init(|| load(false)) }
}

const GL_RENDERBUFFER: usize = 0x8D41;
const GL_RGBA8: u32 = 0x8058;
const GL_DRAW_FRAMEBUFFER: u32 = 0x8CA9;
const GL_READ_FRAMEBUFFER: u32 = 0x8CA8;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;

unsafe fn fail(error: *mut Id, code: isize, message: &str) -> bool {
    if !error.is_null() {
        *error = rt::error("PearRuntime.ANGLE.Drawable", code, message);
    }
    false
}

unsafe fn ready(this: Id, target: usize, error: *mut Id) -> Option<Gl> {
    if current() != this {
        fail(error, 1, "Drawable operations require this context current on the calling thread");
        return None;
    }
    if target != GL_RENDERBUFFER {
        fail(error, 2, "Drawable operations require GL_RENDERBUFFER");
        return None;
    }
    match api() {
        Some(gl) => Some(gl),
        None => {
            fail(error, 3, "Required ANGLE renderbuffer entry points are missing");
            None
        }
    }
}

unsafe fn geometry_matches(s: &Storage) -> bool {
    let bounds: CGRect = msg0(s.layer, "bounds");
    let scale: f64 = msg0(s.layer, "contentsScale");
    rt::rect_eq(bounds, s.geometry.logical_bounds) && scale == s.geometry.backing_scale
}

/// The caller has bound this renderbuffer. glGetError is never drained:
/// engine errors belong to the engine.
unsafe fn storage_matches(gl: &Gl, name: u32, s: &Storage) -> bool {
    if (gl.is_renderbuffer)(name) == 0 {
        return false;
    }
    let (mut w, mut h, mut format, mut samples) = (0, 0, 0, 0);
    (gl.parameter)(0x8D41, 0x8D42, &mut w);
    (gl.parameter)(0x8D41, 0x8D43, &mut h);
    (gl.parameter)(0x8D41, 0x8D44, &mut format);
    (gl.parameter)(0x8D41, 0x8CAB, &mut samples);
    w as f64 == s.geometry.pixel_size.width && h as f64 == s.geometry.pixel_size.height && format as u32 == GL_RGBA8 && samples == 0
}

unsafe fn state(this: Id) -> Option<&'static State> {
    ctx(this).map(|c| &c.drawable)
}

pub unsafe extern "C" fn forget_imp(this: Id, _: Sel, names: *const u32, count: i32) {
    if names.is_null() || count <= 0 || current() != this {
        return;
    }
    let Some(s) = state(this) else { return };
    let mut inner = s.lock();
    for i in 0..count as usize {
        inner.storage.remove(&*names.add(i));
    }
}

pub unsafe fn allocate(this: Id, target: usize, layer: Id, format: usize, retained: bool, error: *mut Id) -> bool {
    let Some(gl) = ready(this, target, error) else { return false };
    if format != 1 {
        return fail(error, 4, "Only RGBA8 drawable storage is currently implemented");
    }
    let mut name = 0;
    (gl.get)(0x8CA7, &mut name);
    let name = name as u32;
    if name == 0 || (gl.is_renderbuffer)(name) == 0 {
        return fail(error, 5, "Bind an ANGLE renderbuffer before allocating drawable storage");
    }
    let Some(s) = state(this) else { return false };
    if layer.is_null() {
        s.lock().storage.remove(&name);
        (gl.storage)(0x8D41, GL_RGBA8, 0, 0);
        return true;
    }
    if !pear_objc::is_kind_of(layer, "CALayer") {
        return fail(error, 6, "Drawable storage requires a CALayer host");
    }
    let bounds: CGRect = msg0(layer, "bounds");
    let scale: f64 = msg0(layer, "contentsScale");
    let (w, h) = (bounds.size.width * scale, bounds.size.height * scale);
    if !scale.is_finite() || scale <= 0.0 || bounds.size.width <= 0.0 || bounds.size.height <= 0.0 || !w.is_finite() || !h.is_finite() || w < 1.0 || h < 1.0 || w > 8192.0 || h > 8192.0 {
        return fail(error, 7, "Invalid drawable geometry");
    }
    let mut binding_error: Id = NIL;
    if !context::bind_layer(this, layer, &mut binding_error) {
        let is_rebind = !binding_error.is_null()
            && rt::ns_string(msg0(binding_error, "domain")) == "PearRuntime.ANGLE.Context"
            && msg0::<isize>(binding_error, "code") == 24;
        if !is_rebind {
            if !error.is_null() {
                *error = binding_error;
            }
            return false;
        }
        // The engine asked to reallocate: rebuild the native surface before
        // publishing the new renderbuffer/input generation.
        if !context::detach_layer(this, error) || !context::bind_layer(this, layer, error) {
            return false;
        }
    }
    s.lock().storage.remove(&name);
    let (pw, ph) = ctx(this).map(|c| (c.width, c.height)).unwrap_or((0, 0));
    (gl.storage)(0x8D41, GL_RGBA8, pw as i32, ph as i32);
    let generation = {
        let mut inner = s.lock();
        inner.generation += 1;
        inner.generation
    };
    let storage = Arc::new(Storage {
        layer: objc_retain(layer),
        geometry: Geometry { logical_bounds: bounds, pixel_size: CGSize { width: pw as f64, height: ph as f64 }, backing_scale: scale, generation },
        retained,
        needs_reallocation: AtomicBool::new(false),
    });
    if !storage_matches(&gl, name, &storage) {
        return fail(error, 8, "ANGLE renderbuffer allocation did not match the native drawable");
    }
    if !geometry_matches(&storage) {
        return fail(error, 9, "Layer geometry changed during allocation; no input generation was published");
    }
    s.lock().storage.insert(name, storage);
    true
}

pub unsafe extern "C" fn allocate_imp(this: Id, _: Sel, target: usize, layer: Id, format: usize, retained: bool, error: *mut Id) -> bool {
    allocate(this, target, layer, format, retained, error)
}

fn error_is(error: Id, domain: &str, code: isize) -> bool {
    unsafe { !error.is_null() && rt::ns_string(msg0(error, "domain")) == domain && msg0::<isize>(error, "code") == code }
}

/// Engines that render on one thread and present from another (EAGL allows a
/// context current on several threads, ANGLE on one): a presenter context in
/// the same sharegroup owns the native surface and blits the shared renderbuffer.
pub unsafe fn present_other_thread(this: Id, error: *mut Id) -> bool {
    let Some(s) = state(this) else { return fail(error, 10, "No drawable storage to present") };
    let (mut storage, name) = {
        let inner = s.lock();
        match inner.storage.iter().max_by_key(|(_, v)| v.geometry.generation) {
            Some((k, v)) => (v.clone(), *k),
            None => return fail(error, 10, "No drawable storage to present"),
        }
    };
    if storage.layer.is_null() {
        return fail(error, 10, "No drawable storage to present");
    }
    // The owning thread gives up the native surface at its next make-current.
    if context::has_window_surface(this) {
        return fail(error, 14, "Waiting for the render thread to release the native surface");
    }
    let c = ctx(this).unwrap();
    if c.presenter.is_null() {
        let presenter: Id = pear_objc::msg3(msg0::<Id>(context::class(), "alloc"), "initWithClientVersion:sharegroup:error:", c.client_version, c.sharegroup, error);
        if presenter.is_null() {
            return false;
        }
        c.presenter = presenter; // +1 from alloc/init
    }
    let presenter = c.presenter;
    let previous = current();
    if !context::set_current(presenter, error) {
        return false;
    }
    let Some(gl) = api() else {
        context::set_current(previous, std::ptr::null_mut());
        return fail(error, 10, "Shared drawable renderbuffer is unavailable");
    };
    let mut old_renderbuffer = 0;
    let result = (|| {
        let mut bind_error: Id = NIL;
        if !context::bind_layer(presenter, storage.layer, &mut bind_error) {
            if !error_is(bind_error, "PearRuntime.ANGLE.Context", 24) {
                if !error.is_null() {
                    *error = bind_error;
                }
                return false;
            }
            if !context::detach_layer(presenter, error) || !context::bind_layer(presenter, storage.layer, error) {
                return false;
            }
        }
        let Some(bind_rb) = gl.bind_renderbuffer else { return fail(error, 10, "Shared drawable renderbuffer is unavailable") };
        if (gl.is_renderbuffer)(name) == 0 {
            return fail(error, 10, "Shared drawable renderbuffer is unavailable");
        }
        (gl.get)(0x8CA7, &mut old_renderbuffer);
        bind_rb(0x8D41, name);
        let pc = ctx(presenter).unwrap();
        // A resize requested from this thread: the renderbuffer is a shared
        // object, so the presenter respecifies it at the new pixel size.
        if storage.needs_reallocation.load(Ordering::Acquire)
            || !geometry_matches(&storage)
            || storage.geometry.pixel_size.width != pc.width as f64
            || storage.geometry.pixel_size.height != pc.height as f64
        {
            (gl.storage)(0x8D41, GL_RGBA8, pc.width as i32, pc.height as i32);
            let generation = {
                let mut inner = s.lock();
                inner.generation += 1;
                inner.generation
            };
            let replacement = Arc::new(Storage {
                layer: objc_retain(storage.layer),
                geometry: Geometry {
                    logical_bounds: msg0(storage.layer, "bounds"),
                    pixel_size: CGSize { width: pc.width as f64, height: pc.height as f64 },
                    backing_scale: msg0(storage.layer, "contentsScale"),
                    generation,
                },
                retained: storage.retained,
                needs_reallocation: AtomicBool::new(false),
            });
            s.lock().storage.insert(name, replacement.clone());
            storage = replacement;
        }
        let ps = &pc.drawable;
        let fb = {
            let mut inner = ps.lock();
            if inner.framebuffer == 0 {
                let mut f = 0u32;
                (gl.gen_framebuffer)(1, &mut f);
                inner.framebuffer = f;
            }
            inner.framebuffer
        };
        if fb == 0 {
            return fail(error, 11, "ANGLE presentation framebuffer allocation failed");
        }
        (gl.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, fb);
        (gl.attach)(GL_DRAW_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, 0x8D41, name);
        (gl.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, 0);
        context::present_framebuffer(presenter, fb, storage.geometry.pixel_size.width as usize, storage.geometry.pixel_size.height as usize, error)
    })();
    if let Some(b) = gl.bind_renderbuffer {
        b(0x8D41, old_renderbuffer as u32);
    }
    context::set_current(previous, std::ptr::null_mut());
    result
}

pub unsafe extern "C" fn present_other_thread_imp(this: Id, _: Sel, error: *mut Id) -> bool {
    present_other_thread(this, error)
}

pub unsafe fn release_native(this: Id) {
    if current() != this {
        return;
    }
    if context::has_window_surface(this) {
        let mut error: Id = NIL;
        if !context::detach_layer(this, &mut error) {
            let reason = if error.is_null() { "unknown".to_string() } else { rt::ns_string(msg0(error, "localizedDescription")) };
            backend::trace(&format!("[PearRuntime][angle] shared-thread surface release failed: {reason}\n"));
        }
    }
    // Make this thread's rendering visible to the presenter context.
    if let Some(f) = api().and_then(|g| g.flush) {
        f();
    }
}

pub unsafe extern "C" fn release_native_imp(this: Id, _: Sel) {
    release_native(this)
}

pub unsafe fn accept(this: Id, layer: Id, error: *mut Id) -> bool {
    let found = state(this).and_then(|s| s.lock().storage.values().find(|v| v.layer == layer).cloned());
    let Some(m) = found else {
        return fail(error, 13, "Drawable storage must first be allocated on the context's own thread");
    };
    if !geometry_matches(&m) {
        m.needs_reallocation.store(true, Ordering::Release);
    }
    true
}

pub unsafe extern "C" fn accept_imp(this: Id, _: Sel, layer: Id, error: *mut Id) -> bool {
    accept(this, layer, error)
}

pub unsafe fn present_bound(this: Id, target: usize, error: *mut Id) -> bool {
    let Some(gl) = ready(this, target, error) else { return false };
    let mut name = 0;
    (gl.get)(0x8CA7, &mut name);
    let name = name as u32;
    let Some(s) = state(this) else { return false };
    let lookup = || s.lock().storage.get(&name).cloned();
    let Some(mut storage) = lookup().filter(|_| name != 0) else {
        return fail(error, 10, "No drawable storage for the bound renderbuffer");
    };
    if storage.needs_reallocation.load(Ordering::Acquire) {
        if !allocate(this, target, storage.layer, 1, storage.retained, error) {
            return false;
        }
        match lookup() {
            Some(st) => storage = st,
            None => return fail(error, 10, "No drawable storage for the bound renderbuffer"),
        }
    }
    // UIKit may transiently resize the layer before the logical fit returns:
    // reject that frame but keep valid GL storage.
    if !geometry_matches(&storage) {
        return fail(error, 12, "Layer geometry changed; waiting for fit or explicit reallocation");
    }
    if !storage_matches(&gl, name, &storage) {
        s.lock().storage.remove(&name);
        return fail(error, 10, "Renderbuffer storage changed; reallocate before presentation");
    }
    if !context::bind_layer(this, storage.layer, error) {
        return false;
    }
    let fb = {
        let mut inner = s.lock();
        if inner.framebuffer == 0 {
            let mut f = 0u32;
            (gl.gen_framebuffer)(1, &mut f);
            inner.framebuffer = f;
        }
        inner.framebuffer
    };
    if fb == 0 {
        return fail(error, 11, "ANGLE presentation framebuffer allocation failed");
    }
    let (mut read, mut draw) = (0, 0);
    (gl.get)(0x8CAA, &mut read);
    (gl.get)(0x8CA6, &mut draw);
    (gl.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, fb);
    (gl.attach)(GL_DRAW_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, 0x8D41, name);
    let result = context::present_framebuffer(this, fb, storage.geometry.pixel_size.width as usize, storage.geometry.pixel_size.height as usize, error);
    // Do not keep deleted/reused game renderbuffer names alive in our FBO.
    (gl.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, fb);
    (gl.attach)(GL_DRAW_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, 0x8D41, 0);
    (gl.bind_framebuffer)(GL_READ_FRAMEBUFFER, read as u32);
    (gl.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, draw as u32);
    result
}

pub unsafe extern "C" fn present_bound_imp(this: Id, _: Sel, target: usize, error: *mut Id) -> bool {
    present_bound(this, target, error)
}

/// Snapshot without touching GL objects (safe on an input thread).
pub unsafe fn copy_geometry(this: Id, rb: u32) -> Option<Geometry> {
    let s = state(this)?;
    let inner = s.lock();
    let storage = inner.storage.get(&rb)?;
    geometry_matches(storage).then_some(storage.geometry)
}

pub unsafe extern "C" fn copy_geometry_imp(this: Id, _: Sel, geometry: *mut Geometry, rb: u32) -> bool {
    if geometry.is_null() {
        return false;
    }
    match copy_geometry(this, rb) {
        Some(g) => {
            *geometry = g;
            true
        }
        None => false,
    }
}

/// Top-left layer coordinates → top-left drawable pixels; outside points and
/// stale generations are rejected, never clamped.
pub unsafe extern "C" fn map_point_imp(this: Id, _: Sel, point: CGPoint, rb: u32, generation: u64, pixel: *mut CGPoint) -> bool {
    if pixel.is_null() {
        return false;
    }
    let Some(g) = copy_geometry(this, rb) else { return false };
    let b = g.logical_bounds;
    if generation != g.generation || !point.x.is_finite() || !point.y.is_finite() {
        return false;
    }
    if !(point.x >= b.origin.x && point.x < b.origin.x + b.size.width && point.y >= b.origin.y && point.y < b.origin.y + b.size.height) {
        return false;
    }
    *pixel = CGPoint {
        x: (point.x - b.origin.x) * g.pixel_size.width / b.size.width,
        y: (point.y - b.origin.y) * g.pixel_size.height / b.size.height,
    };
    true
}
