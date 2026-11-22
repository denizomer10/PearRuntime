//! PearHost GLKit.framework (`GLKView` and `GLKViewController`) in Rust,
//! backed by PearHost OpenGLES (`PearANGLE`).

use pear_objc::{
    assoc_get_weak, assoc_set_weak, autorelease_pool, class_getSuperclass, cls, msg0, msg1, msg2,
    nsstring, object_getClass, objc_getAssociatedObject, objc_msgSendSuper, objc_release,
    objc_retain, objc_setAssociatedObject, replace_method as add_method, responds_to, sel,
    CGRect, CGSize, Class, Id, Imp, ObjcSuper, Sel, NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::ffi::{c_char, c_void, CString};
use std::sync::OnceLock;

#[link(name = "Foundation", kind = "framework")]
extern "C" {
    static NSRunLoopCommonModes: Id;
}

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {
    fn CACurrentMediaTime() -> f64;
}

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    fn PearMissingInstallForImage(address_in_image: *const c_void);
    fn PearHostLogMessage(area: *const c_char, message: *const c_char);
    #[link_name = "_Z19PearANGLEGameSymbolPKc"]
    fn PearANGLEGameSymbol(name: *const c_char) -> *mut c_void;
}

const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_RENDERBUFFER: u32 = 0x8D41;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_DEPTH_ATTACHMENT: u32 = 0x8D00;
const GL_STENCIL_ATTACHMENT: u32 = 0x8D20;
const GL_RENDERBUFFER_WIDTH: u32 = 0x8D42;
const GL_RENDERBUFFER_HEIGHT: u32 = 0x8D43;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_DEPTH_COMPONENT16: u32 = 0x81A5;
const GL_DEPTH_COMPONENT24_OES: u32 = 0x81A6;
const GL_DEPTH24_STENCIL8_OES: u32 = 0x88F0;

const GLK_DEPTH_NONE: isize = 0;
const GLK_DEPTH_16: isize = 1;
const GLK_STENCIL_NONE: isize = 0;

struct GlkFunctions {
    gen_framebuffers: unsafe extern "C" fn(i32, *mut u32),
    gen_renderbuffers: unsafe extern "C" fn(i32, *mut u32),
    delete_framebuffers: unsafe extern "C" fn(i32, *const u32),
    delete_renderbuffers: unsafe extern "C" fn(i32, *const u32),
    bind_framebuffer: unsafe extern "C" fn(u32, u32),
    bind_renderbuffer: unsafe extern "C" fn(u32, u32),
    renderbuffer_storage: unsafe extern "C" fn(u32, u32, i32, i32),
    framebuffer_renderbuffer: unsafe extern "C" fn(u32, u32, u32, u32),
    get_renderbuffer_parameteriv: unsafe extern "C" fn(u32, u32, *mut i32),
    check_framebuffer_status: unsafe extern "C" fn(u32) -> u32,
    viewport: unsafe extern "C" fn(i32, i32, i32, i32),
}

unsafe fn resolve_gl_fn<T>(name: &std::ffi::CStr) -> Option<T> {
    let ptr = PearANGLEGameSymbol(name.as_ptr());
    if ptr.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy(&ptr))
    }
}

fn gl_funcs() -> Option<&'static GlkFunctions> {
    static FUNCS: OnceLock<Option<GlkFunctions>> = OnceLock::new();
    FUNCS
        .get_or_init(|| unsafe {
            Some(GlkFunctions {
                gen_framebuffers: resolve_gl_fn(c"glGenFramebuffers")?,
                gen_renderbuffers: resolve_gl_fn(c"glGenRenderbuffers")?,
                delete_framebuffers: resolve_gl_fn(c"glDeleteFramebuffers")?,
                delete_renderbuffers: resolve_gl_fn(c"glDeleteRenderbuffers")?,
                bind_framebuffer: resolve_gl_fn(c"glBindFramebuffer")?,
                bind_renderbuffer: resolve_gl_fn(c"glBindRenderbuffer")?,
                renderbuffer_storage: resolve_gl_fn(c"glRenderbufferStorage")?,
                framebuffer_renderbuffer: resolve_gl_fn(c"glFramebufferRenderbuffer")?,
                get_renderbuffer_parameteriv: resolve_gl_fn(c"glGetRenderbufferParameteriv")?,
                check_framebuffer_status: resolve_gl_fn(c"glCheckFramebufferStatus")?,
                viewport: resolve_gl_fn(c"glViewport")?,
            })
        })
        .as_ref()
}

fn log_glkit(msg: &str) {
    let c = CString::new(msg).unwrap_or_default();
    unsafe { PearHostLogMessage(c"glkit".as_ptr(), c.as_ptr()) };
}

fn glk_view_class() -> Class {
    abi::GLKView::CLASS.ptr() as Class
}

fn glk_vc_class() -> Class {
    abi::GLKViewController::CLASS.ptr() as Class
}

unsafe fn super0<R>(receiver: Id, defining: Class, selector: &str) -> R {
    let mut call = ObjcSuper {
        receiver,
        super_class: class_getSuperclass(defining),
    };
    let send: unsafe extern "C" fn(*mut ObjcSuper, Sel) -> R =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    send(&mut call, sel(selector))
}

