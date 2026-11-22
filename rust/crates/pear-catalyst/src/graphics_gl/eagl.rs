//! Public EAGL selectors over ANGLE (`PearEAGLContext` / `PearEAGLSharegroup`,
//! exported as `EAGLContext` / `EAGLSharegroup` by symbol aliases). No Apple
//! implementation is used.

use super::backend::trace;
use super::context::{self, NIL};
use super::drawable;
use crate::rt;
use pear_objc::{msg0, msg1, nsstring, objc_release, objc_retain, Id, Sel};
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

type P = *mut c_void;

extern "C" {
    fn pthread_key_create(key: *mut usize, destructor: Option<unsafe extern "C" fn(P)>) -> i32;
    fn pthread_getspecific(key: usize) -> P;
    fn pthread_setspecific(key: usize, value: P) -> i32;
}

// ---------------------------------------------------------------------------
// PearEAGLSharegroup

struct Group {
    angle: Id,       // retained PearANGLESharegroup
    debug_label: Id, // copied
}

unsafe fn group(this: Id) -> &'static mut Group {
    let slot = eagl_sharegroup::slot(this);
    if (*slot).is_null() {
        let angle: Id = msg0(msg0::<Id>(context::angle_sharegroup_class(), "alloc"), "init");
        *slot = Box::into_raw(Box::new(Group { angle, debug_label: NIL })) as P;
    }
    &mut *(*slot as *mut Group)
}

unsafe extern "C" fn group_init(this: Id, _: Sel) -> Id {
    let this: Id = pear_objc::msg_super0(this, "PearEAGLSharegroup", "init");
    if !this.is_null() {
        group(this);
    }
    this
}

unsafe extern "C" fn group_dealloc(this: Id, _: Sel) {
    let slot = eagl_sharegroup::slot(this);
    if !(*slot).is_null() {
        let g = Box::from_raw(*slot as *mut Group);
        *slot = std::ptr::null_mut();
        for o in [g.angle, g.debug_label] {
            if !o.is_null() {
                objc_release(o);
            }
        }
    }
    pear_objc::super_dealloc(this, "PearEAGLSharegroup");
}

unsafe extern "C" fn group_label(this: Id, _: Sel) -> Id {
    group(this).debug_label
}

unsafe extern "C" fn group_set_label(this: Id, _: Sel, label: Id) {
    let g = group(this);
    let old = g.debug_label;
    g.debug_label = if label.is_null() { NIL } else { msg0(label, "copy") };
    if !old.is_null() {
        objc_release(old);
    }
}

crate::objc_class! {
    mod eagl_sharegroup = "PearEAGLSharegroup" : (NSOBJECT_CLASS, NSOBJECT_META),
    state: true,
    methods {
        "init" "@16@0:8" => group_init,
        "dealloc" "v16@0:8" => group_dealloc,
        "debugLabel" "@16@0:8" => group_label,
        "setDebugLabel:" "v24@0:8@16" => group_set_label,
    }
    class_methods {}
}

// ---------------------------------------------------------------------------
// PearEAGLContext

struct Ctx {
    api: usize,
    sharegroup: Id, // retained
    angle: Id,      // retained PearANGLEContext
    debug_label: Id,
    multi_threaded: bool,
    last_error: Mutex<usize>, // retained NSError (atomic property)
    reported_presentation: bool,
    shared_across_threads: bool,
}

unsafe fn state(this: Id) -> Option<&'static mut Ctx> {
    if this.is_null() {
        return None;
    }
    let p = *eagl_context::slot(this);
    if p.is_null() { None } else { Some(&mut *(p as *mut Ctx)) }
}

unsafe fn set_last_error(this: Id, error: Id) {
    let Some(c) = state(this) else { return };
    let mut slot = c.last_error.lock().unwrap_or_else(|e| e.into_inner());
    let old = *slot;
    *slot = if error.is_null() { 0 } else { objc_retain(error) as usize };
    if old != 0 {
        objc_release(old as Id);
    }
}

unsafe extern "C" fn release_adapter(value: P) {
    pear_objc::autorelease_pool(|| objc_release(value as Id));
}

fn adapter_key() -> Option<usize> {
    static KEY: OnceLock<Option<usize>> = OnceLock::new();
    *KEY.get_or_init(|| unsafe {
        let mut key = 0usize;
        (pthread_key_create(&mut key, Some(release_adapter)) == 0).then_some(key)
    })
}

unsafe extern "C" fn init_api(this: Id, _: Sel, api: usize) -> Id {
    init_api_group(this, std::ptr::null_mut(), api, NIL)
}

