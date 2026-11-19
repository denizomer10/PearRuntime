//! PearHost OpenGLES.framework (EAGL on Metal-only PearANGLE) in Rust.

use pear_objc::{
    replace_method as add_method, cls, msg0, msg1, msg2, object_getClass, objc_getAssociatedObject,
    objc_setAssociatedObject, sel, Class, Id, Imp, NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::OnceLock;

const EGL_TRUE: i32 = 1;
const EGL_NONE: i32 = 0x3038;
const EGL_RED_SIZE: i32 = 0x3024;
const EGL_GREEN_SIZE: i32 = 0x3023;
const EGL_BLUE_SIZE: i32 = 0x3022;
const EGL_ALPHA_SIZE: i32 = 0x3021;
const EGL_SURFACE_TYPE: i32 = 0x3033;
const EGL_WINDOW_BIT: i32 = 0x0004;
const EGL_PBUFFER_BIT: i32 = 0x0001;
const EGL_RENDERABLE_TYPE: i32 = 0x3040;
const EGL_OPENGL_ES2_BIT: i32 = 0x0004;
const EGL_OPENGL_ES3_BIT: i32 = 0x0040;
const EGL_WIDTH: i32 = 0x3057;
const EGL_HEIGHT: i32 = 0x3056;
const EGL_CONTEXT_CLIENT_VERSION: i32 = 0x3098;
const EGL_OPENGL_ES_API: u32 = 0x30A0;
const EGL_PLATFORM_ANGLE_ANGLE: i32 = 0x3202;
const EGL_PLATFORM_ANGLE_TYPE_ANGLE: i32 = 0x3203;
const EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE: i32 = 0x3489;

const RTLD_LAZY: c_int = 0x1;
const RTLD_LOCAL: c_int = 0x4;

// Missing-call reporting lives in UIKit.framework (pear-core); OpenGLES.framework
// links against it instead of carrying a second reporter with its own state.
const PEAR_MISSING_BEHAVIOR: c_int = 3;

extern "C" {
    fn PearMissingReport(kind: c_int, owner: *const c_char, name: *const c_char, caller: *const c_void);
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type EglDisplay = *mut c_void;
type EglConfig = *mut c_void;
type EglContext = *mut c_void;
type EglSurface = *mut c_void;

struct EglBridge {
    display: EglDisplay,
    config: EglConfig,
    fn_create_context: Option<unsafe extern "C" fn(EglDisplay, EglConfig, EglContext, *const i32) -> EglContext>,
    fn_destroy_context: Option<unsafe extern "C" fn(EglDisplay, EglContext) -> i32>,
    fn_create_pbuffer: Option<unsafe extern "C" fn(EglDisplay, EglConfig, *const i32) -> EglSurface>,
    fn_create_window: Option<unsafe extern "C" fn(EglDisplay, EglConfig, *mut c_void, *const i32) -> EglSurface>,
    fn_destroy_surface: Option<unsafe extern "C" fn(EglDisplay, EglSurface) -> i32>,
    fn_make_current: Option<unsafe extern "C" fn(EglDisplay, EglSurface, EglSurface, EglContext) -> i32>,
    fn_swap_buffers: Option<unsafe extern "C" fn(EglDisplay, EglSurface) -> i32>,
}

unsafe impl Send for EglBridge {}
unsafe impl Sync for EglBridge {}

static EGL_BRIDGE: OnceLock<EglBridge> = OnceLock::new();

unsafe fn sym<T>(handle: *mut c_void, name: &str) -> Option<T> {
    if handle.is_null() {
        return None;
    }
    let c = CString::new(name).ok()?;
    let p = dlsym(handle, c.as_ptr());
    if p.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy(&p))
    }
}