unsafe fn super1<A, R>(receiver: Id, defining: Class, selector: &str, arg: A) -> R {
    let mut call = ObjcSuper {
        receiver,
        super_class: class_getSuperclass(defining),
    };
    let send: unsafe extern "C" fn(*mut ObjcSuper, Sel, A) -> R =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    send(&mut call, sel(selector), arg)
}

unsafe fn super2<A1, A2, R>(receiver: Id, defining: Class, selector: &str, a1: A1, a2: A2) -> R {
    let mut call = ObjcSuper {
        receiver,
        super_class: class_getSuperclass(defining),
    };
    let send: unsafe extern "C" fn(*mut ObjcSuper, Sel, A1, A2) -> R =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    send(&mut call, sel(selector), a1, a2)
}

unsafe fn set_retained(slot: &mut Id, new_val: Id) {
    if *slot == new_val {
        return;
    }
    if !(*slot).is_null() {
        objc_release(*slot);
    }
    *slot = if new_val.is_null() {
        NIL
    } else {
        objc_retain(new_val)
    };
}

// ============================================================================
// GLKView
// ============================================================================

struct GlkViewState {
    context: Id,
    color_format: isize,
    depth_format: isize,
    stencil_format: isize,
    multisample: isize,
    enable_set_needs_display: bool,
    framebuffer: u32,
    color: u32,
    depth: u32,
    allocated_pixels: CGSize,
    display_scheduled: bool,
}

static GLK_VIEW_STATE_KEY: u8 = 0;
static GLK_VIEW_DELEGATE_KEY: u8 = 0;

unsafe fn glk_view_state(obj: Id) -> Option<&'static mut GlkViewState> {
    let val = objc_getAssociatedObject(obj, &GLK_VIEW_STATE_KEY as *const u8 as *const c_void);
    if val.is_null() {
        return None;
    }
    let ptr: *mut c_void = msg0(val, "pointerValue");
    if ptr.is_null() {
        None
    } else {
        Some(&mut *(ptr as *mut GlkViewState))
    }
}

unsafe fn glk_view_setup(obj: Id) {
    let state = Box::new(GlkViewState {
        context: NIL,
        color_format: 0,
        depth_format: GLK_DEPTH_NONE,
        stencil_format: GLK_STENCIL_NONE,
        multisample: 0,
        enable_set_needs_display: true,
        framebuffer: 0,
        color: 0,
        depth: 0,
        allocated_pixels: CGSize::default(),
        display_scheduled: false,
    });
    let ptr = Box::into_raw(state) as *mut c_void;
    let val: Id = msg1::<*mut c_void, Id>(cls("NSValue"), "valueWithPointer:", ptr);
    objc_setAssociatedObject(
        obj,
        &GLK_VIEW_STATE_KEY as *const u8 as *const c_void,
        val,
        OBJC_ASSOCIATION_RETAIN,
    );
    msg1::<bool, ()>(obj, "setOpaque:", true);
    let scale: f64 = msg0(msg0(cls("UIScreen"), "mainScreen"), "scale");
    msg1::<f64, ()>(obj, "setContentScaleFactor:", scale);
}

unsafe extern "C" fn glk_view_layer_class(_cls: Id, _cmd: Sel) -> Class {
    let eagl = cls("CAEAGLLayer");
    if !eagl.is_null() {
        eagl
    } else {
        cls("CALayer")
    }
}

unsafe extern "C" fn glk_view_init_with_frame(self_obj: Id, _cmd: Sel, frame: CGRect) -> Id {
    let obj: Id = super1(self_obj, glk_view_class(), "initWithFrame:", frame);
    if !obj.is_null() {
        glk_view_setup(obj);
    }
    obj
}

unsafe extern "C" fn glk_view_init_with_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = super1(self_obj, glk_view_class(), "initWithCoder:", coder);
    if !obj.is_null() {
        glk_view_setup(obj);
    }
    obj
}

unsafe extern "C" fn glk_view_init_with_frame_context(
    self_obj: Id,
    _cmd: Sel,
    frame: CGRect,
    context: Id,
) -> Id {
    let obj: Id = msg1(self_obj, "initWithFrame:", frame);
    if !obj.is_null() {
        msg1::<Id, ()>(obj, "setContext:", context);
    }
    obj
}

unsafe extern "C" fn glk_view_dealloc(self_obj: Id, _cmd: Sel) {
    msg0::<()>(self_obj, "deleteDrawable");
    let val = objc_getAssociatedObject(
        self_obj,
        &GLK_VIEW_STATE_KEY as *const u8 as *const c_void,
    );
    if !val.is_null() {
        let ptr: *mut c_void = msg0(val, "pointerValue");
        if !ptr.is_null() {
            let mut s = Box::from_raw(ptr as *mut GlkViewState);
            set_retained(&mut s.context, NIL);
        }
        objc_setAssociatedObject(
            self_obj,
            &GLK_VIEW_STATE_KEY as *const u8 as *const c_void,
            NIL,
            OBJC_ASSOCIATION_RETAIN,
        );
    }
    super0::<()>(self_obj, glk_view_class(), "dealloc");
}

unsafe extern "C" fn glk_view_context(self_obj: Id, _cmd: Sel) -> Id {
    glk_view_state(self_obj).map(|s| s.context).unwrap_or(NIL)
}

unsafe extern "C" fn glk_view_set_context(self_obj: Id, _cmd: Sel, context: Id) {
    let Some(s) = glk_view_state(self_obj) else {
        return;
    };
    if s.context == context {
        return;
    }
    msg0::<()>(self_obj, "deleteDrawable");
    set_retained(&mut s.context, context);
}

