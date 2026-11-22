//! EGL ownership adapter: `PearANGLESharegroup` / `PearANGLEContext`. No system
//! EAGL objects enter ANGLE. Surface dimensions are pixels, never UIKit points.

use super::backend::{self, display, trace};
use super::drawable;
use crate::rt;
use pear_objc::{msg0, objc_release, objc_retain, CGRect, Id, Sel};
use std::ffi::{c_char, c_void, CStr};
use std::sync::OnceLock;

pub const NIL: Id = std::ptr::null_mut();
type P = *mut c_void;

// ---------------------------------------------------------------------------
// A process-wide recursive lock (the original `std::recursive_mutex`).

pub struct RecursiveLock(OnceLock<usize>);
extern "C" {
    fn pthread_mutexattr_init(a: *mut [u64; 2]) -> i32;
    fn pthread_mutexattr_settype(a: *mut [u64; 2], kind: i32) -> i32;
    fn pthread_mutex_init(m: *mut [u64; 8], a: *const [u64; 2]) -> i32;
    fn pthread_mutex_lock(m: *mut [u64; 8]) -> i32;
    fn pthread_mutex_unlock(m: *mut [u64; 8]) -> i32;
    fn pthread_self() -> usize;
    fn pthread_equal(a: usize, b: usize) -> i32;
    fn pthread_key_create(key: *mut usize, destructor: Option<unsafe extern "C" fn(P)>) -> i32;
    fn pthread_getspecific(key: usize) -> P;
    fn pthread_setspecific(key: usize, value: P) -> i32;
}
const PTHREAD_MUTEX_RECURSIVE: i32 = 2;

impl RecursiveLock {
    pub const fn new() -> Self {
        RecursiveLock(OnceLock::new())
    }
    fn raw(&self) -> *mut [u64; 8] {
        *self.0.get_or_init(|| unsafe {
            let m = Box::into_raw(Box::new([0u64; 8]));
            let mut attr = [0u64; 2];
            pthread_mutexattr_init(&mut attr);
            pthread_mutexattr_settype(&mut attr, PTHREAD_MUTEX_RECURSIVE);
            pthread_mutex_init(m, &attr);
            m as usize
        }) as *mut [u64; 8]
    }
    pub fn lock(&self) -> RecursiveGuard<'_> {
        unsafe { pthread_mutex_lock(self.raw()) };
        RecursiveGuard(self)
    }
}
pub struct RecursiveGuard<'a>(&'a RecursiveLock);
impl Drop for RecursiveGuard<'_> {
    fn drop(&mut self) {
        unsafe { pthread_mutex_unlock(self.0.raw()) };
    }
}

pub static OPERATIONS: RecursiveLock = RecursiveLock::new();

// ---------------------------------------------------------------------------
// EGL entry points

#[derive(Clone, Copy)]
pub struct Egl {
    pub choose: unsafe extern "C" fn(P, *const i32, *mut P, i32, *mut i32) -> u32,
    pub create: unsafe extern "C" fn(P, P, P, *const i32) -> P,
    pub destroy: unsafe extern "C" fn(P, P) -> u32,
    pub pbuffer: unsafe extern "C" fn(P, P, *const i32) -> P,
    pub destroy_surface: unsafe extern "C" fn(P, P) -> u32,
    pub make_current: unsafe extern "C" fn(P, P, P, P) -> u32,
    pub error: unsafe extern "C" fn() -> i32,
    pub swap: unsafe extern "C" fn(P, P) -> u32,
    pub release_thread: unsafe extern "C" fn() -> u32,
    pub window: unsafe extern "C" fn(P, P, P, *const i32) -> P,
    pub query_surface: unsafe extern "C" fn(P, P, i32, *mut i32) -> u32,
}

static EGL: OnceLock<Egl> = OnceLock::new();

unsafe fn ctx_error(error: *mut Id, code: isize, reason: &str) -> bool {
    if !error.is_null() {
        *error = rt::error("PearRuntime.ANGLE.Context", code, reason);
    }
    false
}