fn egl_bridge() -> &'static EglBridge {
    EGL_BRIDGE.get_or_init(|| unsafe {
        let p1 = CString::new("@rpath/libEGL.dylib").unwrap();
        let p2 = CString::new("libEGL.dylib").unwrap();
        let mut handle = dlopen(p1.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
        if handle.is_null() {
            handle = dlopen(p2.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
        }

        let fn_get_platform_display: Option<unsafe extern "C" fn(i32, *mut c_void, *const isize) -> EglDisplay> =
            sym(handle, "eglGetPlatformDisplay");
        let fn_initialize: Option<unsafe extern "C" fn(EglDisplay, *mut i32, *mut i32) -> i32> =
            sym(handle, "eglInitialize");
        let fn_bind_api: Option<unsafe extern "C" fn(u32) -> i32> = sym(handle, "eglBindAPI");
        let fn_choose_config: Option<unsafe extern "C" fn(EglDisplay, *const i32, *mut EglConfig, i32, *mut i32) -> i32> =
            sym(handle, "eglChooseConfig");

        let mut display = std::ptr::null_mut();
        let mut config = std::ptr::null_mut();

        if let (Some(get_disp), Some(init_fn), Some(bind_fn), Some(choose_fn)) =
            (fn_get_platform_display, fn_initialize, fn_bind_api, fn_choose_config)
        {
            let attribs: [isize; 3] = [
                EGL_PLATFORM_ANGLE_TYPE_ANGLE as isize,
                EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE as isize,
                EGL_NONE as isize,
            ];
            let d = get_disp(EGL_PLATFORM_ANGLE_ANGLE, std::ptr::null_mut(), attribs.as_ptr());
            if !d.is_null() && init_fn(d, std::ptr::null_mut(), std::ptr::null_mut()) == EGL_TRUE {
                bind_fn(EGL_OPENGL_ES_API);
                let cfg_attribs: [i32; 13] = [
                    EGL_SURFACE_TYPE, EGL_WINDOW_BIT | EGL_PBUFFER_BIT,
                    EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT | EGL_OPENGL_ES3_BIT,
                    EGL_RED_SIZE, 8,
                    EGL_GREEN_SIZE, 8,
                    EGL_BLUE_SIZE, 8,
                    EGL_ALPHA_SIZE, 8,
                    EGL_NONE,
                ];
                let mut cfg: EglConfig = std::ptr::null_mut();
                let mut count: i32 = 0;
                if choose_fn(d, cfg_attribs.as_ptr(), &mut cfg, 1, &mut count) == EGL_TRUE && count > 0 {
                    display = d;
                    config = cfg;
                }
            }
        }

        EglBridge {
            display,
            config,
            fn_create_context: sym(handle, "eglCreateContext"),
            fn_destroy_context: sym(handle, "eglDestroyContext"),
            fn_create_pbuffer: sym(handle, "eglCreatePbufferSurface"),
            fn_create_window: sym(handle, "eglCreateWindowSurface"),
            fn_destroy_surface: sym(handle, "eglDestroySurface"),
            fn_make_current: sym(handle, "eglMakeCurrent"),
            fn_swap_buffers: sym(handle, "eglSwapBuffers"),
        }
    })
}

impl EglBridge {
    fn is_available(&self) -> bool {
        !self.display.is_null() && !self.config.is_null()
    }

    unsafe fn create_context(&self, version: i32, share: EglContext) -> EglContext {
        let Some(f) = self.fn_create_context else { return std::ptr::null_mut() };
        let attribs = [EGL_CONTEXT_CLIENT_VERSION, version, EGL_NONE];
        f(self.display, self.config, share, attribs.as_ptr())
    }

    unsafe fn destroy_context(&self, ctx: EglContext) {
        if let (Some(f), false) = (self.fn_destroy_context, ctx.is_null()) {
            f(self.display, ctx);
        }
    }

    unsafe fn create_pbuffer(&self, w: i32, h: i32) -> EglSurface {
        let Some(f) = self.fn_create_pbuffer else { return std::ptr::null_mut() };
        let attribs = [EGL_WIDTH, w.max(1), EGL_HEIGHT, h.max(1), EGL_NONE];
        f(self.display, self.config, attribs.as_ptr())
    }

    unsafe fn create_window_surface(&self, layer: Id) -> EglSurface {
        let Some(f) = self.fn_create_window else { return std::ptr::null_mut() };
        f(self.display, self.config, layer, std::ptr::null())
    }

    unsafe fn destroy_surface(&self, surf: EglSurface) {
        if let (Some(f), false) = (self.fn_destroy_surface, surf.is_null()) {
            f(self.display, surf);
        }
    }

    unsafe fn make_current(&self, surf: EglSurface, ctx: EglContext) -> bool {
        let Some(f) = self.fn_make_current else { return false };
        f(self.display, surf, surf, ctx) == EGL_TRUE
    }

    unsafe fn swap_buffers(&self, surf: EglSurface) -> bool {
        let Some(f) = self.fn_swap_buffers else { return false };
        f(self.display, surf) == EGL_TRUE
    }
}

struct ContextState {
    api: usize,
    sharegroup: Id,
    egl_context: EglContext,
    default_surface: EglSurface,
    bound_surface: EglSurface,
    multi_threaded: bool,
}

static CTX_STATE_KEY: u8 = 0;
static SG_CTX_KEY: u8 = 0;
const CURRENT_CTX_KEY: &str = "PearHost.EAGLCurrentContext";

unsafe fn ctx_state(obj: Id) -> Option<&'static mut ContextState> {
    let val = objc_getAssociatedObject(obj, &CTX_STATE_KEY as *const u8 as *const c_void);
    if val.is_null() {
        return None;
    }
    let ptr: *mut c_void = msg0(val, "pointerValue");
    if ptr.is_null() {
        None
    } else {
        Some(&mut *(ptr as *mut ContextState))
    }
}