unsafe extern "C" fn init_api_group(this: Id, _: Sel, api: usize, group_obj: Id) -> Id {
    let this: Id = pear_objc::msg_super0(this, "PearEAGLContext", "init");
    if this.is_null() {
        return NIL;
    }
    if adapter_key().is_none() || (!group_obj.is_null() && !msg1::<Id, bool>(group_obj, "isKindOfClass:", eagl_sharegroup::class())) {
        objc_release(this);
        return NIL;
    }
    let sharegroup = if group_obj.is_null() { msg0(msg0::<Id>(eagl_sharegroup::class(), "alloc"), "init") } else { objc_retain(group_obj) };
    let mut error: Id = NIL;
    let angle: Id = pear_objc::msg4(msg0::<Id>(context::class(), "alloc"), "initWithClientVersion:sharegroup:serializedThreadSharing:error:", api as isize, group(sharegroup).angle, true, &mut error as *mut Id);
    if angle.is_null() {
        objc_release(sharegroup);
        objc_release(this);
        return NIL;
    }
    *eagl_context::slot(this) = Box::into_raw(Box::new(Ctx {
        api,
        sharegroup,
        angle,
        debug_label: NIL,
        multi_threaded: false,
        last_error: Mutex::new(0),
        reported_presentation: false,
        shared_across_threads: false,
    })) as P;
    if rt::env_set("PEAR_GRAPHICS_BACKEND") {
        trace(&format!("[PearRuntime][angle] context GLES{api} created\n"));
    }
    this
}

/// `+[PearEAGLContext currentContext]` (+0).
pub fn current() -> Id {
    match adapter_key() {
        Some(k) => unsafe { pthread_getspecific(k) as Id },
        None => NIL,
    }
}

unsafe extern "C" fn current_imp(_: Id, _: Sel) -> Id {
    current()
}

pub unsafe fn set_current(context_obj: Id) -> bool {
    let Some(key) = adapter_key() else { return false };
    if !context_obj.is_null() && !msg1::<Id, bool>(context_obj, "isKindOfClass:", eagl_context::class()) {
        return false;
    }
    let previous = current();
    let mut error: Id = NIL;
    let angle = state(context_obj).map(|c| c.angle).unwrap_or(NIL);
    // Never claim success with a nil native context: GL commands would be
    // silently discarded.
    if !context::set_current(angle, &mut error) {
        static FAILURES: AtomicU32 = AtomicU32::new(0);
        if FAILURES.fetch_add(1, Ordering::Relaxed) < 8 {
            let reason = if error.is_null() { "unknown".to_string() } else { rt::ns_string(msg0(error, "localizedDescription")) };
            trace(&format!("[PearRuntime][angle] EAGL setCurrentContext failed: {reason}\n"));
        }
        set_last_error(context_obj, error);
        return false;
    }
    if state(context_obj).is_some_and(|c| c.shared_across_threads) {
        drawable::release_native(angle);
    }
    let replacement = if context_obj.is_null() { NIL } else { objc_retain(context_obj) };
    let old = pthread_getspecific(key) as Id;
    if pthread_setspecific(key, replacement as P) != 0 {
        if !replacement.is_null() {
            objc_release(replacement);
        }
        context::set_current(state(previous).map(|c| c.angle).unwrap_or(NIL), std::ptr::null_mut());
        return false;
    }
    set_last_error(context_obj, NIL);
    if !old.is_null() {
        objc_release(old);
    }
    true
}

unsafe extern "C" fn set_current_imp(_: Id, _: Sel, context_obj: Id) -> bool {
    set_current(context_obj)
}

unsafe fn adapter_error(this: Id, code: isize, message: &str) -> bool {
    set_last_error(this, rt::error("PearRuntime.ANGLE.EAGL", code, message));
    false
}

/// EAGL drawable calls act on the receiver, current or not (GTA III sizes its
/// drawable while no context is current): make it current for the call and
/// restore the thread's previous one.
unsafe fn with_current(angle: Id, error: *mut Id, body: impl FnOnce() -> bool) -> bool {
    let previous = context::current();
    if previous == angle {
        return body();
    }
    if !context::set_current(angle, error) {
        return false;
    }
    let result = body();
    context::set_current(previous, std::ptr::null_mut());
    result
}

fn error_is(error: Id, domain: &str, code: isize) -> bool {
    unsafe { !error.is_null() && rt::ns_string(msg0(error, "domain")) == domain && msg0::<isize>(error, "code") == code }
}