unsafe extern "C" fn glk_view_delegate(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get_weak(self_obj, &GLK_VIEW_DELEGATE_KEY)
}

unsafe extern "C" fn glk_view_set_delegate(self_obj: Id, _cmd: Sel, delegate: Id) {
    assoc_set_weak(self_obj, &GLK_VIEW_DELEGATE_KEY, delegate);
}

unsafe extern "C" fn glk_view_color_format(self_obj: Id, _cmd: Sel) -> isize {
    glk_view_state(self_obj).map(|s| s.color_format).unwrap_or(0)
}
unsafe extern "C" fn glk_view_set_color_format(self_obj: Id, _cmd: Sel, fmt: isize) {
    if let Some(s) = glk_view_state(self_obj) {
        s.color_format = fmt;
    }
}

unsafe extern "C" fn glk_view_depth_format(self_obj: Id, _cmd: Sel) -> isize {
    glk_view_state(self_obj)
        .map(|s| s.depth_format)
        .unwrap_or(GLK_DEPTH_NONE)
}
unsafe extern "C" fn glk_view_set_depth_format(self_obj: Id, _cmd: Sel, fmt: isize) {
    if let Some(s) = glk_view_state(self_obj) {
        s.depth_format = fmt;
    }
}

unsafe extern "C" fn glk_view_stencil_format(self_obj: Id, _cmd: Sel) -> isize {
    glk_view_state(self_obj)
        .map(|s| s.stencil_format)
        .unwrap_or(GLK_STENCIL_NONE)
}
unsafe extern "C" fn glk_view_set_stencil_format(self_obj: Id, _cmd: Sel, fmt: isize) {
    if let Some(s) = glk_view_state(self_obj) {
        s.stencil_format = fmt;
    }
}

unsafe extern "C" fn glk_view_multisample(self_obj: Id, _cmd: Sel) -> isize {
    glk_view_state(self_obj).map(|s| s.multisample).unwrap_or(0)
}
unsafe extern "C" fn glk_view_set_multisample(self_obj: Id, _cmd: Sel, ms: isize) {
    if let Some(s) = glk_view_state(self_obj) {
        s.multisample = ms;
    }
}

unsafe extern "C" fn glk_view_drawable_width(self_obj: Id, _cmd: Sel) -> isize {
    glk_view_state(self_obj)
        .map(|s| s.allocated_pixels.width as isize)
        .unwrap_or(0)
}

unsafe extern "C" fn glk_view_drawable_height(self_obj: Id, _cmd: Sel) -> isize {
    glk_view_state(self_obj)
        .map(|s| s.allocated_pixels.height as isize)
        .unwrap_or(0)
}

unsafe extern "C" fn glk_view_enable_set_needs_display(self_obj: Id, _cmd: Sel) -> bool {
    glk_view_state(self_obj)
        .map(|s| s.enable_set_needs_display)
        .unwrap_or(true)
}
unsafe extern "C" fn glk_view_set_enable_set_needs_display(self_obj: Id, _cmd: Sel, en: bool) {
    if let Some(s) = glk_view_state(self_obj) {
        s.enable_set_needs_display = en;
    }
}

extern "C" fn glk_deferred_display(ctx: *mut c_void) {
    unsafe {
        let view = ctx as Id;
        if let Some(s) = glk_view_state(view) {
            s.display_scheduled = false;
            msg0::<()>(view, "display");
        }
        objc_release(view);
    }
}

unsafe extern "C" fn glk_view_layout_subviews(self_obj: Id, _cmd: Sel) {
    super0::<()>(self_obj, glk_view_class(), "layoutSubviews");
    if let Some(s) = glk_view_state(self_obj) {
        if s.enable_set_needs_display {
            msg0::<()>(self_obj, "setNeedsDisplay");
        }
    }
}

unsafe extern "C" fn glk_view_set_needs_display(self_obj: Id, _cmd: Sel) {
    super0::<()>(self_obj, glk_view_class(), "setNeedsDisplay");
    let Some(s) = glk_view_state(self_obj) else {
        return;
    };
    if !s.enable_set_needs_display || s.display_scheduled {
        return;
    }
    s.display_scheduled = true;
    objc_retain(self_obj);
    dispatch_async_f(
        &_dispatch_main_q as *const c_void,
        self_obj,
        glk_deferred_display,
    );
}

unsafe fn glk_make_current(s: &GlkViewState) -> bool {
    if s.context.is_null() {
        return false;
    }
    let eagl_cls = cls("EAGLContext") as Id;
    if eagl_cls.is_null() {
        return false;
    }
    let cur: Id = msg0(eagl_cls, "currentContext");
    if cur != s.context && !msg1::<Id, bool>(eagl_cls, "setCurrentContext:", s.context) {
        return false;
    }
    gl_funcs().is_some()
}

unsafe fn glk_wanted_pixels(self_obj: Id) -> CGSize {
    let layer: Id = msg0(self_obj, "layer");
    let layer_scale: f64 = if layer.is_null() {
        0.0
    } else {
        msg0(layer, "contentsScale")
    };
    let scale = if layer_scale > 0.0 {
        layer_scale
    } else {
        msg0(self_obj, "contentScaleFactor")
    };
    let bounds: CGRect = if layer.is_null() {
        msg0(self_obj, "bounds")
    } else {
        msg0(layer, "bounds")
    };
    CGSize {
        width: (bounds.size.width * scale).round(),
        height: (bounds.size.height * scale).round(),
    }
}

