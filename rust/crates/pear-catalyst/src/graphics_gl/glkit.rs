//! ANGLE-backed GLKView / GLKViewController.
//!
//! GLKit's own GLKView binds renderbuffers in the *system* OpenGL ES; handed
//! an ANGLE context, ANGLE never saw a bound renderbuffer and every frame was
//! refused. These classes implement the public GLKView / GLKViewController
//! behaviour on ANGLE; import routing binds the app's class references
//! (subclasses' superclass included) to them. No GLKit code is used.

use super::backend::trace;
use super::context::NIL;
use super::eagl;
use super::routing::game_symbol;
use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, objc_release, objc_retain, CGRect, CGSize, Id, Sel};
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::sync::OnceLock;

type P = *mut c_void;

struct Gl {
    gen_framebuffers: unsafe extern "C" fn(i32, *mut u32),
    gen_renderbuffers: unsafe extern "C" fn(i32, *mut u32),
    delete_framebuffers: unsafe extern "C" fn(i32, *const u32),
    delete_renderbuffers: unsafe extern "C" fn(i32, *const u32),
    bind_framebuffer: unsafe extern "C" fn(u32, u32),
    bind_renderbuffer: unsafe extern "C" fn(u32, u32),
    renderbuffer_storage: unsafe extern "C" fn(u32, u32, i32, i32),
    framebuffer_renderbuffer: unsafe extern "C" fn(u32, u32, u32, u32),
    get_renderbuffer_parameter: unsafe extern "C" fn(u32, u32, *mut i32),
    check_framebuffer_status: unsafe extern "C" fn(u32) -> u32,
    viewport: unsafe extern "C" fn(i32, i32, i32, i32),
}
unsafe impl Send for Gl {}
unsafe impl Sync for Gl {}

/// Resolved through the routing table: ANGLE only, started on demand.
fn gl() -> Option<&'static Gl> {
    static GL: OnceLock<Option<Gl>> = OnceLock::new();
    GL.get_or_init(|| unsafe {
        macro_rules! s {
            ($n:literal) => {{
                let p = game_symbol($n.as_ptr());
                if p.is_null() { None } else { Some(std::mem::transmute(p)) }
            }};
        }
        Some(Gl {
            gen_framebuffers: s!(c"glGenFramebuffers")?,
            gen_renderbuffers: s!(c"glGenRenderbuffers")?,
            delete_framebuffers: s!(c"glDeleteFramebuffers")?,
            delete_renderbuffers: s!(c"glDeleteRenderbuffers")?,
            bind_framebuffer: s!(c"glBindFramebuffer")?,
            bind_renderbuffer: s!(c"glBindRenderbuffer")?,
            renderbuffer_storage: s!(c"glRenderbufferStorage")?,
            framebuffer_renderbuffer: s!(c"glFramebufferRenderbuffer")?,
            get_renderbuffer_parameter: s!(c"glGetRenderbufferParameteriv")?,
            check_framebuffer_status: s!(c"glCheckFramebufferStatus")?,
            viewport: s!(c"glViewport")?,
        })
    })
    .as_ref()
}

/// A zeroing weak reference.
struct Weak(Box<UnsafeCell<Id>>);
impl Weak {
    fn new() -> Weak {
        Weak(Box::new(UnsafeCell::new(NIL)))
    }
    unsafe fn set(&self, obj: Id) {
        pear_objc::objc_storeWeak(self.0.get(), obj);
    }
    unsafe fn get(&self) -> Id {
        let o = pear_objc::objc_loadWeakRetained(self.0.get());
        if o.is_null() { o } else { pear_objc::autoreleased(o) }
    }
}
impl Drop for Weak {
    fn drop(&mut self) {
        unsafe { pear_objc::objc_destroyWeak(self.0.get()) };
    }
}

// Public GLKit enum values (GLKView.h).
const DEPTH_NONE: isize = 0;
const DEPTH_16: isize = 1;
const STENCIL_NONE: isize = 0;

const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_RENDERBUFFER: u32 = 0x8D41;

// ---------------------------------------------------------------------------
// PearGLKView

struct View {
    context: Id, // retained PearEAGLContext
    delegate: Weak,
    color_format: isize,
    depth_format: isize,
    stencil_format: isize,
    multisample: isize,
    enable_set_needs_display: bool,
    framebuffer: u32,
    color: u32,
    depth: u32,
    allocated: CGSize,
    display_scheduled: bool,
}