unsafe extern "C" fn renderbuffer_storage(this: Id, _: Sel, target: usize, drawable_obj: Id) -> bool {
    set_last_error(this, NIL);
    let Some(c) = state(this) else { return false };
    if !drawable_obj.is_null() && !pear_objc::is_kind_of(drawable_obj, "CALayer") {
        return adapter_error(this, 1, "EAGL drawable must be a CALayer host");
    }
    let mut properties: Id = NIL;
    if !drawable_obj.is_null() && pear_objc::responds_to(drawable_obj, "drawableProperties") {
        properties = msg0(drawable_obj, "drawableProperties");
        if !properties.is_null() && !pear_objc::is_kind_of(properties, "NSDictionary") {
            return adapter_error(this, 2, "Invalid EAGL drawable properties");
        }
    }
    let get = |k: &str| -> Id { if properties.is_null() { NIL } else { msg1(properties, "objectForKey:", nsstring(k)) } };
    let equal = |a: Id, b: &str| !a.is_null() && msg1::<Id, bool>(a, "isEqual:", nsstring(b));
    let format = get("EAGLDrawablePropertyColorFormat");
    // RGB565 (early titles) is stored as RGBA8: same colours, higher precision,
    // composited opaque since it has no alpha.
    let rgb565 = equal(format, "EAGLColorFormat565");
    if !format.is_null() && !equal(format, "EAGLColorFormat8888") && !rgb565 {
        trace(&format!("[PearRuntime][angle] unsupported EAGL colour format {}\n", rt::description(format)));
        return adapter_error(this, 3, "Only EAGL RGBA8 and RGB565 drawables are implemented");
    }
    let retained = get("EAGLDrawablePropertyRetained");
    if !retained.is_null() && !pear_objc::is_kind_of(retained, "NSNumber") {
        return adapter_error(this, 4, "Retained backing must be a boolean");
    }
    if rgb565 && !drawable_obj.is_null() && !msg0::<bool>(drawable_obj, "isOpaque") {
        if msg0::<bool>(pear_objc::cls("NSThread"), "isMainThread") {
            msg1::<bool, ()>(drawable_obj, "setOpaque:", true);
        } else {
            let layer = objc_retain(drawable_obj) as usize;
            rt::async_main(move || unsafe {
                msg1::<bool, ()>(layer as Id, "setOpaque:", true);
                objc_release(layer as Id);
            });
        }
    }
    let retained_flag = !retained.is_null() && msg0::<bool>(retained, "boolValue");
    let angle = c.angle;
    let mut error: Id = NIL;
    let mut inner: Id = NIL;
    let mut result = with_current(angle, &mut error, || drawable::allocate(angle, target, drawable_obj, 1, retained_flag, &mut inner));
    if error.is_null() {
        error = inner;
    }
    // Current on the app's render thread while layout (main) resizes the view.
    if !result && error_is(error, "PearRuntime.ANGLE.Context", 7) && !drawable_obj.is_null() && pear_objc::is_kind_of(drawable_obj, "CALayer") {
        let mut accept_error: Id = NIL;
        result = drawable::accept(angle, drawable_obj, &mut accept_error);
        error = if result { NIL } else { accept_error };
    }
    set_last_error(this, error);
    // Failures are always logged (bounded); every allocation with PEAR_DEBUG_GL.
    static FAILURES: AtomicU32 = AtomicU32::new(0);
    let verbose = rt::env_set("PEAR_DEBUG_GL") || rt::env_set("PEAR_GRAPHICS_BACKEND");
    if verbose || (!result && FAILURES.fetch_add(1, Ordering::Relaxed) < 8) {
        let (w, h) = context::ctx(angle).map(|c| (c.width, c.height)).unwrap_or((0, 0));
        let reason = if error.is_null() { "none".to_string() } else { rt::ns_string(msg0(error, "localizedDescription")) };
        trace(&format!("[PearRuntime][angle] storage {w}x{h} result={} error={reason}\n", result as i32));
    }
    result
}

unsafe extern "C" fn present_renderbuffer(this: Id, _: Sel, target: usize) -> bool {
    let Some(c) = state(this) else { return false };
    let angle = c.angle;
    let mut error: Id = NIL;
    let mut inner: Id = NIL;
    let mut result = with_current(angle, &mut error, || drawable::present_bound(angle, target, &mut inner));
    if error.is_null() {
        error = inner;
    }
    if !result && error_is(error, "PearRuntime.ANGLE.Context", 7) {
        let mut shared_error: Id = NIL;
        result = drawable::present_other_thread(angle, &mut shared_error);
        error = if result { NIL } else { shared_error };
    }
    set_last_error(this, error);
    static FAILURES: AtomicU32 = AtomicU32::new(0);
    if !result && FAILURES.fetch_add(1, Ordering::Relaxed) < 8 {
        let reason = if error.is_null() { "(null)".to_string() } else { rt::ns_string(msg0(error, "localizedDescription")) };
        trace(&format!("[PearRuntime][angle] presentation failed: {reason}\n"));
    }
    if result && !c.reported_presentation {
        c.reported_presentation = true;
        trace("[PearRuntime][angle] first presentation succeeded\n");
    }
    result
}