unsafe fn glk_ensure_drawable(self_obj: Id, s: &mut GlkViewState) -> bool {
    let wanted = glk_wanted_pixels(self_obj);
    if wanted.width < 1.0 || wanted.height < 1.0 {
        return false;
    }
    if s.framebuffer != 0 && s.allocated_pixels == wanted {
        return true;
    }
    let Some(gl) = gl_funcs() else {
        return false;
    };
    if s.framebuffer == 0 {
        (gl.gen_framebuffers)(1, &mut s.framebuffer);
    }
    if s.color == 0 {
        (gl.gen_renderbuffers)(1, &mut s.color);
    }
    (gl.bind_framebuffer)(GL_FRAMEBUFFER, s.framebuffer);
    (gl.bind_renderbuffer)(GL_RENDERBUFFER, s.color);

    let layer: Id = msg0(self_obj, "layer");
    if !msg2::<usize, Id, bool>(
        s.context,
        "renderbufferStorage:fromDrawable:",
        GL_RENDERBUFFER as usize,
        layer,
    ) {
        s.allocated_pixels = CGSize::default();
        return false;
    }
    (gl.framebuffer_renderbuffer)(
        GL_FRAMEBUFFER,
        GL_COLOR_ATTACHMENT0,
        GL_RENDERBUFFER,
        s.color,
    );

    let mut width: i32 = 0;
    let mut height: i32 = 0;
    (gl.get_renderbuffer_parameteriv)(GL_RENDERBUFFER, GL_RENDERBUFFER_WIDTH, &mut width);
    (gl.get_renderbuffer_parameteriv)(GL_RENDERBUFFER, GL_RENDERBUFFER_HEIGHT, &mut height);

    if s.depth_format != GLK_DEPTH_NONE || s.stencil_format != GLK_STENCIL_NONE {
        if s.depth == 0 {
            (gl.gen_renderbuffers)(1, &mut s.depth);
        }
        (gl.bind_renderbuffer)(GL_RENDERBUFFER, s.depth);
        let stencil = s.stencil_format != GLK_STENCIL_NONE;
        let format = if stencil {
            GL_DEPTH24_STENCIL8_OES
        } else if s.depth_format == GLK_DEPTH_16 {
            GL_DEPTH_COMPONENT16
        } else {
            GL_DEPTH_COMPONENT24_OES
        };
        (gl.renderbuffer_storage)(GL_RENDERBUFFER, format, width, height);
        (gl.framebuffer_renderbuffer)(
            GL_FRAMEBUFFER,
            GL_DEPTH_ATTACHMENT,
            GL_RENDERBUFFER,
            s.depth,
        );
        if stencil {
            (gl.framebuffer_renderbuffer)(
                GL_FRAMEBUFFER,
                GL_STENCIL_ATTACHMENT,
                GL_RENDERBUFFER,
                s.depth,
            );
        }
        (gl.bind_renderbuffer)(GL_RENDERBUFFER, s.color);
    }

    let status = (gl.check_framebuffer_status)(GL_FRAMEBUFFER);
    if status != GL_FRAMEBUFFER_COMPLETE {
        log_glkit(&format!(
            "framebuffer incomplete 0x{status:x} ({width}x{height})"
        ));
        return false;
    }
    s.allocated_pixels = CGSize {
        width: width as f64,
        height: height as f64,
    };
    true
}

unsafe extern "C" fn glk_view_bind_drawable(self_obj: Id, _cmd: Sel) {
    let Some(s) = glk_view_state(self_obj) else {
        return;
    };
    if !glk_make_current(s) || !glk_ensure_drawable(self_obj, s) {
        return;
    }
    if let Some(gl) = gl_funcs() {
        (gl.bind_framebuffer)(GL_FRAMEBUFFER, s.framebuffer);
        (gl.viewport)(
            0,
            0,
            s.allocated_pixels.width as i32,
            s.allocated_pixels.height as i32,
        );
    }
}

unsafe extern "C" fn glk_view_delete_drawable(self_obj: Id, _cmd: Sel) {
    let Some(s) = glk_view_state(self_obj) else {
        return;
    };
    if s.framebuffer == 0 && s.color == 0 && s.depth == 0 {
        return;
    }
    if glk_make_current(s) {
        if let Some(gl) = gl_funcs() {
            if s.framebuffer != 0 {
                (gl.delete_framebuffers)(1, &s.framebuffer);
            }
            let buffers = [s.color, s.depth];
            (gl.delete_renderbuffers)(2, buffers.as_ptr());
        }
    }
    s.framebuffer = 0;
    s.color = 0;
    s.depth = 0;
    s.allocated_pixels = CGSize::default();
}

unsafe extern "C" fn glk_view_display(self_obj: Id, _cmd: Sel) {
    autorelease_pool(|| {
        let Some(s) = glk_view_state(self_obj) else {
            return;
        };
        if !glk_make_current(s) {
            return;
        }
        msg0::<()>(self_obj, "bindDrawable");
        if s.framebuffer == 0 || s.allocated_pixels.width < 1.0 {
            return;
        }
        let bounds: CGRect = msg0(self_obj, "bounds");
        msg1::<CGRect, ()>(self_obj, "drawRect:", bounds);
        if let Some(gl) = gl_funcs() {
            (gl.bind_renderbuffer)(GL_RENDERBUFFER, s.color);
        }
        msg1::<usize, bool>(s.context, "presentRenderbuffer:", GL_RENDERBUFFER as usize);
    });
}