unsafe fn get_sg_ctx(sg: Id) -> EglContext {
    let val = objc_getAssociatedObject(sg, &SG_CTX_KEY as *const u8 as *const c_void);
    if val.is_null() {
        std::ptr::null_mut()
    } else {
        msg0(val, "pointerValue")
    }
}

unsafe fn set_sg_ctx(sg: Id, ctx: EglContext) {
    let val: Id = msg1::<*mut c_void, Id>(cls("NSValue"), "valueWithPointer:", ctx);
    objc_setAssociatedObject(sg, &SG_CTX_KEY as *const u8 as *const c_void, val, OBJC_ASSOCIATION_RETAIN);
}

unsafe extern "C" fn eagl_init_with_api(self_obj: Id, _cmd: pear_objc::Sel, api: usize) -> Id {
    msg2::<usize, Id, Id>(self_obj, "initWithAPI:sharegroup:", api, NIL)
}

unsafe extern "C" fn eagl_init_with_api_sg(
    self_obj: Id,
    _cmd: pear_objc::Sel,
    api: usize,
    sharegroup: Id,
) -> Id {
    let self_obj: Id = pear_objc::msg_super0(self_obj, "EAGLContext", "init");
    if self_obj.is_null() {
        return NIL;
    }
    // Only the ES 1/2/3 rendering APIs exist (kEAGLRenderingAPIOpenGLES1..3).
    if !(1..=3).contains(&api) {
        pear_objc::objc_release(self_obj);
        return NIL;
    }
    let bridge = egl_bridge();
    if !bridge.is_available() {
        let owner = CString::new("EAGLContext").unwrap();
        let name = CString::new("initWithAPI: (PearANGLE libEGL.dylib not loaded; returned nil)").unwrap();
        PearMissingReport(PEAR_MISSING_BEHAVIOR, owner.as_ptr(), name.as_ptr(), std::ptr::null());
        pear_objc::objc_release(self_obj);
        return NIL;
    }

    let sg = if !sharegroup.is_null() {
        pear_objc::objc_retain(sharegroup)
    } else {
        msg0(cls("EAGLSharegroup"), "new")
    };
    let shared_egl = get_sg_ctx(sg);
    let version = if api >= 3 { 3 } else { 2 };
    let egl_ctx = bridge.create_context(version, shared_egl);
    if egl_ctx.is_null() {
        pear_objc::objc_release(sg);
        pear_objc::objc_release(self_obj);
        return NIL;
    }
    if shared_egl.is_null() {
        set_sg_ctx(sg, egl_ctx);
    }
    let default_surf = bridge.create_pbuffer(16, 16);
    let state = Box::new(ContextState {
        api,
        sharegroup: sg,
        egl_context: egl_ctx,
        default_surface: default_surf,
        bound_surface: std::ptr::null_mut(),
        multi_threaded: false,
    });
    let ptr = Box::into_raw(state) as *mut c_void;
    let val: Id = msg1::<*mut c_void, Id>(cls("NSValue"), "valueWithPointer:", ptr);
    objc_setAssociatedObject(self_obj, &CTX_STATE_KEY as *const u8 as *const c_void, val, OBJC_ASSOCIATION_RETAIN);
    self_obj
}