unsafe extern "C" fn present_scheduled(this: Id, _: Sel, _target: usize, _time: f64) -> bool {
    adapter_error(this, 5, "Scheduled presentation is not implemented; use immediate presentation")
}

unsafe extern "C" fn api_imp(this: Id, _: Sel) -> usize {
    state(this).map(|c| c.api).unwrap_or(0)
}
unsafe extern "C" fn sharegroup_imp(this: Id, _: Sel) -> Id {
    state(this).map(|c| c.sharegroup).unwrap_or(NIL)
}
unsafe extern "C" fn angle_imp(this: Id, _: Sel) -> Id {
    state(this).map(|c| c.angle).unwrap_or(NIL)
}
unsafe extern "C" fn label_imp(this: Id, _: Sel) -> Id {
    state(this).map(|c| c.debug_label).unwrap_or(NIL)
}
unsafe extern "C" fn set_label_imp(this: Id, _: Sel, label: Id) {
    if let Some(c) = state(this) {
        let old = c.debug_label;
        c.debug_label = if label.is_null() { NIL } else { msg0(label, "copy") };
        if !old.is_null() {
            objc_release(old);
        }
    }
}
unsafe extern "C" fn multi_threaded_imp(this: Id, _: Sel) -> bool {
    state(this).is_some_and(|c| c.multi_threaded)
}
unsafe extern "C" fn set_multi_threaded_imp(this: Id, _: Sel, v: bool) {
    if let Some(c) = state(this) {
        c.multi_threaded = v;
    }
}
unsafe extern "C" fn last_error_imp(this: Id, _: Sel) -> Id {
    let Some(c) = state(this) else { return NIL };
    let e = *c.last_error.lock().unwrap_or_else(|e| e.into_inner());
    // Atomic property getter: retain + autorelease.
    if e == 0 { NIL } else { pear_objc::autoreleased(objc_retain(e as Id)) }
}

unsafe extern "C" fn context_dealloc(this: Id, _: Sel) {
    let slot = eagl_context::slot(this);
    if !(*slot).is_null() {
        let c = Box::from_raw(*slot as *mut Ctx);
        *slot = std::ptr::null_mut();
        let e = *c.last_error.lock().unwrap_or_else(|e| e.into_inner());
        for o in [c.sharegroup, c.angle, c.debug_label, e as Id] {
            if !o.is_null() {
                objc_release(o);
            }
        }
    }
    pear_objc::super_dealloc(this, "PearEAGLContext");
}

crate::objc_class! {
    mod eagl_context = "PearEAGLContext" : (NSOBJECT_CLASS, NSOBJECT_META),
    state: true,
    methods {
        "initWithAPI:" "@24@0:8Q16" => init_api,
        "initWithAPI:sharegroup:" "@32@0:8Q16@24" => init_api_group,
        "API" "Q16@0:8" => api_imp,
        "sharegroup" "@16@0:8" => sharegroup_imp,
        "debugLabel" "@16@0:8" => label_imp,
        "setDebugLabel:" "v24@0:8@16" => set_label_imp,
        "isMultiThreaded" "B16@0:8" => multi_threaded_imp,
        "setMultiThreaded:" "v20@0:8B16" => set_multi_threaded_imp,
        "lastError" "@16@0:8" => last_error_imp,
        "angleContext" "@16@0:8" => angle_imp,
        "renderbufferStorage:fromDrawable:" "B32@0:8Q16@24" => renderbuffer_storage,
        "presentRenderbuffer:" "B24@0:8Q16" => present_renderbuffer,
        "presentRenderbuffer:atTime:" "B32@0:8Q16d24" => present_scheduled,
        "presentRenderbuffer:afterMinimumDuration:" "B32@0:8Q16d24" => present_scheduled,
        "dealloc" "v16@0:8" => context_dealloc,
    }
    class_methods {
        "currentContext" "@16@0:8" => current_imp,
        "setCurrentContext:" "B24@0:8@16" => set_current_imp,
    }
}

pub fn context_class() -> pear_objc::Class {
    eagl_context::class()
}

pub fn sharegroup_class() -> pear_objc::Class {
    eagl_sharegroup::class()
}

/// The PearEAGLContext's ANGLE context (for the geometry shim).
pub unsafe fn api_of(this: Id) -> usize {
    state(this).map(|c| c.api).unwrap_or(0)
}