unsafe extern "C" fn glk_view_draw_rect(self_obj: Id, _cmd: Sel, rect: CGRect) {
    let delegate = assoc_get_weak(self_obj, &GLK_VIEW_DELEGATE_KEY);
    if !delegate.is_null() && responds_to(delegate, "glkView:drawInRect:") {
        objc_retain(delegate);
        msg2::<Id, CGRect, ()>(delegate, "glkView:drawInRect:", self_obj, rect);
        objc_release(delegate);
    }
}

unsafe extern "C" fn glk_view_snapshot(_self_obj: Id, _cmd: Sel) -> Id {
    NIL
}

// ============================================================================
// GLKViewController
// ============================================================================

struct GlkVcState {
    preferred_fps: isize,
    paused: bool,
    frames_displayed: isize,
    pause_on_resign: bool,
    resume_on_active: bool,
    first_resume: f64,
    last_resume: f64,
    last_update: f64,
    last_draw: f64,
    time_since_first_resume: f64,
    time_since_last_resume: f64,
    time_since_last_update: f64,
    time_since_last_draw: f64,
    link: Id,
}

static GLK_VC_STATE_KEY: u8 = 0;
static GLK_VC_DELEGATE_KEY: u8 = 0;

unsafe fn glk_vc_state(obj: Id) -> Option<&'static mut GlkVcState> {
    let val = objc_getAssociatedObject(obj, &GLK_VC_STATE_KEY as *const u8 as *const c_void);
    if val.is_null() {
        return None;
    }
    let ptr: *mut c_void = msg0(val, "pointerValue");
    if ptr.is_null() {
        None
    } else {
        Some(&mut *(ptr as *mut GlkVcState))
    }
}

unsafe fn glk_vc_setup(obj: Id) {
    let state = Box::new(GlkVcState {
        preferred_fps: 30,
        paused: false,
        frames_displayed: 0,
        pause_on_resign: true,
        resume_on_active: true,
        first_resume: 0.0,
        last_resume: 0.0,
        last_update: 0.0,
        last_draw: 0.0,
        time_since_first_resume: 0.0,
        time_since_last_resume: 0.0,
        time_since_last_update: 0.0,
        time_since_last_draw: 0.0,
        link: NIL,
    });
    let ptr = Box::into_raw(state) as *mut c_void;
    let val: Id = msg1::<*mut c_void, Id>(cls("NSValue"), "valueWithPointer:", ptr);
    objc_setAssociatedObject(
        obj,
        &GLK_VC_STATE_KEY as *const u8 as *const c_void,
        val,
        OBJC_ASSOCIATION_RETAIN,
    );

    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    let f: unsafe extern "C" fn(Id, Sel, Id, Sel, Id, Id) =
        std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
    f(
        nc,
        sel("addObserver:selector:name:object:"),
        obj,
        sel("pearWillResignActive:"),
        nsstring("UIApplicationWillResignActiveNotification"),
        NIL,
    );
    f(
        nc,
        sel("addObserver:selector:name:object:"),
        obj,
        sel("pearDidBecomeActive:"),
        nsstring("UIApplicationDidBecomeActiveNotification"),
        NIL,
    );
}

unsafe extern "C" fn glk_vc_init_nib(self_obj: Id, _cmd: Sel, nib: Id, bundle: Id) -> Id {
    let obj: Id = super2(self_obj, glk_vc_class(), "initWithNibName:bundle:", nib, bundle);
    if !obj.is_null() {
        glk_vc_setup(obj);
    }
    obj
}

unsafe extern "C" fn glk_vc_init_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = super1(self_obj, glk_vc_class(), "initWithCoder:", coder);
    if !obj.is_null() {
        glk_vc_setup(obj);
    }
    obj
}

unsafe extern "C" fn glk_vc_dealloc(self_obj: Id, _cmd: Sel) {
    glk_vc_stop(self_obj);
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg1::<Id, ()>(nc, "removeObserver:", self_obj);
    let val = objc_getAssociatedObject(
        self_obj,
        &GLK_VC_STATE_KEY as *const u8 as *const c_void,
    );
    if !val.is_null() {
        let ptr: *mut c_void = msg0(val, "pointerValue");
        if !ptr.is_null() {
            let _ = Box::from_raw(ptr as *mut GlkVcState);
        }
        objc_setAssociatedObject(
            self_obj,
            &GLK_VC_STATE_KEY as *const u8 as *const c_void,
            NIL,
            OBJC_ASSOCIATION_RETAIN,
        );
    }
    super0::<()>(self_obj, glk_vc_class(), "dealloc");
}

unsafe extern "C" fn glk_vc_load_view(self_obj: Id, _cmd: Sel) {
    let nn: Id = msg0(self_obj, "nibName");
    if !nn.is_null() && msg0::<usize>(nn, "length") > 0 {
        super0::<()>(self_obj, glk_vc_class(), "loadView");
        return;
    }
    let bounds: CGRect = msg0(msg0(cls("UIScreen"), "mainScreen"), "bounds");
    let view: Id = msg1(msg0(glk_view_class() as Id, "alloc"), "initWithFrame:", bounds);
    msg1::<Id, ()>(self_obj, "setView:", view);
    objc_release(view);
}