pub unsafe fn egl(error: *mut Id) -> Option<Egl> {
    if display().is_null() {
        ctx_error(error, 1, "ANGLE backend must be initialized first");
        return None;
    }
    if let Some(e) = EGL.get() {
        return Some(*e);
    }
    macro_rules! load {
        ($n:literal) => {
            match backend::proc(CStr::from_bytes_with_nul_unchecked(concat!($n, "\0").as_bytes())) {
                Some(f) => f,
                None => {
                    ctx_error(error, 2, "Required EGL context API is unavailable");
                    return None;
                }
            }
        };
    }
    let e = Egl {
        choose: load!("eglChooseConfig"),
        create: load!("eglCreateContext"),
        destroy: load!("eglDestroyContext"),
        pbuffer: load!("eglCreatePbufferSurface"),
        destroy_surface: load!("eglDestroySurface"),
        make_current: load!("eglMakeCurrent"),
        error: load!("eglGetError"),
        swap: load!("eglSwapBuffers"),
        release_thread: load!("eglReleaseThread"),
        window: load!("eglCreateWindowSurface"),
        query_surface: load!("eglQuerySurface"),
    };
    let _ = EGL.set(e);
    Some(e)
}

const EGL_NONE: i32 = 0x3038;
const EGL_TRUE: i32 = 1;
const EGL_WIDTH: i32 = 0x3057;
const EGL_HEIGHT: i32 = 0x3056;

// ---------------------------------------------------------------------------
// PearANGLESharegroup

pub struct Group {
    pub version: isize,
    pub members: Vec<usize>, // EGLContext handles
}

unsafe extern "C" fn group_init(this: Id, _: Sel) -> Id {
    let this: Id = pear_objc::msg_super0(this, "PearANGLESharegroup", "init");
    if !this.is_null() {
        *sharegroup::slot(this) = Box::into_raw(Box::new(Group { version: 0, members: Vec::new() })) as P;
    }
    this
}

unsafe extern "C" fn group_dealloc(this: Id, _: Sel) {
    let slot = sharegroup::slot(this);
    if !(*slot).is_null() {
        drop(Box::from_raw(*slot as *mut Group));
        *slot = std::ptr::null_mut();
    }
    pear_objc::super_dealloc(this, "PearANGLESharegroup");
}

pub unsafe fn group(this: Id) -> &'static mut Group {
    if (*sharegroup::slot(this)).is_null() {
        *sharegroup::slot(this) = Box::into_raw(Box::new(Group { version: 0, members: Vec::new() })) as P;
    }
    &mut *(*sharegroup::slot(this) as *mut Group)
}

crate::objc_class! {
    mod sharegroup = "PearANGLESharegroup" : (NSOBJECT_CLASS, NSOBJECT_META),
    state: true,
    methods {
        "init" "@16@0:8" => group_init,
        "dealloc" "v16@0:8" => group_dealloc,
    }
    class_methods {}
}

// ---------------------------------------------------------------------------
// PearANGLEContext

pub struct Ctx {
    pub context: P,
    pub surface: P,
    pub config: P,
    pub owner: usize,
    pub has_owner: bool,
    pub serialized: bool,
    pub bindings: usize,
    pub debug_installed: bool,
    pub width: usize,
    pub height: usize,
    pub layer: Id, // retained
    pub bounds: CGRect,
    pub scale: f64,
    pub client_version: isize,
    pub sharegroup: Id, // retained
    pub drawable: drawable::State,
    pub presenter: Id, // retained presenter context (shared-thread presentation)
}

pub unsafe fn ctx(this: Id) -> Option<&'static mut Ctx> {
    if this.is_null() {
        return None;
    }
    let p = *angle_context::slot(this);
    if p.is_null() { None } else { Some(&mut *(p as *mut Ctx)) }
}

static TLS_KEY: OnceLock<Option<usize>> = OnceLock::new();

pub fn tls_key() -> Option<usize> {
    *TLS_KEY.get_or_init(|| unsafe {
        let mut key = 0usize;
        (pthread_key_create(&mut key, Some(thread_exited)) == 0).then_some(key)
    })
}