unsafe extern "C" fn eagl_dealloc(self_obj: Id, _cmd: pear_objc::Sel) {
    let val = objc_getAssociatedObject(self_obj, &CTX_STATE_KEY as *const u8 as *const c_void);
    if !val.is_null() {
        let ptr: *mut c_void = msg0(val, "pointerValue");
        if !ptr.is_null() {
            let state = Box::from_raw(ptr as *mut ContextState);
            let bridge = egl_bridge();
            bridge.destroy_surface(state.bound_surface);
            bridge.destroy_surface(state.default_surface);
            bridge.destroy_context(state.egl_context);
            pear_objc::objc_release(state.sharegroup);
        }
        objc_setAssociatedObject(self_obj, &CTX_STATE_KEY as *const u8 as *const c_void, NIL, OBJC_ASSOCIATION_RETAIN);
    }
    pear_objc::super_dealloc(self_obj, "EAGLContext");
}

unsafe extern "C" fn eagl_api(self_obj: Id, _cmd: pear_objc::Sel) -> usize {
    ctx_state(self_obj).map(|s| s.api).unwrap_or(0)
}

unsafe extern "C" fn eagl_sharegroup(self_obj: Id, _cmd: pear_objc::Sel) -> Id {
    ctx_state(self_obj).map(|s| s.sharegroup).unwrap_or(NIL)
}

unsafe extern "C" fn eagl_is_multithreaded(self_obj: Id, _cmd: pear_objc::Sel) -> bool {
    ctx_state(self_obj).map(|s| s.multi_threaded).unwrap_or(false)
}

unsafe extern "C" fn eagl_set_multithreaded(self_obj: Id, _cmd: pear_objc::Sel, v: bool) {
    if let Some(s) = ctx_state(self_obj) {
        s.multi_threaded = v;
    }
}

unsafe extern "C" fn eagl_current_context(_self_cls: Id, _cmd: pear_objc::Sel) -> Id {
    let thread: Id = msg0(cls("NSThread"), "currentThread");
    let dict: Id = msg0(thread, "threadDictionary");
    msg1::<Id, Id>(dict, "objectForKey:", pear_objc::nsstring(CURRENT_CTX_KEY))
}

unsafe extern "C" fn eagl_set_current_context(_self_cls: Id, _cmd: pear_objc::Sel, context: Id) -> bool {
    let bridge = egl_bridge();
    if !bridge.is_available() {
        return context.is_null();
    }
    let thread: Id = msg0(cls("NSThread"), "currentThread");
    let dict: Id = msg0(thread, "threadDictionary");
    let key = pear_objc::nsstring(CURRENT_CTX_KEY);

    if !context.is_null() {
        let Some(state) = ctx_state(context) else { return false };
        let surf = if !state.bound_surface.is_null() {
            state.bound_surface
        } else {
            state.default_surface
        };
        if !bridge.make_current(surf, state.egl_context) {
            return false;
        }
        msg2::<Id, Id, ()>(dict, "setObject:forKey:", context, key);
        true
    } else {
        bridge.make_current(std::ptr::null_mut(), std::ptr::null_mut());
        msg1::<Id, ()>(dict, "removeObjectForKey:", key);
        true
    }
}

unsafe extern "C" fn eagl_renderbuffer_storage(
    self_obj: Id,
    _cmd: pear_objc::Sel,
    _target: usize,
    drawable: Id,
) -> bool {
    let bridge = egl_bridge();
    if !bridge.is_available() {
        return false;
    }
    let Some(state) = ctx_state(self_obj) else { return false };
    let is_current = eagl_current_context(NIL, sel("currentContext")) == self_obj;
    if is_current {
        // Never destroy the surface that is current on this thread.
        bridge.make_current(state.default_surface, state.egl_context);
    }
    bridge.destroy_surface(state.bound_surface);
    if !drawable.is_null() && pear_objc::is_kind_of(drawable, "CALayer") {
        state.bound_surface = bridge.create_window_surface(drawable);
        let ok = !state.bound_surface.is_null();
        if is_current {
            let surf = if ok { state.bound_surface } else { state.default_surface };
            bridge.make_current(surf, state.egl_context);
        }
        ok
    } else {
        state.bound_surface = std::ptr::null_mut();
        if eagl_current_context(NIL, sel("currentContext")) == self_obj {
            bridge.make_current(state.default_surface, state.egl_context);
        }
        true
    }
}