unsafe extern "C" fn glk_vc_view_did_load(self_obj: Id, _cmd: Sel) {
    super0::<()>(self_obj, glk_vc_class(), "viewDidLoad");
    let view: Id = msg0(self_obj, "view");
    if !view.is_null() && msg1::<Class, bool>(view, "isKindOfClass:", glk_view_class()) {
        // Display is driven by the controller's CADisplayLink, not setNeedsDisplay.
        msg1::<bool, ()>(view, "setEnableSetNeedsDisplay:", false);
        if msg0::<Id>(view, "delegate").is_null() && responds_to(self_obj, "glkView:drawInRect:") {
            msg1::<Id, ()>(view, "setDelegate:", self_obj);
        }
    }
}

unsafe extern "C" fn glk_vc_view_did_appear(self_obj: Id, _cmd: Sel, animated: bool) {
    super1::<bool, ()>(self_obj, glk_vc_class(), "viewDidAppear:", animated);
    if let Some(s) = glk_vc_state(self_obj) {
        if !s.paused {
            glk_vc_start(self_obj);
        }
    }
}

unsafe extern "C" fn glk_vc_view_did_disappear(self_obj: Id, _cmd: Sel, animated: bool) {
    super1::<bool, ()>(self_obj, glk_vc_class(), "viewDidDisappear:", animated);
    glk_vc_stop(self_obj);
}

unsafe extern "C" fn glk_vc_delegate(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get_weak(self_obj, &GLK_VC_DELEGATE_KEY)
}

unsafe extern "C" fn glk_vc_set_delegate(self_obj: Id, _cmd: Sel, delegate: Id) {
    assoc_set_weak(self_obj, &GLK_VC_DELEGATE_KEY, delegate);
}

unsafe extern "C" fn glk_vc_preferred_fps(self_obj: Id, _cmd: Sel) -> isize {
    glk_vc_state(self_obj).map(|s| s.preferred_fps).unwrap_or(30)
}

unsafe extern "C" fn glk_vc_frames_per_second(self_obj: Id, _cmd: Sel) -> isize {
    let raw_max: isize = msg0(msg0(cls("UIScreen"), "mainScreen"), "maximumFramesPerSecond");
    let max_fps = if raw_max > 0 { raw_max } else { 60 };
    let pref = glk_vc_state(self_obj).map(|s| s.preferred_fps).unwrap_or(30);
    let target = if pref > 0 { pref } else { max_fps };
    target.clamp(1, max_fps)
}

unsafe extern "C" fn glk_vc_set_preferred_fps(self_obj: Id, _cmd: Sel, rate: isize) {
    let Some(s) = glk_vc_state(self_obj) else {
        return;
    };
    s.preferred_fps = rate;
    if !s.link.is_null() {
        let fps = glk_vc_frames_per_second(self_obj, sel("framesPerSecond"));
        msg1::<isize, ()>(s.link, "setPreferredFramesPerSecond:", fps);
    }
}

unsafe extern "C" fn glk_vc_is_paused(self_obj: Id, _cmd: Sel) -> bool {
    glk_vc_state(self_obj).map(|s| s.paused).unwrap_or(false)
}

unsafe extern "C" fn glk_vc_set_paused(self_obj: Id, _cmd: Sel, paused: bool) {
    let Some(s) = glk_vc_state(self_obj) else {
        return;
    };
    if s.paused == paused {
        return;
    }
    s.paused = paused;
    if paused {
        glk_vc_stop(self_obj);
        return;
    }
    if msg0::<bool>(self_obj, "isViewLoaded") {
        let view: Id = msg0(self_obj, "view");
        if !view.is_null() && !msg0::<Id>(view, "window").is_null() {
            glk_vc_start(self_obj);
        }
    }
}

unsafe extern "C" fn glk_vc_frames_displayed(self_obj: Id, _cmd: Sel) -> isize {
    glk_vc_state(self_obj).map(|s| s.frames_displayed).unwrap_or(0)
}
unsafe extern "C" fn glk_vc_time_first_resume(self_obj: Id, _cmd: Sel) -> f64 {
    glk_vc_state(self_obj).map(|s| s.time_since_first_resume).unwrap_or(0.0)
}
unsafe extern "C" fn glk_vc_time_last_resume(self_obj: Id, _cmd: Sel) -> f64 {
    glk_vc_state(self_obj).map(|s| s.time_since_last_resume).unwrap_or(0.0)
}
unsafe extern "C" fn glk_vc_time_last_update(self_obj: Id, _cmd: Sel) -> f64 {
    glk_vc_state(self_obj).map(|s| s.time_since_last_update).unwrap_or(0.0)
}
unsafe extern "C" fn glk_vc_time_last_draw(self_obj: Id, _cmd: Sel) -> f64 {
    glk_vc_state(self_obj).map(|s| s.time_since_last_draw).unwrap_or(0.0)
}

unsafe extern "C" fn glk_vc_pause_on_resign(self_obj: Id, _cmd: Sel) -> bool {
    glk_vc_state(self_obj).map(|s| s.pause_on_resign).unwrap_or(true)
}
unsafe extern "C" fn glk_vc_set_pause_on_resign(self_obj: Id, _cmd: Sel, v: bool) {
    if let Some(s) = glk_vc_state(self_obj) {
        s.pause_on_resign = v;
    }
}