unsafe fn view(this: Id) -> &'static mut View {
    let slot = glk_view::slot(this);
    if (*slot).is_null() {
        *slot = Box::into_raw(Box::new(View {
            context: NIL,
            delegate: Weak::new(),
            color_format: 0,
            depth_format: 0,
            stencil_format: 0,
            multisample: 0,
            enable_set_needs_display: false,
            framebuffer: 0,
            color: 0,
            depth: 0,
            allocated: CGSize::default(),
            display_scheduled: false,
        })) as P;
    }
    &mut *(*slot as *mut View)
}

/// The drawable host only has to be a CALayer; CAEAGLLayer carries the
/// drawable-properties API engines may set, when the class still exists.
unsafe extern "C" fn layer_class(_: Id, _: Sel) -> Id {
    let c = rt::class_named("CAEAGLLayer");
    if c.is_null() { cls("CALayer") } else { c }
}

unsafe fn setup(this: Id) {
    view(this).enable_set_needs_display = true;
    msg1::<bool, ()>(this, "setOpaque:", true);
    let scale: f64 = msg0(msg0::<Id>(cls("UIScreen"), "mainScreen"), "scale");
    msg1::<f64, ()>(this, "setContentScaleFactor:", scale);
}

unsafe extern "C" fn view_init_frame(this: Id, _: Sel, frame: CGRect) -> Id {
    let this: Id = pear_objc::msg_super1(this, "PearGLKView", "initWithFrame:", frame);
    if !this.is_null() {
        setup(this);
    }
    this
}

unsafe extern "C" fn view_init_coder(this: Id, _: Sel, coder: Id) -> Id {
    let this: Id = pear_objc::msg_super1(this, "PearGLKView", "initWithCoder:", coder);
    if !this.is_null() {
        setup(this);
    }
    this
}

unsafe extern "C" fn view_init_frame_context(this: Id, _: Sel, frame: CGRect, context: Id) -> Id {
    let this: Id = msg1(this, "initWithFrame:", frame);
    if !this.is_null() {
        let v = view(this);
        v.context = if context.is_null() { NIL } else { objc_retain(context) };
    }
    this
}

unsafe extern "C" fn view_context(this: Id, _: Sel) -> Id {
    view(this).context
}

unsafe extern "C" fn view_set_context(this: Id, _: Sel, context: Id) {
    let v = view(this);
    if context == v.context {
        return;
    }
    msg0::<()>(this, "deleteDrawable");
    let v = view(this);
    let old = v.context;
    v.context = if context.is_null() { NIL } else { objc_retain(context) };
    if !old.is_null() {
        objc_release(old);
    }
}

unsafe extern "C" fn view_delegate(this: Id, _: Sel) -> Id {
    view(this).delegate.get()
}
unsafe extern "C" fn view_set_delegate(this: Id, _: Sel, delegate: Id) {
    view(this).delegate.set(delegate);
}

macro_rules! int_property {
    ($get:ident, $set:ident, $field:ident) => {
        unsafe extern "C" fn $get(this: Id, _: Sel) -> isize {
            view(this).$field
        }
        unsafe extern "C" fn $set(this: Id, _: Sel, value: isize) {
            view(this).$field = value;
        }
    };
}
int_property!(color_format, set_color_format, color_format);
int_property!(depth_format, set_depth_format, depth_format);
int_property!(stencil_format, set_stencil_format, stencil_format);
int_property!(multisample, set_multisample, multisample);

unsafe extern "C" fn drawable_width(this: Id, _: Sel) -> isize {
    view(this).allocated.width as isize
}
unsafe extern "C" fn drawable_height(this: Id, _: Sel) -> isize {
    view(this).allocated.height as isize
}
unsafe extern "C" fn enable_snd(this: Id, _: Sel) -> bool {
    view(this).enable_set_needs_display
}
unsafe extern "C" fn set_enable_snd(this: Id, _: Sel, value: bool) {
    view(this).enable_set_needs_display = value;
}

unsafe extern "C" fn view_layout(this: Id, _: Sel) {
    pear_objc::msg_super0::<()>(this, "PearGLKView", "layoutSubviews");
    // Reallocate on the next display, in the display's own GL context.
    if view(this).enable_set_needs_display {
        msg0::<()>(this, "setNeedsDisplay");
    }
}