/// A thread exited with a context current: unbind it and release EGL's
/// per-thread state.
unsafe extern "C" fn thread_exited(value: P) {
    pear_objc::autorelease_pool(|| {
        let context = value as Id;
        let _g = OPERATIONS.lock();
        if let Some(e) = EGL.get() {
            (e.make_current)(display(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            (e.release_thread)();
        }
        if let Some(c) = ctx(context) {
            if c.bindings > 0 {
                c.bindings -= 1;
            }
            c.has_owner = c.bindings > 0;
        }
        objc_release(context);
    });
}

/// `+[PearANGLEContext currentContext]` (+0).
pub fn current() -> Id {
    match tls_key() {
        Some(k) => unsafe { pthread_getspecific(k) as Id },
        None => NIL,
    }
}

unsafe extern "C" fn current_imp(_: Id, _: Sel) -> Id {
    current()
}

unsafe fn init_with(this: Id, version: isize, group_obj: Id, serialized: bool, error: *mut Id) -> Id {
    let this: Id = pear_objc::msg_super0(this, "PearANGLEContext", "init");
    if this.is_null() {
        return NIL;
    }
    let fail = |this: Id| -> Id {
        objc_release(this);
        NIL
    };
    let _g = OPERATIONS.lock();
    if tls_key().is_none() {
        ctx_error(error, 20, "Context thread-local storage is unavailable");
        return fail(this);
    }
    let Some(e) = egl(error) else { return fail(this) };
    if !(1..=3).contains(&version) {
        ctx_error(error, 3, "Supported GLES client versions are 1, 2 and 3");
        return fail(this);
    }
    let sharegroup_obj = if group_obj.is_null() { pear_objc::autoreleased(msg0(msg0::<Id>(sharegroup::class(), "alloc"), "init")) } else { group_obj };
    let g = group(sharegroup_obj);
    if g.version != 0 && g.version != version {
        ctx_error(error, 4, "A sharegroup cannot mix GLES client versions");
        return fail(this);
    }
    let renderable = match version {
        1 => 0x0001,
        2 => 0x0004,
        _ => 0x0040,
    };
    let config_attrs = [0x3033, 0x0001 | 0x0004, 0x3040, renderable, 0x3024, 8, 0x3023, 8, 0x3022, 8, 0x3021, 8, EGL_NONE];
    let mut config: P = std::ptr::null_mut();
    let mut count = 0;
    if (e.choose)(display(), config_attrs.as_ptr(), &mut config, 1, &mut count) == 0 || count != 1 {
        ctx_error(error, 5, "ANGLE has no compatible GLES configuration");
        return fail(this);
    }
    let shared = g.members.first().copied().unwrap_or(0) as P;
    // 0x5E01 is private to the PearRuntime ANGLE fork, not a Khronos token.
    let attributes = [0x3098, version as i32, if serialized { 0x5E01 } else { EGL_NONE }, EGL_TRUE, EGL_NONE];
    let context = (e.create)(display(), config, shared, attributes.as_ptr());
    if context.is_null() {
        ctx_error(error, 6, &format!("EGL context creation failed (0x{:x})", (e.error)()));
        return fail(this);
    }
    let state = Box::new(Ctx {
        context,
        surface: std::ptr::null_mut(),
        config,
        owner: 0,
        has_owner: false,
        serialized,
        bindings: 0,
        debug_installed: false,
        width: 0,
        height: 0,
        layer: NIL,
        bounds: CGRect::default(),
        scale: 0.0,
        client_version: version,
        sharegroup: objc_retain(sharegroup_obj),
        drawable: drawable::State::default(),
        presenter: NIL,
    });
    *angle_context::slot(this) = Box::into_raw(state) as P;
    if !resize(this, 1, 1, error) {
        return fail(this);
    }
    let g = group(sharegroup_obj);
    g.version = version;
    g.members.push(context as usize);
    this
}

unsafe extern "C" fn init_version(this: Id, _: Sel, version: isize, group_obj: Id, error: *mut Id) -> Id {
    init_with(this, version, group_obj, false, error)
}

unsafe extern "C" fn init_version_serialized(this: Id, _: Sel, version: isize, group_obj: Id, serialized: bool, error: *mut Id) -> Id {
    init_with(this, version, group_obj, serialized, error)
}

unsafe extern "C" fn sharegroup_imp(this: Id, _: Sel) -> Id {
    ctx(this).map(|c| c.sharegroup).unwrap_or(NIL)
}
unsafe extern "C" fn client_version_imp(this: Id, _: Sel) -> isize {
    ctx(this).map(|c| c.client_version).unwrap_or(0)
}
unsafe extern "C" fn pixel_width_imp(this: Id, _: Sel) -> usize {
    let _g = OPERATIONS.lock();
    ctx(this).map(|c| c.width).unwrap_or(0)
}
unsafe extern "C" fn pixel_height_imp(this: Id, _: Sel) -> usize {
    let _g = OPERATIONS.lock();
    ctx(this).map(|c| c.height).unwrap_or(0)
}

extern "C" {
    fn strstr(haystack: *const c_char, needle: *const c_char) -> *const c_char;
}

/// Opt-in GL_KHR_debug error messages (never consume the app's glGetError).
unsafe extern "C" fn debug_message(source: u32, kind: u32, id: u32, severity: u32, length: i32, message: *const c_char, _: P) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    if kind != 0x824C || COUNT.fetch_add(1, Ordering::Relaxed) >= 64 {
        return;
    }
    let text = if message.is_null() {
        String::new()
    } else {
        let n = (length.clamp(0, 1024)) as usize;
        String::from_utf8_lossy(std::slice::from_raw_parts(message as *const u8, n.min(CStr::from_ptr(message).to_bytes().len()))).into_owned()
    };
    trace(&format!("[PearRuntime][angle-validation] source=0x{source:x} id={id} severity=0x{severity:x} {text}\n"));
}

unsafe fn install_debug_messages() {
    let callback: Option<unsafe extern "C" fn(unsafe extern "C" fn(u32, u32, u32, u32, i32, *const c_char, P), P)> = backend::proc(c"glDebugMessageCallbackKHR");
    let enable: Option<unsafe extern "C" fn(u32)> = backend::proc(c"glEnable");
    let string: Option<unsafe extern "C" fn(u32) -> *const c_char> = backend::proc(c"glGetString");
    let extensions = string.map(|s| s(0x1F03)).unwrap_or(std::ptr::null());
    if let (Some(cb), Some(en), false) = (callback, enable, extensions.is_null()) {
        if !strstr(extensions, c"GL_KHR_debug".as_ptr()).is_null() {
            cb(debug_message, std::ptr::null_mut());
            en(0x92E0);
        }
    }
}

pub unsafe fn set_current(context: Id, error: *mut Id) -> bool {
    let _g = OPERATIONS.lock();
    let previous = current();
    if context == previous {
        return true;
    }
    let Some(e) = egl(error) else { return false };
    let thread = pthread_self();
    if let Some(c) = ctx(context) {
        if !c.serialized && c.has_owner && pthread_equal(c.owner, thread) == 0 {
            return ctx_error(error, 7, "Context is current on another thread");
        }
    }
    let (surface, handle) = ctx(context).map(|c| (c.surface, c.context)).unwrap_or((std::ptr::null_mut(), std::ptr::null_mut()));
    if (e.make_current)(display(), surface, surface, handle) == 0 {
        return ctx_error(error, 8, &format!("EGL make-current failed (0x{:x})", (e.error)()));
    }
    let key = tls_key().unwrap_or(0);
    let replacement = if context.is_null() { NIL } else { objc_retain(context) };
    let old = pthread_getspecific(key) as Id;
    if pthread_setspecific(key, replacement as P) != 0 {
        if !replacement.is_null() {
            objc_release(replacement);
        }
        let (ps, pc) = ctx(previous).map(|c| (c.surface, c.context)).unwrap_or((std::ptr::null_mut(), std::ptr::null_mut()));
        (e.make_current)(display(), ps, ps, pc);
        return ctx_error(error, 21, "Context thread-local binding failed");
    }
    if let Some(p) = ctx(previous) {
        if p.bindings > 0 {
            p.bindings -= 1;
        }
        p.has_owner = p.bindings > 0;
    }
    if let Some(c) = ctx(context) {
        c.bindings += 1;
        c.owner = thread;
        c.has_owner = true;
        if !c.debug_installed && rt::env_set("PEAR_DEBUG_GL") {
            c.debug_installed = true;
            install_debug_messages();
        }
    }
    if !old.is_null() {
        objc_release(old);
    }
    true
}

unsafe extern "C" fn set_current_imp(_: Id, _: Sel, context: Id, error: *mut Id) -> bool {
    set_current(context, error)
}

pub unsafe fn resize(this: Id, width: usize, height: usize, error: *mut Id) -> bool {
    let _g = OPERATIONS.lock();
    let Some(c) = ctx(this) else { return false };
    if !c.layer.is_null() {
        return ctx_error(error, 22, "Detach the native surface before allocating a pbuffer drawable");
    }
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return ctx_error(error, 9, "Drawable dimensions must be 1…8192 pixels");
    }
    if c.has_owner && current() != this {
        return ctx_error(error, 7, "Cannot resize a context owned by another thread");
    }
    if width == c.width && height == c.height {
        return true;
    }
    let Some(e) = egl(error) else { return false };
    let attributes = [EGL_WIDTH, width as i32, EGL_HEIGHT, height as i32, EGL_NONE];
    let replacement = (e.pbuffer)(display(), c.config, attributes.as_ptr());
    if replacement.is_null() {
        return ctx_error(error, 10, "EGL drawable allocation failed");
    }
    if c.has_owner && (e.make_current)(display(), replacement, replacement, c.context) == 0 {
        (e.destroy_surface)(display(), replacement);
        return ctx_error(error, 11, "EGL drawable switch failed; previous drawable retained");
    }
    let previous = c.surface;
    c.surface = replacement;
    c.width = width;
    c.height = height;
    if !previous.is_null() {
        (e.destroy_surface)(display(), previous);
    }
    true
}