unsafe extern "C" fn eagl_present_renderbuffer(self_obj: Id, _cmd: pear_objc::Sel, _target: usize) -> bool {
    let bridge = egl_bridge();
    if !bridge.is_available() {
        return false;
    }
    let Some(state) = ctx_state(self_obj) else { return false };
    let surf = if !state.bound_surface.is_null() {
        state.bound_surface
    } else {
        state.default_surface
    };
    bridge.swap_buffers(surf)
}

#[no_mangle]
pub extern "C" fn pear_gles_register_classes() {
    unsafe {
        let eagl_cls: Class = cls("EAGLContext");
        if eagl_cls.is_null() {
            return;
        }
        let meta: Class = object_getClass(eagl_cls);
        add_method(eagl_cls, "initWithAPI:", eagl_init_with_api as Imp, "@@:Q");
        add_method(eagl_cls, "initWithAPI:sharegroup:", eagl_init_with_api_sg as Imp, "@@:Q@");
        add_method(eagl_cls, "dealloc", eagl_dealloc as Imp, "v@:");
        add_method(eagl_cls, "API", eagl_api as Imp, "Q@:");
        add_method(eagl_cls, "sharegroup", eagl_sharegroup as Imp, "@@:");
        add_method(eagl_cls, "isMultiThreaded", eagl_is_multithreaded as Imp, "B@:");
        add_method(eagl_cls, "setMultiThreaded:", eagl_set_multithreaded as Imp, "v@:B");
        add_method(eagl_cls, "renderbufferStorage:fromDrawable:", eagl_renderbuffer_storage as Imp, "B@:Q@");
        add_method(eagl_cls, "presentRenderbuffer:", eagl_present_renderbuffer as Imp, "B@:Q");

        add_method(meta, "currentContext", eagl_current_context as Imp, "@@:");
        add_method(meta, "setCurrentContext:", eagl_set_current_context as Imp, "B@:@");
    }
}

extern "C" {
    fn PearMissingInstallForImage(address_in_image: *const c_void);
}

/// Image initializer: methods onto the class symbols, then the missing-call reporter.
extern "C" fn image_load() {
    pear_gles_register_classes();
    unsafe { PearMissingInstallForImage(image_load as *const c_void) };
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_GLES_INIT: extern "C" fn() = image_load;

// ---- OpenGLES.framework ABI -------------------------------------------------
// The class symbols and string constants iOS binaries bind to. Every method
// is registered on these classes by `pear_gles_register_classes`.
mod abi {
    extern "C" {
        #[link_name = "OBJC_CLASS_$_NSObject"]
        static NSObject_CLASS: std::ffi::c_void;
        #[link_name = "OBJC_METACLASS_$_NSObject"]
        static NSObject_META: std::ffi::c_void;
    }

    pear_objc::image_info!();
    pear_objc::empty_class!(EAGLSharegroup : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
    pear_objc::empty_class!(EAGLContext : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);

    pear_objc::nsstring_constant!(kEAGLDrawablePropertyRetainedBacking = "EAGLDrawablePropertyRetainedBacking");
    pear_objc::nsstring_constant!(kEAGLDrawablePropertyColorFormat = "EAGLDrawablePropertyColorFormat");
    pear_objc::nsstring_constant!(kEAGLColorFormatRGBA8 = "EAGLColorFormatRGBA8");
    pear_objc::nsstring_constant!(kEAGLColorFormatRGB565 = "EAGLColorFormatRGB565");
    pear_objc::nsstring_constant!(kEAGLColorFormatSRGBA8 = "EAGLColorFormatSRGBA8");
}