unsafe extern "C" fn view_set_needs_display(this: Id, _: Sel) {
    pear_objc::msg_super0::<()>(this, "PearGLKView", "setNeedsDisplay");
    let v = view(this);
    if !v.enable_set_needs_display || v.display_scheduled {
        return;
    }
    v.display_scheduled = true;
    let me = objc_retain(this) as usize;
    rt::async_main(move || unsafe {
        let this = me as Id;
        view(this).display_scheduled = false;
        msg0::<()>(this, "display");
        objc_release(this);
    });
}

unsafe fn make_current(this: Id) -> bool {
    let context = view(this).context;
    if context.is_null() {
        return false;
    }
    if eagl::current() != context && !eagl::set_current(context) {
        return false;
    }
    gl().is_some()
}

unsafe fn wanted_pixels(this: Id) -> CGSize {
    let layer: Id = msg0(this, "layer");
    let layer_scale: f64 = msg0(layer, "contentsScale");
    let scale = if layer_scale > 0.0 { layer_scale } else { msg0(this, "contentScaleFactor") };
    let bounds: CGRect = msg0(layer, "bounds");
    CGSize { width: (bounds.size.width * scale).round(), height: (bounds.size.height * scale).round() }
}

unsafe fn ensure_drawable(this: Id) -> bool {
    let wanted = wanted_pixels(this);
    if wanted.width < 1.0 || wanted.height < 1.0 {
        return false;
    }
    let v = view(this);
    if v.framebuffer != 0 && wanted == v.allocated {
        return true;
    }
    let Some(gl) = gl() else { return false };
    if v.framebuffer == 0 {
        (gl.gen_framebuffers)(1, &mut v.framebuffer);
    }
    if v.color == 0 {
        (gl.gen_renderbuffers)(1, &mut v.color);
    }
    (gl.bind_framebuffer)(GL_FRAMEBUFFER, v.framebuffer);
    (gl.bind_renderbuffer)(GL_RENDERBUFFER, v.color);
    let layer: Id = msg0(this, "layer");
    if !msg2::<usize, Id, bool>(v.context, "renderbufferStorage:fromDrawable:", GL_RENDERBUFFER as usize, layer) {
        v.allocated = CGSize::default();
        return false;
    }
    (gl.framebuffer_renderbuffer)(GL_FRAMEBUFFER, 0x8CE0, GL_RENDERBUFFER, v.color);
    let (mut w, mut h) = (0, 0);
    (gl.get_renderbuffer_parameter)(GL_RENDERBUFFER, 0x8D42, &mut w);
    (gl.get_renderbuffer_parameter)(GL_RENDERBUFFER, 0x8D43, &mut h);
    if v.depth_format != DEPTH_NONE || v.stencil_format != STENCIL_NONE {
        if v.depth == 0 {
            (gl.gen_renderbuffers)(1, &mut v.depth);
        }
        (gl.bind_renderbuffer)(GL_RENDERBUFFER, v.depth);
        let stencil = v.stencil_format != STENCIL_NONE;
        let format = if stencil { 0x88F0 } else if v.depth_format == DEPTH_16 { 0x81A5 } else { 0x81A6 };
        (gl.renderbuffer_storage)(GL_RENDERBUFFER, format, w, h);
        (gl.framebuffer_renderbuffer)(GL_FRAMEBUFFER, 0x8D00, GL_RENDERBUFFER, v.depth);
        if stencil {
            (gl.framebuffer_renderbuffer)(GL_FRAMEBUFFER, 0x8D20, GL_RENDERBUFFER, v.depth);
        }
        (gl.bind_renderbuffer)(GL_RENDERBUFFER, v.color);
    }
    let status = (gl.check_framebuffer_status)(GL_FRAMEBUFFER);
    if status != 0x8CD5 {
        trace(&format!("[PearRuntime][glkit] framebuffer incomplete 0x{status:x} ({w}x{h})\n"));
        return false;
    }
    // Allocation follows the layer's bounds; the request is satisfied even
    // when the engine adapts its own pixel size.
    v.allocated = CGSize { width: w as f64, height: h as f64 };
    true
}