unsafe extern "C" fn resize_imp(this: Id, _: Sel, width: usize, height: usize, error: *mut Id) -> bool {
    resize(this, width, height, error)
}

pub unsafe fn has_window_surface(this: Id) -> bool {
    let _g = OPERATIONS.lock();
    ctx(this).is_some_and(|c| !c.layer.is_null())
}

unsafe extern "C" fn has_window_surface_imp(this: Id, _: Sel) -> bool {
    has_window_surface(this)
}

unsafe fn layer_geometry(layer: Id) -> (CGRect, f64) {
    (msg0(layer, "bounds"), msg0(layer, "contentsScale"))
}

pub unsafe fn bind_layer(this: Id, layer: Id, error: *mut Id) -> bool {
    let _g = OPERATIONS.lock();
    if current() != this {
        return ctx_error(error, 12, "Native surface binding requires this context on the calling thread");
    }
    if layer.is_null() || !pear_objc::is_kind_of(layer, "CALayer") {
        return ctx_error(error, 23, "A native CALayer is required");
    }
    let Some(c) = ctx(this) else { return false };
    let (bounds, scale) = layer_geometry(layer);
    let (w, h) = (bounds.size.width * scale, bounds.size.height * scale);
    if !scale.is_finite() || scale <= 0.0 || bounds.size.width <= 0.0 || bounds.size.height <= 0.0 || !w.is_finite() || !h.is_finite() || w < 1.0 || h < 1.0 || w > 8192.0 || h > 8192.0 {
        return ctx_error(error, 9, "Native drawable pixel dimensions must be 1…8192");
    }
    if c.layer == layer {
        if rt::rect_eq(bounds, c.bounds) && scale == c.scale {
            return true;
        }
        return ctx_error(error, 24, "Native surface geometry changed; detach and rebind at a frame boundary");
    }
    let Some(e) = egl(error) else { return false };
    let attributes = [EGL_NONE];
    let replacement = (e.window)(display(), c.config, layer as P, attributes.as_ptr());
    if replacement.is_null() {
        return ctx_error(error, 25, &format!("EGL native surface creation failed (0x{:x})", (e.error)()));
    }
    if (e.make_current)(display(), replacement, replacement, c.context) == 0 {
        (e.destroy_surface)(display(), replacement);
        return ctx_error(error, 26, "Native surface binding failed; previous drawable retained");
    }
    let (mut aw, mut ah) = (0, 0);
    let valid = (e.query_surface)(display(), replacement, EGL_WIDTH, &mut aw) != 0 && (e.query_surface)(display(), replacement, EGL_HEIGHT, &mut ah) != 0 && aw > 0 && ah > 0;
    if !valid {
        (e.make_current)(display(), c.surface, c.surface, c.context);
        (e.destroy_surface)(display(), replacement);
        return ctx_error(error, 27, "EGL returned invalid native drawable dimensions");
    }
    let previous = c.surface;
    let previous_layer = c.layer; // released only after EGL has let go of it
    c.surface = replacement;
    c.width = aw as usize;
    c.height = ah as usize;
    c.layer = objc_retain(layer);
    c.bounds = bounds;
    c.scale = scale;
    (e.destroy_surface)(display(), previous);
    if !previous_layer.is_null() {
        objc_release(previous_layer);
    }
    true
}