unsafe extern "C" fn glk_vc_resume_on_active(self_obj: Id, _cmd: Sel) -> bool {
    glk_vc_state(self_obj).map(|s| s.resume_on_active).unwrap_or(true)
}
unsafe extern "C" fn glk_vc_set_resume_on_active(self_obj: Id, _cmd: Sel, v: bool) {
    if let Some(s) = glk_vc_state(self_obj) {
        s.resume_on_active = v;
    }
}

unsafe extern "C" fn glk_vc_will_resign_active(self_obj: Id, _cmd: Sel, _notif: Id) {
    if glk_vc_pause_on_resign(self_obj, sel("pauseOnWillResignActive")) {
        msg1::<bool, ()>(self_obj, "setPaused:", true);
    }
}

unsafe extern "C" fn glk_vc_did_become_active(self_obj: Id, _cmd: Sel, _notif: Id) {
    if glk_vc_resume_on_active(self_obj, sel("resumeOnDidBecomeActive")) {
        msg1::<bool, ()>(self_obj, "setPaused:", false);
    }
}

unsafe fn glk_vc_start(self_obj: Id) {
    let Some(s) = glk_vc_state(self_obj) else {
        return;
    };
    if !s.link.is_null() {
        return;
    }
    let now = CACurrentMediaTime();
    if s.first_resume == 0.0 {
        s.first_resume = now;
    }
    s.last_resume = now;
    let scr: Id = msg0(cls("UIScreen"), "mainScreen");
    let link: Id = msg2::<Id, Sel, Id>(scr, "displayLinkWithTarget:selector:", self_obj, sel("pearTick:"));
    if !link.is_null() {
        let fps = glk_vc_frames_per_second(self_obj, sel("framesPerSecond"));
        msg1::<isize, ()>(link, "setPreferredFramesPerSecond:", fps);
        let run_loop: Id = msg0(cls("NSRunLoop"), "mainRunLoop");
        msg2::<Id, Id, ()>(link, "addToRunLoop:forMode:", run_loop, NSRunLoopCommonModes);
        set_retained(&mut s.link, link);
    }
}

unsafe fn glk_vc_stop(self_obj: Id) {
    if let Some(s) = glk_vc_state(self_obj) {
        if !s.link.is_null() {
            msg0::<()>(s.link, "invalidate");
            set_retained(&mut s.link, NIL);
        }
    }
}

unsafe extern "C" fn glk_vc_tick(self_obj: Id, _cmd: Sel, _link: Id) {
    let Some(s) = glk_vc_state(self_obj) else {
        return;
    };
    let now = CACurrentMediaTime();
    s.time_since_last_update = if s.last_update > 0.0 { now - s.last_update } else { 0.0 };
    s.last_update = now;
    s.time_since_first_resume = now - s.first_resume;
    s.time_since_last_resume = now - s.last_resume;

    let delegate = assoc_get_weak(self_obj, &GLK_VC_DELEGATE_KEY);
    if !delegate.is_null() && responds_to(delegate, "glkViewControllerUpdate:") {
        objc_retain(delegate);
        msg1::<Id, ()>(delegate, "glkViewControllerUpdate:", self_obj);
        objc_release(delegate);
    } else if responds_to(self_obj, "update") {
        msg0::<()>(self_obj, "update");
    }

    let view: Id = msg0(self_obj, "view");
    if !view.is_null() && msg1::<Class, bool>(view, "isKindOfClass:", glk_view_class()) {
        msg0::<()>(view, "display");
    }

    let draw_now = CACurrentMediaTime();
    s.time_since_last_draw = if s.last_draw > 0.0 { draw_now - s.last_draw } else { 0.0 };
    s.last_draw = draw_now;
    s.frames_displayed += 1;
}

// ============================================================================
// Registration & GLKit.framework ABI
// ============================================================================