unsafe extern "C" fn view_bind_drawable(this: Id, _: Sel) {
    if !make_current(this) || !ensure_drawable(this) {
        return;
    }
    let (Some(gl), v) = (gl(), view(this)) else { return };
    (gl.bind_framebuffer)(GL_FRAMEBUFFER, v.framebuffer);
    (gl.viewport)(0, 0, v.allocated.width as i32, v.allocated.height as i32);
}

unsafe extern "C" fn view_delete_drawable(this: Id, _: Sel) {
    let v = view(this);
    if v.framebuffer == 0 && v.color == 0 && v.depth == 0 {
        return;
    }
    if make_current(this) {
        if let Some(gl) = gl() {
            let v = view(this);
            if v.framebuffer != 0 {
                (gl.delete_framebuffers)(1, &v.framebuffer);
            }
            let buffers = [v.color, v.depth];
            (gl.delete_renderbuffers)(2, buffers.as_ptr());
        }
    }
    let v = view(this);
    v.framebuffer = 0;
    v.color = 0;
    v.depth = 0;
    v.allocated = CGSize::default();
}

unsafe extern "C" fn view_display(this: Id, _: Sel) {
    if !make_current(this) {
        return;
    }
    msg0::<()>(this, "bindDrawable");
    let v = view(this);
    if v.framebuffer == 0 || v.allocated.width < 1.0 {
        return;
    }
    let bounds: CGRect = msg0(this, "bounds");
    msg1::<CGRect, ()>(this, "drawRect:", bounds);
    let v = view(this);
    if let Some(gl) = gl() {
        (gl.bind_renderbuffer)(GL_RENDERBUFFER, v.color);
    }
    msg1::<usize, bool>(v.context, "presentRenderbuffer:", GL_RENDERBUFFER as usize);
}

unsafe extern "C" fn view_draw_rect(this: Id, _: Sel, rect: CGRect) {
    let delegate = view(this).delegate.get();
    if !delegate.is_null() && pear_objc::responds_to(delegate, "glkView:drawInRect:") {
        msg2::<Id, CGRect, ()>(delegate, "glkView:drawInRect:", this, rect);
    }
}

unsafe extern "C" fn view_snapshot(_: Id, _: Sel) -> Id {
    NIL
}

unsafe extern "C" fn view_dealloc(this: Id, _: Sel) {
    msg0::<()>(this, "deleteDrawable");
    let slot = glk_view::slot(this);
    if !(*slot).is_null() {
        let v = Box::from_raw(*slot as *mut View);
        *slot = std::ptr::null_mut();
        if !v.context.is_null() {
            objc_release(v.context);
        }
    }
    pear_objc::super_dealloc(this, "PearGLKView");
}

crate::objc_class! {
    mod glk_view = "PearGLKView" : (UIVIEW_CLASS, UIVIEW_META),
    state: true,
    methods {
        "initWithFrame:" "@48@0:8{CGRect={CGPoint=dd}{CGSize=dd}}16" => view_init_frame,
        "initWithCoder:" "@24@0:8@16" => view_init_coder,
        "initWithFrame:context:" "@56@0:8{CGRect={CGPoint=dd}{CGSize=dd}}16@48" => view_init_frame_context,
        "context" "@16@0:8" => view_context,
        "setContext:" "v24@0:8@16" => view_set_context,
        "delegate" "@16@0:8" => view_delegate,
        "setDelegate:" "v24@0:8@16" => view_set_delegate,
        "drawableColorFormat" "q16@0:8" => color_format,
        "setDrawableColorFormat:" "v24@0:8q16" => set_color_format,
        "drawableDepthFormat" "q16@0:8" => depth_format,
        "setDrawableDepthFormat:" "v24@0:8q16" => set_depth_format,
        "drawableStencilFormat" "q16@0:8" => stencil_format,
        "setDrawableStencilFormat:" "v24@0:8q16" => set_stencil_format,
        "drawableMultisample" "q16@0:8" => multisample,
        "setDrawableMultisample:" "v24@0:8q16" => set_multisample,
        "drawableWidth" "q16@0:8" => drawable_width,
        "drawableHeight" "q16@0:8" => drawable_height,
        "enableSetNeedsDisplay" "B16@0:8" => enable_snd,
        "setEnableSetNeedsDisplay:" "v20@0:8B16" => set_enable_snd,
        "layoutSubviews" "v16@0:8" => view_layout,
        "setNeedsDisplay" "v16@0:8" => view_set_needs_display,
        "bindDrawable" "v16@0:8" => view_bind_drawable,
        "deleteDrawable" "v16@0:8" => view_delete_drawable,
        "display" "v16@0:8" => view_display,
        "drawRect:" "v48@0:8{CGRect={CGPoint=dd}{CGSize=dd}}16" => view_draw_rect,
        "snapshot" "@16@0:8" => view_snapshot,
        "dealloc" "v16@0:8" => view_dealloc,
    }
    class_methods {
        "layerClass" "#16@0:8" => layer_class,
    }
}