unsafe extern "C" fn bind_layer_imp(this: Id, _: Sel, layer: Id, error: *mut Id) -> bool {
    bind_layer(this, layer, error)
}

pub unsafe fn detach_layer(this: Id, error: *mut Id) -> bool {
    let _g = OPERATIONS.lock();
    if current() != this {
        return ctx_error(error, 12, "Native surface detachment requires this context on the calling thread");
    }
    let Some(c) = ctx(this) else { return false };
    if c.layer.is_null() {
        return true;
    }
    let Some(e) = egl(error) else { return false };
    let attributes = [EGL_WIDTH, c.width as i32, EGL_HEIGHT, c.height as i32, EGL_NONE];
    let replacement = (e.pbuffer)(display(), c.config, attributes.as_ptr());
    if replacement.is_null() {
        return ctx_error(error, 10, "Replacement pbuffer allocation failed; native surface retained");
    }
    if (e.make_current)(display(), replacement, replacement, c.context) == 0 {
        (e.destroy_surface)(display(), replacement);
        return ctx_error(error, 11, "Replacement pbuffer binding failed; native surface retained");
    }
    let previous = c.surface;
    c.surface = replacement;
    (e.destroy_surface)(display(), previous);
    objc_release(c.layer);
    c.layer = NIL;
    true
}