#[no_mangle]
pub extern "C" fn pear_glkit_register_classes() {
    unsafe {
        let gv = glk_view_class();
        if !gv.is_null() {
            let meta = object_getClass(gv as Id);
            add_method(meta, "layerClass", glk_view_layer_class as Imp, "#@:");

            add_method(gv, "initWithFrame:", glk_view_init_with_frame as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}");
            add_method(gv, "initWithCoder:", glk_view_init_with_coder as Imp, "@@:@");
            add_method(gv, "initWithFrame:context:", glk_view_init_with_frame_context as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}@");
            add_method(gv, "dealloc", glk_view_dealloc as Imp, "v@:");

            add_method(gv, "context", glk_view_context as Imp, "@@:");
            add_method(gv, "setContext:", glk_view_set_context as Imp, "v@:@");
            add_method(gv, "delegate", glk_view_delegate as Imp, "@@:");
            add_method(gv, "setDelegate:", glk_view_set_delegate as Imp, "v@:@");

            add_method(gv, "drawableColorFormat", glk_view_color_format as Imp, "q@:");
            add_method(gv, "setDrawableColorFormat:", glk_view_set_color_format as Imp, "v@:q");
            add_method(gv, "drawableDepthFormat", glk_view_depth_format as Imp, "q@:");
            add_method(gv, "setDrawableDepthFormat:", glk_view_set_depth_format as Imp, "v@:q");
            add_method(gv, "drawableStencilFormat", glk_view_stencil_format as Imp, "q@:");
            add_method(gv, "setDrawableStencilFormat:", glk_view_set_stencil_format as Imp, "v@:q");
            add_method(gv, "drawableMultisample", glk_view_multisample as Imp, "q@:");
            add_method(gv, "setDrawableMultisample:", glk_view_set_multisample as Imp, "v@:q");

            add_method(gv, "drawableWidth", glk_view_drawable_width as Imp, "q@:");
            add_method(gv, "drawableHeight", glk_view_drawable_height as Imp, "q@:");
            add_method(gv, "enableSetNeedsDisplay", glk_view_enable_set_needs_display as Imp, "B@:");
            add_method(gv, "setEnableSetNeedsDisplay:", glk_view_set_enable_set_needs_display as Imp, "v@:B");

            add_method(gv, "layoutSubviews", glk_view_layout_subviews as Imp, "v@:");
            add_method(gv, "setNeedsDisplay", glk_view_set_needs_display as Imp, "v@:");
            add_method(gv, "bindDrawable", glk_view_bind_drawable as Imp, "v@:");
            add_method(gv, "deleteDrawable", glk_view_delete_drawable as Imp, "v@:");
            add_method(gv, "display", glk_view_display as Imp, "v@:");
            add_method(gv, "drawRect:", glk_view_draw_rect as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
            add_method(gv, "snapshot", glk_view_snapshot as Imp, "@@:");
        }

        let gvc = glk_vc_class();
        if !gvc.is_null() {
            add_method(gvc, "initWithNibName:bundle:", glk_vc_init_nib as Imp, "@@:@@");
            add_method(gvc, "initWithCoder:", glk_vc_init_coder as Imp, "@@:@");
            add_method(gvc, "dealloc", glk_vc_dealloc as Imp, "v@:");

            add_method(gvc, "loadView", glk_vc_load_view as Imp, "v@:");
            add_method(gvc, "viewDidLoad", glk_vc_view_did_load as Imp, "v@:");
            add_method(gvc, "viewDidAppear:", glk_vc_view_did_appear as Imp, "v@:B");
            add_method(gvc, "viewDidDisappear:", glk_vc_view_did_disappear as Imp, "v@:B");

            add_method(gvc, "delegate", glk_vc_delegate as Imp, "@@:");
            add_method(gvc, "setDelegate:", glk_vc_set_delegate as Imp, "v@:@");
            add_method(gvc, "preferredFramesPerSecond", glk_vc_preferred_fps as Imp, "q@:");
            add_method(gvc, "setPreferredFramesPerSecond:", glk_vc_set_preferred_fps as Imp, "v@:q");
            add_method(gvc, "framesPerSecond", glk_vc_frames_per_second as Imp, "q@:");
            add_method(gvc, "isPaused", glk_vc_is_paused as Imp, "B@:");
            add_method(gvc, "setPaused:", glk_vc_set_paused as Imp, "v@:B");

            add_method(gvc, "framesDisplayed", glk_vc_frames_displayed as Imp, "q@:");
            add_method(gvc, "timeSinceFirstResume", glk_vc_time_first_resume as Imp, "d@:");
            add_method(gvc, "timeSinceLastResume", glk_vc_time_last_resume as Imp, "d@:");
            add_method(gvc, "timeSinceLastUpdate", glk_vc_time_last_update as Imp, "d@:");
            add_method(gvc, "timeSinceLastDraw", glk_vc_time_last_draw as Imp, "d@:");

            add_method(gvc, "pauseOnWillResignActive", glk_vc_pause_on_resign as Imp, "B@:");
            add_method(gvc, "setPauseOnWillResignActive:", glk_vc_set_pause_on_resign as Imp, "v@:B");
            add_method(gvc, "resumeOnDidBecomeActive", glk_vc_resume_on_active as Imp, "B@:");
            add_method(gvc, "setResumeOnDidBecomeActive:", glk_vc_set_resume_on_active as Imp, "v@:B");

            add_method(gvc, "pearWillResignActive:", glk_vc_will_resign_active as Imp, "v@:@");
            add_method(gvc, "pearDidBecomeActive:", glk_vc_did_become_active as Imp, "v@:@");
            add_method(gvc, "pearTick:", glk_vc_tick as Imp, "v@:@");
        }
    }
}

extern "C" fn image_load() {
    pear_glkit_register_classes();
    unsafe { PearMissingInstallForImage(image_load as *const c_void) };
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_GLKIT_INIT: extern "C" fn() = image_load;

mod abi {
    extern "C" {
        #[link_name = "OBJC_CLASS_$_UIView"]
        static UIView_CLASS: std::ffi::c_void;
        #[link_name = "OBJC_METACLASS_$_UIView"]
        static UIView_META: std::ffi::c_void;
        #[link_name = "OBJC_CLASS_$_UIViewController"]
        static UIViewController_CLASS: std::ffi::c_void;
        #[link_name = "OBJC_METACLASS_$_UIViewController"]
        static UIViewController_META: std::ffi::c_void;
    }

    pear_objc::image_info!();
    pear_objc::empty_class!(GLKView : &raw const super::UIView_CLASS, &raw const super::UIView_META);
    pear_objc::empty_class!(GLKViewController : &raw const super::UIViewController_CLASS, &raw const super::UIViewController_META);
}