// ---------------------------------------------------------------------------
// PearGLKViewController

struct Controller {
    delegate: Weak,
    preferred_fps: isize,
    paused: bool,
    frames_displayed: isize,
    since_first_resume: f64,
    since_last_resume: f64,
    since_last_update: f64,
    since_last_draw: f64,
    pause_on_resign: bool,
    resume_on_active: bool,
    link: Id, // retained CADisplayLink
    first_resume: f64,
    last_resume: f64,
    last_update: f64,
    last_draw: f64,
    observers: Vec<usize>, // retained tokens
}

unsafe fn controller(this: Id) -> &'static mut Controller {
    let slot = glk_view_controller::slot(this);
    if (*slot).is_null() {
        *slot = Box::into_raw(Box::new(Controller {
            delegate: Weak::new(),
            preferred_fps: 0,
            paused: false,
            frames_displayed: 0,
            since_first_resume: 0.0,
            since_last_resume: 0.0,
            since_last_update: 0.0,
            since_last_draw: 0.0,
            pause_on_resign: false,
            resume_on_active: false,
            link: NIL,
            first_resume: 0.0,
            last_resume: 0.0,
            last_update: 0.0,
            last_draw: 0.0,
            observers: Vec::new(),
        })) as P;
    }
    &mut *(*slot as *mut Controller)
}

unsafe fn controller_setup(this: Id) {
    let c = controller(this);
    c.preferred_fps = 30;
    c.pause_on_resign = true;
    c.resume_on_active = true;
    for (name, pause) in [("UIApplicationWillResignActiveNotification", true), ("UIApplicationDidBecomeActiveNotification", false)] {
        let weak = std::rc::Rc::new(Weak::new());
        weak.set(this);
        let token = rt::observe_main(name, move |_| unsafe {
            let me = weak.get();
            if me.is_null() {
                return;
            }
            let c = controller(me);
            if pause && c.pause_on_resign {
                msg1::<bool, ()>(me, "setPaused:", true);
            } else if !pause && c.resume_on_active {
                msg1::<bool, ()>(me, "setPaused:", false);
            }
        });
        controller(this).observers.push(objc_retain(token) as usize);
    }
}

unsafe extern "C" fn vc_init_nib(this: Id, _: Sel, nib: Id, bundle: Id) -> Id {
    let this: Id = pear_objc::msg_super2(this, "PearGLKViewController", "initWithNibName:bundle:", nib, bundle);
    if !this.is_null() {
        controller_setup(this);
    }
    this
}

unsafe extern "C" fn vc_init_coder(this: Id, _: Sel, coder: Id) -> Id {
    let this: Id = pear_objc::msg_super1(this, "PearGLKViewController", "initWithCoder:", coder);
    if !this.is_null() {
        controller_setup(this);
    }
    this
}

unsafe extern "C" fn vc_load_view(this: Id, _: Sel) {
    if !msg0::<Id>(this, "nibName").is_null() {
        pear_objc::msg_super0::<()>(this, "PearGLKViewController", "loadView");
        return;
    }
    let bounds: CGRect = msg0(msg0::<Id>(cls("UIScreen"), "mainScreen"), "bounds");
    let v: Id = msg1(msg0::<Id>(glk_view::class(), "alloc"), "initWithFrame:", bounds);
    msg1::<Id, ()>(this, "setView:", v);
    objc_release(v);
}