unsafe extern "C" fn detach_layer_imp(this: Id, _: Sel, error: *mut Id) -> bool {
    detach_layer(this, error)
}

struct PresentGl {
    get: unsafe extern "C" fn(u32, *mut i32),
    bind_core: Option<unsafe extern "C" fn(u32, u32)>,
    bind_oes: Option<unsafe extern "C" fn(u32, u32)>,
    status_core: Option<unsafe extern "C" fn(u32) -> u32>,
    status_oes: Option<unsafe extern "C" fn(u32) -> u32>,
    blit_angle: Option<unsafe extern "C" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32)>,
    blit_core: Option<unsafe extern "C" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32)>,
    enabled: unsafe extern "C" fn(u32) -> u8,
    enable: unsafe extern "C" fn(u32),
    disable: unsafe extern "C" fn(u32),
    string: unsafe extern "C" fn(u32) -> *const c_char,
    read_buffer: Option<unsafe extern "C" fn(u32)>,
}
unsafe impl Send for PresentGl {}
unsafe impl Sync for PresentGl {}

/// Resolved once, after the immutable backend initialized: no per-frame
/// symbol lookup and no system-GL fallback.
fn present_gl() -> Option<&'static PresentGl> {
    static GL: OnceLock<Option<PresentGl>> = OnceLock::new();
    GL.get_or_init(|| unsafe {
        Some(PresentGl {
            get: backend::proc(c"glGetIntegerv")?,
            bind_core: backend::proc(c"glBindFramebuffer"),
            bind_oes: backend::proc(c"glBindFramebufferOES"),
            status_core: backend::proc(c"glCheckFramebufferStatus"),
            status_oes: backend::proc(c"glCheckFramebufferStatusOES"),
            blit_angle: backend::proc(c"glBlitFramebufferANGLE"),
            blit_core: backend::proc(c"glBlitFramebuffer"),
            enabled: backend::proc(c"glIsEnabled")?,
            enable: backend::proc(c"glEnable")?,
            disable: backend::proc(c"glDisable")?,
            string: backend::proc(c"glGetString")?,
            read_buffer: backend::proc(c"glReadBuffer"),
        })
    })
    .as_ref()
}

pub unsafe fn present_framebuffer(this: Id, framebuffer: u32, width: usize, height: usize, error: *mut Id) -> bool {
    let _g = OPERATIONS.lock();
    if current() != this {
        return ctx_error(error, 12, "Presentation requires this context on the calling thread");
    }
    let Some(c) = ctx(this) else { return false };
    if !c.layer.is_null() {
        let (bounds, scale) = layer_geometry(c.layer);
        if !rt::rect_eq(bounds, c.bounds) || scale != c.scale {
            return ctx_error(error, 24, "Native geometry changed; rebuild drawable and input geometry before presentation");
        }
    }
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return ctx_error(error, 13, "Framebuffer presentation requires valid pixel dimensions");
    }
    if c.client_version < 3 && (width != c.width || height != c.height) {
        return ctx_error(error, 19, "GLES 2 framebuffer blit requires identical source/drawable pixel dimensions");
    }
    let gl = present_gl();
    let v1 = c.client_version == 1;
    let bind = gl.and_then(|g| if v1 { g.bind_oes } else { g.bind_core });
    let status = gl.and_then(|g| if v1 { g.status_oes } else { g.status_core });
    let blit = gl.and_then(|g| if c.client_version >= 3 { g.blit_core } else { g.blit_angle });
    let (Some(gl), Some(bind), Some(status), Some(blit)) = (gl, bind, status, blit) else {
        return ctx_error(error, 14, "Required ANGLE framebuffer operations are unavailable");
    };
    if c.client_version >= 3 && gl.read_buffer.is_none() {
        return ctx_error(error, 14, "Required ANGLE framebuffer operations are unavailable");
    }
    if c.client_version < 3 {
        let extensions = (gl.string)(0x1F03);
        if extensions.is_null() || strstr(extensions, c"GL_ANGLE_framebuffer_blit".as_ptr()).is_null() {
            return ctx_error(error, 15, "GLES 2 presentation requires ANGLE_framebuffer_blit");
        }
    }
    const READ: u32 = 0x8CA8;
    const DRAW: u32 = 0x8CA9;
    if framebuffer != 0 {
        let (mut old_read, mut old_draw, mut source_read) = (0, 0, 0x8CE0);
        (gl.get)(0x8CAA, &mut old_read);
        (gl.get)(0x8CA6, &mut old_draw);
        let scissor = (gl.enabled)(0x0C11) != 0;
        let mut changed_read = false;
        let result = (|| {
            bind(READ, framebuffer);
            if status(READ) != 0x8CD5 {
                return Err((16, "Source framebuffer is incomplete"));
            }
            if c.client_version >= 3 {
                (gl.get)(0x0C02, &mut source_read);
                (gl.read_buffer.unwrap())(0x8CE0);
                changed_read = true;
            }
            bind(DRAW, 0);
            if status(DRAW) != 0x8CD5 {
                return Err((17, "ANGLE drawable framebuffer is incomplete"));
            }
            if scissor {
                (gl.disable)(0x0C11);
            }
            blit(0, 0, width as i32, height as i32, 0, 0, c.width as i32, c.height as i32, 0x4000, 0x2600);
            Ok(())
        })();
        if changed_read {
            (gl.read_buffer.unwrap())(source_read as u32);
        }
        bind(READ, old_read as u32);
        bind(DRAW, old_draw as u32);
        if scissor {
            (gl.enable)(0x0C11);
        }
        if let Err((code, message)) = result {
            return ctx_error(error, code, message);
        }
    }
    let Some(e) = egl(error) else { return false };
    if (e.swap)(display(), c.surface) == 0 {
        return ctx_error(error, 18, "EGL drawable presentation failed");
    }
    true
}