unsafe extern "C" fn vc_view_did_load(this: Id, _: Sel) {
    pear_objc::msg_super0::<()>(this, "PearGLKViewController", "viewDidLoad");
    let v: Id = msg0(this, "view");
    if msg1::<Id, bool>(v, "isKindOfClass:", glk_view::class()) {
        // Display is driven by the controller, not by setNeedsDisplay.
        msg1::<bool, ()>(v, "setEnableSetNeedsDisplay:", false);
        if msg0::<Id>(v, "delegate").is_null() && pear_objc::responds_to(this, "glkView:drawInRect:") {
            msg1::<Id, ()>(v, "setDelegate:", this);
        }
    }
}

unsafe extern "C" fn vc_did_appear(this: Id, _: Sel, animated: bool) {
    pear_objc::msg_super1::<bool, ()>(this, "PearGLKViewController", "viewDidAppear:", animated);
    if !controller(this).paused {
        start(this);
    }
}

unsafe extern "C" fn vc_did_disappear(this: Id, _: Sel, animated: bool) {
    pear_objc::msg_super1::<bool, ()>(this, "PearGLKViewController", "viewDidDisappear:", animated);
    stop(this);
}

unsafe fn fps(this: Id) -> isize {
    let max: isize = msg0(msg0::<Id>(cls("UIScreen"), "mainScreen"), "maximumFramesPerSecond");
    let maximum = if max != 0 { max } else { 60 };
    let preferred = controller(this).preferred_fps;
    (if preferred > 0 { preferred } else { maximum }).min(maximum).max(1)
}

unsafe extern "C" fn vc_fps(this: Id, _: Sel) -> isize {
    fps(this)
}

unsafe extern "C" fn vc_preferred_fps(this: Id, _: Sel) -> isize {
    controller(this).preferred_fps
}

unsafe extern "C" fn vc_set_preferred_fps(this: Id, _: Sel, rate: isize) {
    controller(this).preferred_fps = rate;
    let link = controller(this).link;
    if !link.is_null() {
        msg1::<isize, ()>(link, "setPreferredFramesPerSecond:", fps(this));
    }
}

unsafe extern "C" fn vc_paused(this: Id, _: Sel) -> bool {
    controller(this).paused
}

unsafe extern "C" fn vc_set_paused(this: Id, _: Sel, paused: bool) {
    let c = controller(this);
    if c.paused == paused {
        return;
    }
    c.paused = paused;
    if paused {
        stop(this);
        return;
    }
    if msg0::<bool>(this, "isViewLoaded") && !msg0::<Id>(msg0::<Id>(this, "view"), "window").is_null() {
        start(this);
    }
}

extern "C" {
    fn CACurrentMediaTime() -> f64;
}

unsafe fn start(this: Id) {
    let c = controller(this);
    if !c.link.is_null() {
        return;
    }
    let now = CACurrentMediaTime();
    if c.first_resume == 0.0 {
        c.first_resume = now;
    }
    c.last_resume = now;
    let link: Id = msg2(cls("CADisplayLink"), "displayLinkWithTarget:selector:", this, pear_objc::sel("pearTick:"));
    c.link = objc_retain(link);
    msg1::<isize, ()>(link, "setPreferredFramesPerSecond:", fps(this));
    let mode: Id = pear_objc::nsstring("kCFRunLoopCommonModes");
    msg2::<Id, Id, ()>(link, "addToRunLoop:forMode:", msg0::<Id>(cls("NSRunLoop"), "mainRunLoop"), mode);
}

unsafe fn stop(this: Id) {
    let c = controller(this);
    if !c.link.is_null() {
        msg0::<()>(c.link, "invalidate");
        objc_release(c.link);
        c.link = NIL;
    }
}

unsafe extern "C" fn vc_tick(this: Id, _: Sel, _link: Id) {
    let now = CACurrentMediaTime();
    {
        let c = controller(this);
        c.since_last_update = if c.last_update > 0.0 { now - c.last_update } else { 0.0 };
        c.last_update = now;
        c.since_first_resume = now - c.first_resume;
        c.since_last_resume = now - c.last_resume;
    }
    let delegate = controller(this).delegate.get();
    if !delegate.is_null() && pear_objc::responds_to(delegate, "glkViewControllerUpdate:") {
        msg1::<Id, ()>(delegate, "glkViewControllerUpdate:", this);
    } else if pear_objc::responds_to(this, "update") {
        msg0::<()>(this, "update");
    }
    let v: Id = msg0(this, "view");
    if msg1::<Id, bool>(v, "isKindOfClass:", glk_view::class()) {
        msg0::<()>(v, "display");
    }
    let c = controller(this);
    c.since_last_draw = if c.last_draw > 0.0 { now - c.last_draw } else { 0.0 };
    c.last_draw = now;
    c.frames_displayed += 1;
}

macro_rules! vc_getter {
    ($name:ident, $field:ident, $ty:ty) => {
        unsafe extern "C" fn $name(this: Id, _: Sel) -> $ty {
            controller(this).$field
        }
    };
}
vc_getter!(vc_frames_displayed, frames_displayed, isize);
vc_getter!(vc_since_first_resume, since_first_resume, f64);
vc_getter!(vc_since_last_resume, since_last_resume, f64);
vc_getter!(vc_since_last_update, since_last_update, f64);
vc_getter!(vc_since_last_draw, since_last_draw, f64);
vc_getter!(vc_pause_on_resign, pause_on_resign, bool);
vc_getter!(vc_resume_on_active, resume_on_active, bool);

unsafe extern "C" fn vc_set_pause_on_resign(this: Id, _: Sel, v: bool) {
    controller(this).pause_on_resign = v;
}
unsafe extern "C" fn vc_set_resume_on_active(this: Id, _: Sel, v: bool) {
    controller(this).resume_on_active = v;
}
unsafe extern "C" fn vc_delegate(this: Id, _: Sel) -> Id {
    controller(this).delegate.get()
}
unsafe extern "C" fn vc_set_delegate(this: Id, _: Sel, d: Id) {
    controller(this).delegate.set(d);
}

unsafe extern "C" fn vc_dealloc(this: Id, _: Sel) {
    stop(this);
    let slot = glk_view_controller::slot(this);
    if !(*slot).is_null() {
        let c = Box::from_raw(*slot as *mut Controller);
        *slot = std::ptr::null_mut();
        let center: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        for o in &c.observers {
            msg1::<Id, ()>(center, "removeObserver:", *o as Id);
            objc_release(*o as Id);
        }
    }
    pear_objc::super_dealloc(this, "PearGLKViewController");
}

crate::objc_class! {
    mod glk_view_controller = "PearGLKViewController" : (UIVIEWCONTROLLER_CLASS, UIVIEWCONTROLLER_META),
    state: true,
    methods {
        "initWithNibName:bundle:" "@32@0:8@16@24" => vc_init_nib,
        "initWithCoder:" "@24@0:8@16" => vc_init_coder,
        "loadView" "v16@0:8" => vc_load_view,
        "viewDidLoad" "v16@0:8" => vc_view_did_load,
        "viewDidAppear:" "v20@0:8B16" => vc_did_appear,
        "viewDidDisappear:" "v20@0:8B16" => vc_did_disappear,
        "framesPerSecond" "q16@0:8" => vc_fps,
        "preferredFramesPerSecond" "q16@0:8" => vc_preferred_fps,
        "setPreferredFramesPerSecond:" "v24@0:8q16" => vc_set_preferred_fps,
        "isPaused" "B16@0:8" => vc_paused,
        "setPaused:" "v20@0:8B16" => vc_set_paused,
        "framesDisplayed" "q16@0:8" => vc_frames_displayed,
        "timeSinceFirstResume" "d16@0:8" => vc_since_first_resume,
        "timeSinceLastResume" "d16@0:8" => vc_since_last_resume,
        "timeSinceLastUpdate" "d16@0:8" => vc_since_last_update,
        "timeSinceLastDraw" "d16@0:8" => vc_since_last_draw,
        "pauseOnWillResignActive" "B16@0:8" => vc_pause_on_resign,
        "setPauseOnWillResignActive:" "v20@0:8B16" => vc_set_pause_on_resign,
        "resumeOnDidBecomeActive" "B16@0:8" => vc_resume_on_active,
        "setResumeOnDidBecomeActive:" "v20@0:8B16" => vc_set_resume_on_active,
        "delegate" "@16@0:8" => vc_delegate,
        "setDelegate:" "v24@0:8@16" => vc_set_delegate,
        "pearTick:" "v24@0:8@16" => vc_tick,
        "dealloc" "v16@0:8" => vc_dealloc,
    }
    class_methods {}
}

pub fn view_class() -> pear_objc::Class {
    glk_view::class()
}

pub fn view_controller_class() -> pear_objc::Class {
    glk_view_controller::class()
}