unsafe extern "C" fn present_framebuffer_imp(this: Id, _: Sel, framebuffer: u32, width: usize, height: usize, error: *mut Id) -> bool {
    present_framebuffer(this, framebuffer, width, height, error)
}

unsafe extern "C" fn context_dealloc(this: Id, _: Sel) {
    {
        let _g = OPERATIONS.lock();
        let slot = angle_context::slot(this);
        if !(*slot).is_null() {
            let c = Box::from_raw(*slot as *mut Ctx);
            *slot = std::ptr::null_mut();
            if let Some(e) = EGL.get() {
                if !c.context.is_null() {
                    group(c.sharegroup).members.retain(|m| *m != c.context as usize);
                    (e.destroy)(display(), c.context);
                }
                if !c.surface.is_null() {
                    (e.destroy_surface)(display(), c.surface);
                }
            }
            for obj in [c.layer, c.sharegroup, c.presenter] {
                if !obj.is_null() {
                    objc_release(obj);
                }
            }
        }
    }
    pear_objc::super_dealloc(this, "PearANGLEContext");
}

crate::objc_class! {
    mod angle_context = "PearANGLEContext" : (NSOBJECT_CLASS, NSOBJECT_META),
    state: true,
    methods {
        "initWithClientVersion:sharegroup:error:" "@40@0:8q16@24^@32" => init_version,
        "initWithClientVersion:sharegroup:serializedThreadSharing:error:" "@44@0:8q16@24B32^@36" => init_version_serialized,
        "sharegroup" "@16@0:8" => sharegroup_imp,
        "clientVersion" "q16@0:8" => client_version_imp,
        "pixelWidth" "Q16@0:8" => pixel_width_imp,
        "pixelHeight" "Q16@0:8" => pixel_height_imp,
        "resizePixelWidth:height:error:" "B40@0:8Q16Q24^@32" => resize_imp,
        "bindDrawableLayer:error:" "B32@0:8@16^@24" => bind_layer_imp,
        "detachDrawableLayerWithError:" "B24@0:8^@16" => detach_layer_imp,
        "hasWindowSurface" "B16@0:8" => has_window_surface_imp,
        "presentFramebuffer:width:height:error:" "B44@0:8I16Q20Q28^@36" => present_framebuffer_imp,
        "dealloc" "v16@0:8" => context_dealloc,
        "allocateBoundRenderbuffer:fromLayer:colorFormat:retainedBacking:error:" "B52@0:8Q16@24Q32B40^@44" => drawable::allocate_imp,
        "presentBoundRenderbuffer:error:" "B32@0:8Q16^@24" => drawable::present_bound_imp,
        "acceptDrawableLayer:fromOtherThreadWithError:" "B32@0:8@16^@24" => drawable::accept_imp,
        "presentDrawableFromOtherThreadWithError:" "B24@0:8^@16" => drawable::present_other_thread_imp,
        "releaseNativeSurfaceForSharedThreadUse" "v16@0:8" => drawable::release_native_imp,
        "forgetDrawableRenderbuffers:count:" "v28@0:8r^I16i24" => drawable::forget_imp,
        "copyDrawableGeometry:forRenderbuffer:" "B28@0:8^{?={CGRect={CGPoint=dd}{CGSize=dd}}{CGSize=dd}dQ}16I24" => drawable::copy_geometry_imp,
        "mapLayerPoint:forRenderbuffer:generation:toPixel:" "B48@0:8{CGPoint=dd}16I32Q36^{CGPoint=dd}44" => drawable::map_point_imp,
    }
    class_methods {
        "currentContext" "@16@0:8" => current_imp,
        "setCurrentContext:error:" "B32@0:8@16^@24" => set_current_imp,
    }
}

pub fn class() -> pear_objc::Class {
    angle_context::class()
}

pub fn angle_sharegroup_class() -> pear_objc::Class {
    sharegroup::class()
}

