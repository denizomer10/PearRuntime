//! PearHost MetalKit.framework (`MTKView` as a `UIView` subclass) in Rust.

use pear_objc::{
    autorelease_pool, cls, msg0, msg1, msg2, msg4, msg_super0, msg_super1, object_getClass,
    objc_getAssociatedObject, objc_release, objc_retain, objc_setAssociatedObject,
    replace_method as add_method, responds_to, sel, super_dealloc, CGRect, CGSize, Class, Id,
    Imp, Sel, NIL, OBJC_ASSOCIATION_RETAIN,
};

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}
use std::ffi::{c_double, c_void};

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct MTLClearColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct CAFrameRateRange {
    minimum: f32,
    maximum: f32,
    preferred: f32,
}

const MTL_PIXEL_FORMAT_INVALID: usize = 0;
const MTL_PIXEL_FORMAT_BGRA8_UNORM: usize = 80;
const MTL_PIXEL_FORMAT_STENCIL8: usize = 253;
const MTL_PIXEL_FORMAT_DEPTH32_FLOAT_STENCIL8: usize = 260;
const MTL_PIXEL_FORMAT_X32_STENCIL8: usize = 261;
const MTL_TEXTURE_TYPE_2D: usize = 2;
const MTL_TEXTURE_TYPE_2D_MULTISAMPLE: usize = 4;
const MTL_STORAGE_MODE_PRIVATE: usize = 2;
const MTL_TEXTURE_USAGE_RENDER_TARGET: usize = 4;
const MTL_LOAD_ACTION_CLEAR: usize = 2;
const MTL_STORE_ACTION_DONT_CARE: usize = 0;
const MTL_STORE_ACTION_STORE: usize = 1;
const MTL_STORE_ACTION_MULTISAMPLE_RESOLVE: usize = 2;

#[link(name = "Metal", kind = "framework")]
extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}

struct MtkState {
    device: Id,
    delegate: Id, // zeroing weak slot (objc_storeWeak); heap address is stable
    current_drawable: Id,
    pass_descriptor: Id,
    depth_texture: Id,
    msaa_texture: Id,
    depth_format: usize,
    sample_count: usize,
    clear_color: MTLClearColor,
    clear_depth: f64,
    clear_stencil: u32,
    fps: isize,
    paused: bool,
    enable_set_needs_display: bool,
    auto_resize: bool,
    needs_display: bool,
    link: Id,
}

static MTK_STATE_KEY: u8 = 0;

unsafe fn mtk_state(obj: Id) -> Option<&'static mut MtkState> {
    let val = objc_getAssociatedObject(obj, &MTK_STATE_KEY as *const u8 as *const c_void);
    if val.is_null() {
        return None;
    }
    let ptr: *mut c_void = msg0(val, "pointerValue");
    if ptr.is_null() {
        None
    } else {
        Some(&mut *(ptr as *mut MtkState))
    }
}

/// The delegate, retained for the duration of a call (nil once it is gone).
unsafe fn delegate_retained(s: &mut MtkState) -> Id {
    pear_objc::objc_loadWeakRetained(&mut s.delegate)
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

unsafe fn metal_layer(obj: Id) -> Id {
    msg0(obj, "layer")
}

unsafe fn mtk_setup(obj: Id, device: Id) {
    let state = Box::new(MtkState {
        device: NIL,
        delegate: NIL,
        current_drawable: NIL,
        pass_descriptor: NIL,
        depth_texture: NIL,
        msaa_texture: NIL,
        depth_format: MTL_PIXEL_FORMAT_INVALID,
        sample_count: 1,
        clear_color: MTLClearColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        clear_depth: 1.0,
        clear_stencil: 0,
        fps: 60,
        paused: false,
        enable_set_needs_display: false,
        auto_resize: true,
        needs_display: false,
        link: NIL,
    });
    let ptr = Box::into_raw(state) as *mut c_void;
    let val: Id = msg1::<*mut c_void, Id>(cls("NSValue"), "valueWithPointer:", ptr);
    objc_setAssociatedObject(obj, &MTK_STATE_KEY as *const u8 as *const c_void, val, OBJC_ASSOCIATION_RETAIN);

    let layer = metal_layer(obj);
    msg1::<usize, ()>(layer, "setPixelFormat:", MTL_PIXEL_FORMAT_BGRA8_UNORM);
    msg1::<bool, ()>(layer, "setFramebufferOnly:", true);
    msg1::<Id, ()>(obj, "setDevice:", device);
    let main_scr: Id = msg0(cls("UIScreen"), "mainScreen");
    let scale: f64 = msg0(main_scr, "scale");
    msg1::<f64, ()>(obj, "setContentScaleFactor:", scale);
}

unsafe extern "C" fn mtk_layer_class(_cls: Id, _cmd: Sel) -> Class {
    cls("CAMetalLayer")
}

unsafe extern "C" fn mtk_init_with_frame(self_obj: Id, _cmd: Sel, frame: CGRect) -> Id {
    msg2::<CGRect, Id, Id>(self_obj, "initWithFrame:device:", frame, NIL)
}

unsafe extern "C" fn mtk_init_with_frame_device(self_obj: Id, _cmd: Sel, frame: CGRect, device: Id) -> Id {
    let obj: Id = msg_super1(self_obj, "MTKView", "initWithFrame:", frame);
    if !obj.is_null() {
        mtk_setup(obj, device);
    }
    obj
}

unsafe extern "C" fn mtk_init_with_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = msg_super1(self_obj, "MTKView", "initWithCoder:", coder);
    if !obj.is_null() {
        mtk_setup(obj, NIL);
    }
    obj
}

unsafe extern "C" fn mtk_dealloc(self_obj: Id, _cmd: Sel) {
    let val = objc_getAssociatedObject(self_obj, &MTK_STATE_KEY as *const u8 as *const c_void);
    if !val.is_null() {
        let ptr: *mut c_void = msg0(val, "pointerValue");
        if !ptr.is_null() {
            let mut s = Box::from_raw(ptr as *mut MtkState);
            if !s.link.is_null() {
                msg0::<()>(s.link, "invalidate");
                set_retained(&mut s.link, NIL);
            }
            pear_objc::objc_destroyWeak(&mut s.delegate);
            set_retained(&mut s.device, NIL);
            set_retained(&mut s.current_drawable, NIL);
            set_retained(&mut s.pass_descriptor, NIL);
            set_retained(&mut s.depth_texture, NIL);
            set_retained(&mut s.msaa_texture, NIL);
        }
        objc_setAssociatedObject(self_obj, &MTK_STATE_KEY as *const u8 as *const c_void, NIL, OBJC_ASSOCIATION_RETAIN);
    }
    super_dealloc(self_obj, "MTKView");
}

unsafe extern "C" fn mtk_device(self_obj: Id, _cmd: Sel) -> Id {
    mtk_state(self_obj).map(|s| s.device).unwrap_or(NIL)
}

unsafe extern "C" fn mtk_set_device(self_obj: Id, _cmd: Sel, device: Id) {
    if let Some(s) = mtk_state(self_obj) {
        set_retained(&mut s.device, device);
        msg1::<Id, ()>(metal_layer(self_obj), "setDevice:", device);
        set_retained(&mut s.depth_texture, NIL);
        set_retained(&mut s.msaa_texture, NIL);
    }
}

unsafe extern "C" fn mtk_preferred_device(_self_obj: Id, _cmd: Sel) -> Id {
    MTLCreateSystemDefaultDevice()
}

unsafe extern "C" fn mtk_delegate(self_obj: Id, _cmd: Sel) -> Id {
    match mtk_state(self_obj) {
        Some(s) => pear_objc::autoreleased(delegate_retained(s)),
        None => NIL,
    }
}

unsafe extern "C" fn mtk_set_delegate(self_obj: Id, _cmd: Sel, delegate: Id) {
    if let Some(s) = mtk_state(self_obj) {
        pear_objc::objc_storeWeak(&mut s.delegate, delegate);
        mtk_update_loop(self_obj);
    }
}

unsafe extern "C" fn mtk_color_pixel_format(self_obj: Id, _cmd: Sel) -> usize {
    msg0(metal_layer(self_obj), "pixelFormat")
}

unsafe extern "C" fn mtk_set_color_pixel_format(self_obj: Id, _cmd: Sel, fmt: usize) {
    msg1::<usize, ()>(metal_layer(self_obj), "setPixelFormat:", fmt);
    if let Some(s) = mtk_state(self_obj) {
        set_retained(&mut s.msaa_texture, NIL);
    }
}

unsafe extern "C" fn mtk_depth_pixel_format(self_obj: Id, _cmd: Sel) -> usize {
    mtk_state(self_obj).map(|s| s.depth_format).unwrap_or(0)
}

unsafe extern "C" fn mtk_set_depth_pixel_format(self_obj: Id, _cmd: Sel, fmt: usize) {
    if let Some(s) = mtk_state(self_obj) {
        s.depth_format = fmt;
        set_retained(&mut s.depth_texture, NIL);
    }
}

unsafe extern "C" fn mtk_sample_count(self_obj: Id, _cmd: Sel) -> usize {
    mtk_state(self_obj).map(|s| s.sample_count).unwrap_or(1)
}

unsafe extern "C" fn mtk_set_sample_count(self_obj: Id, _cmd: Sel, n: usize) {
    if let Some(s) = mtk_state(self_obj) {
        s.sample_count = n.max(1);
        set_retained(&mut s.depth_texture, NIL);
        set_retained(&mut s.msaa_texture, NIL);
    }
}

unsafe extern "C" fn mtk_clear_color(self_obj: Id, _cmd: Sel) -> MTLClearColor {
    mtk_state(self_obj).map(|s| s.clear_color).unwrap_or_default()
}

unsafe extern "C" fn mtk_set_clear_color(self_obj: Id, _cmd: Sel, c: MTLClearColor) {
    if let Some(s) = mtk_state(self_obj) {
        s.clear_color = c;
    }
}

unsafe extern "C" fn mtk_clear_depth(self_obj: Id, _cmd: Sel) -> c_double {
    mtk_state(self_obj).map(|s| s.clear_depth).unwrap_or(1.0)
}

unsafe extern "C" fn mtk_set_clear_depth(self_obj: Id, _cmd: Sel, d: c_double) {
    if let Some(s) = mtk_state(self_obj) {
        s.clear_depth = d;
    }
}

unsafe extern "C" fn mtk_clear_stencil(self_obj: Id, _cmd: Sel) -> u32 {
    mtk_state(self_obj).map(|s| s.clear_stencil).unwrap_or(0)
}

unsafe extern "C" fn mtk_set_clear_stencil(self_obj: Id, _cmd: Sel, st: u32) {
    if let Some(s) = mtk_state(self_obj) {
        s.clear_stencil = st;
    }
}

unsafe extern "C" fn mtk_framebuffer_only(self_obj: Id, _cmd: Sel) -> bool {
    msg0(metal_layer(self_obj), "framebufferOnly")
}

unsafe extern "C" fn mtk_set_framebuffer_only(self_obj: Id, _cmd: Sel, f: bool) {
    msg1::<bool, ()>(metal_layer(self_obj), "setFramebufferOnly:", f);
}

unsafe extern "C" fn mtk_presents_with_transaction(self_obj: Id, _cmd: Sel) -> bool {
    msg0(metal_layer(self_obj), "presentsWithTransaction")
}

unsafe extern "C" fn mtk_set_presents_with_transaction(self_obj: Id, _cmd: Sel, p: bool) {
    msg1::<bool, ()>(metal_layer(self_obj), "setPresentsWithTransaction:", p);
}

unsafe extern "C" fn mtk_depth_usage(_self_obj: Id, _cmd: Sel) -> usize {
    MTL_TEXTURE_USAGE_RENDER_TARGET
}

unsafe extern "C" fn mtk_preferred_fps(self_obj: Id, _cmd: Sel) -> isize {
    mtk_state(self_obj).map(|s| s.fps).unwrap_or(60)
}

unsafe extern "C" fn mtk_set_preferred_fps(self_obj: Id, _cmd: Sel, fps: isize) {
    if let Some(s) = mtk_state(self_obj) {
        s.fps = fps;
        mtk_update_loop(self_obj);
    }
}

unsafe extern "C" fn mtk_is_paused(self_obj: Id, _cmd: Sel) -> bool {
    mtk_state(self_obj).map(|s| s.paused).unwrap_or(false)
}

unsafe extern "C" fn mtk_set_paused(self_obj: Id, _cmd: Sel, p: bool) {
    if let Some(s) = mtk_state(self_obj) {
        s.paused = p;
        mtk_update_loop(self_obj);
    }
}

unsafe extern "C" fn mtk_enable_set_needs_display(self_obj: Id, _cmd: Sel) -> bool {
    mtk_state(self_obj).map(|s| s.enable_set_needs_display).unwrap_or(false)
}

unsafe extern "C" fn mtk_set_enable_set_needs_display(self_obj: Id, _cmd: Sel, e: bool) {
    if let Some(s) = mtk_state(self_obj) {
        s.enable_set_needs_display = e;
        mtk_update_loop(self_obj);
    }
}

unsafe extern "C" fn mtk_auto_resize(self_obj: Id, _cmd: Sel) -> bool {
    mtk_state(self_obj).map(|s| s.auto_resize).unwrap_or(true)
}

unsafe extern "C" fn mtk_set_auto_resize(self_obj: Id, _cmd: Sel, a: bool) {
    if let Some(s) = mtk_state(self_obj) {
        s.auto_resize = a;
    }
}

unsafe extern "C" fn mtk_drawable_size(self_obj: Id, _cmd: Sel) -> CGSize {
    msg0(metal_layer(self_obj), "drawableSize")
}

unsafe extern "C" fn mtk_set_drawable_size(self_obj: Id, _cmd: Sel, size: CGSize) {
    let layer = metal_layer(self_obj);
    let cur: CGSize = msg0(layer, "drawableSize");
    if cur == size {
        return;
    }
    msg1::<CGSize, ()>(layer, "setDrawableSize:", size);
    if let Some(s) = mtk_state(self_obj) {
        set_retained(&mut s.depth_texture, NIL);
        set_retained(&mut s.msaa_texture, NIL);
        let d = delegate_retained(s);
        if !d.is_null() {
            if responds_to(d, "mtkView:drawableSizeWillChange:") {
                msg2::<Id, CGSize, ()>(d, "mtkView:drawableSizeWillChange:", self_obj, size);
            }
            objc_release(d);
        }
    }
}

unsafe extern "C" fn mtk_preferred_drawable_size(self_obj: Id, _cmd: Sel) -> CGSize {
    let scale: f64 = msg0(self_obj, "contentScaleFactor");
    let bounds: CGRect = msg0(self_obj, "bounds");
    CGSize {
        width: bounds.size.width * scale,
        height: bounds.size.height * scale,
    }
}

unsafe fn mtk_resize_drawable(self_obj: Id) {
    let Some(s) = mtk_state(self_obj) else { return };
    if !s.auto_resize {
        return;
    }
    let want: CGSize = msg0(self_obj, "preferredDrawableSize");
    if want.width < 1.0 || want.height < 1.0 {
        return;
    }
    msg1::<CGSize, ()>(self_obj, "setDrawableSize:", want);
}

unsafe fn mtk_update_loop(self_obj: Id) {
    let Some(s) = mtk_state(self_obj) else { return };
    let window: Id = msg0(self_obj, "window");
    let want = !window.is_null() && !s.paused && !s.enable_set_needs_display;
    if want && s.link.is_null() {
        let scr: Id = msg0(cls("UIScreen"), "mainScreen");
        let link: Id = msg2::<Id, Sel, Id>(scr, "displayLinkWithTarget:selector:", self_obj, sel("pear_tick:"));
        if !link.is_null() {
            let run_loop: Id = msg0(cls("NSRunLoop"), "mainRunLoop");
            msg2::<Id, Id, ()>(link, "addToRunLoop:forMode:", run_loop, pear_objc::nsstring("kCFRunLoopCommonModes"));
            set_retained(&mut s.link, link);
        }
    } else if !want && !s.link.is_null() {
        msg0::<()>(s.link, "invalidate");
        set_retained(&mut s.link, NIL);
    }
    if !s.link.is_null() && s.fps > 0 {
        let f = s.fps as f32;
        msg1::<CAFrameRateRange, ()>(
            s.link,
            "setPreferredFrameRateRange:",
            CAFrameRateRange {
                minimum: f,
                maximum: f,
                preferred: f,
            },
        );
    }
}

unsafe extern "C" fn mtk_layout_subviews(self_obj: Id, _cmd: Sel) {
    msg_super0::<()>(self_obj, "MTKView", "layoutSubviews");
    mtk_resize_drawable(self_obj);
}

unsafe extern "C" fn mtk_did_move_to_window(self_obj: Id, _cmd: Sel) {
    msg_super0::<()>(self_obj, "MTKView", "didMoveToWindow");
    mtk_resize_drawable(self_obj);
    mtk_update_loop(self_obj);
}

// The drawable follows the view's size (autoResizeDrawable), so every
// geometry change resizes it, not only layout passes.
unsafe extern "C" fn mtk_set_frame(self_obj: Id, _cmd: Sel, frame: CGRect) {
    msg_super1::<CGRect, ()>(self_obj, "MTKView", "setFrame:", frame);
    mtk_resize_drawable(self_obj);
}

unsafe extern "C" fn mtk_set_bounds(self_obj: Id, _cmd: Sel, bounds: CGRect) {
    msg_super1::<CGRect, ()>(self_obj, "MTKView", "setBounds:", bounds);
    mtk_resize_drawable(self_obj);
}

unsafe extern "C" fn mtk_set_content_scale(self_obj: Id, _cmd: Sel, scale: f64) {
    msg_super1::<f64, ()>(self_obj, "MTKView", "setContentScaleFactor:", scale);
    mtk_resize_drawable(self_obj);
}

unsafe extern "C" fn mtk_colorspace(self_obj: Id, _cmd: Sel) -> *mut c_void {
    msg0(metal_layer(self_obj), "colorspace")
}

unsafe extern "C" fn mtk_set_colorspace(self_obj: Id, _cmd: Sel, cs: *mut c_void) {
    msg1::<*mut c_void, ()>(metal_layer(self_obj), "setColorspace:", cs);
}

extern "C" fn mtk_deferred_draw(ctx: *mut c_void) {
    unsafe {
        let view = ctx as Id;
        if let Some(s) = mtk_state(view) {
            s.needs_display = false;
            msg0::<()>(view, "draw");
        }
        objc_release(view);
    }
}

// With enableSetNeedsDisplay the view draws once per setNeedsDisplay, on the
// next main-queue turn (coalesced), instead of from the display link.
unsafe extern "C" fn mtk_set_needs_display(self_obj: Id, _cmd: Sel) {
    msg_super0::<()>(self_obj, "MTKView", "setNeedsDisplay");
    let Some(s) = mtk_state(self_obj) else { return };
    if s.enable_set_needs_display && !s.needs_display {
        s.needs_display = true;
        objc_retain(self_obj);
        dispatch_async_f(&_dispatch_main_q as *const c_void, self_obj, mtk_deferred_draw);
    }
}

unsafe extern "C" fn mtk_tick(self_obj: Id, _cmd: Sel, _link: Id) {
    msg0::<()>(self_obj, "draw");
}

unsafe extern "C" fn mtk_draw(self_obj: Id, _cmd: Sel) {
    autorelease_pool(|| {
        let Some(s) = mtk_state(self_obj) else { return };
        set_retained(&mut s.current_drawable, NIL);
        set_retained(&mut s.pass_descriptor, NIL);
        let d = delegate_retained(s);
        if !d.is_null() {
            msg1::<Id, ()>(d, "drawInMTKView:", self_obj);
            objc_release(d);
        } else {
            let bounds: CGRect = msg0(self_obj, "bounds");
            msg1::<CGRect, ()>(self_obj, "drawRect:", bounds);
        }
        set_retained(&mut s.current_drawable, NIL);
        set_retained(&mut s.pass_descriptor, NIL);
    });
}

unsafe extern "C" fn mtk_release_drawables(self_obj: Id, _cmd: Sel) {
    if let Some(s) = mtk_state(self_obj) {
        set_retained(&mut s.depth_texture, NIL);
        set_retained(&mut s.msaa_texture, NIL);
        set_retained(&mut s.current_drawable, NIL);
    }
}

unsafe extern "C" fn mtk_current_drawable(self_obj: Id, _cmd: Sel) -> Id {
    let Some(s) = mtk_state(self_obj) else { return NIL };
    if s.current_drawable.is_null() {
        let next: Id = msg0(metal_layer(self_obj), "nextDrawable");
        set_retained(&mut s.current_drawable, next);
    }
    s.current_drawable
}

unsafe fn mtk_make_texture(self_obj: Id, format: usize, samples: usize, usage: usize) -> Id {
    let Some(s) = mtk_state(self_obj) else { return NIL };
    let size: CGSize = msg0(self_obj, "drawableSize");
    if s.device.is_null() || size.width < 1.0 || size.height < 1.0 {
        return NIL;
    }
    let desc: Id = msg4::<usize, usize, usize, bool, Id>(
        cls("MTLTextureDescriptor"),
        "texture2DDescriptorWithPixelFormat:width:height:mipmapped:",
        format,
        size.width as usize,
        size.height as usize,
        false,
    );
    msg1::<usize, ()>(
        desc,
        "setTextureType:",
        if samples > 1 {
            MTL_TEXTURE_TYPE_2D_MULTISAMPLE
        } else {
            MTL_TEXTURE_TYPE_2D
        },
    );
    msg1::<usize, ()>(desc, "setSampleCount:", samples);
    msg1::<usize, ()>(desc, "setUsage:", usage);
    msg1::<usize, ()>(desc, "setStorageMode:", MTL_STORAGE_MODE_PRIVATE);
    msg1::<Id, Id>(s.device, "newTextureWithDescriptor:", desc)
}

unsafe extern "C" fn mtk_depth_stencil_texture(self_obj: Id, _cmd: Sel) -> Id {
    let Some(s) = mtk_state(self_obj) else { return NIL };
    if s.depth_format == MTL_PIXEL_FORMAT_INVALID {
        return NIL;
    }
    if s.depth_texture.is_null() {
        let tex = mtk_make_texture(self_obj, s.depth_format, s.sample_count, MTL_TEXTURE_USAGE_RENDER_TARGET);
        set_retained(&mut s.depth_texture, tex);
        if !tex.is_null() {
            objc_release(tex);
        }
    }
    s.depth_texture
}

unsafe extern "C" fn mtk_multisample_color_texture(self_obj: Id, _cmd: Sel) -> Id {
    let Some(s) = mtk_state(self_obj) else { return NIL };
    if s.sample_count <= 1 {
        return NIL;
    }
    if s.msaa_texture.is_null() {
        let fmt = mtk_color_pixel_format(self_obj, sel("colorPixelFormat"));
        let tex = mtk_make_texture(self_obj, fmt, s.sample_count, MTL_TEXTURE_USAGE_RENDER_TARGET);
        set_retained(&mut s.msaa_texture, tex);
        if !tex.is_null() {
            objc_release(tex);
        }
    }
    s.msaa_texture
}

unsafe extern "C" fn mtk_current_render_pass_descriptor(self_obj: Id, _cmd: Sel) -> Id {
    let Some(s) = mtk_state(self_obj) else { return NIL };
    if !s.pass_descriptor.is_null() {
        return s.pass_descriptor;
    }
    let drawable = mtk_current_drawable(self_obj, sel("currentDrawable"));
    if drawable.is_null() {
        return NIL;
    }
    let p: Id = msg0(cls("MTLRenderPassDescriptor"), "renderPassDescriptor");
    let color_atts: Id = msg0(p, "colorAttachments");
    let c: Id = msg1::<usize, Id>(color_atts, "objectAtIndexedSubscript:", 0);
    let draw_tex: Id = msg0(drawable, "texture");
    if s.sample_count > 1 {
        let msaa = mtk_multisample_color_texture(self_obj, sel("multisampleColorTexture"));
        msg1::<Id, ()>(c, "setTexture:", msaa);
        msg1::<Id, ()>(c, "setResolveTexture:", draw_tex);
        msg1::<usize, ()>(c, "setStoreAction:", MTL_STORE_ACTION_MULTISAMPLE_RESOLVE);
    } else {
        msg1::<Id, ()>(c, "setTexture:", draw_tex);
        msg1::<usize, ()>(c, "setStoreAction:", MTL_STORE_ACTION_STORE);
    }
    msg1::<usize, ()>(c, "setLoadAction:", MTL_LOAD_ACTION_CLEAR);
    msg1::<MTLClearColor, ()>(c, "setClearColor:", s.clear_color);

    let depth = mtk_depth_stencil_texture(self_obj, sel("depthStencilTexture"));
    if !depth.is_null() {
        let da: Id = msg0(p, "depthAttachment");
        msg1::<Id, ()>(da, "setTexture:", depth);
        msg1::<usize, ()>(da, "setLoadAction:", MTL_LOAD_ACTION_CLEAR);
        msg1::<usize, ()>(da, "setStoreAction:", MTL_STORE_ACTION_DONT_CARE);
        msg1::<f64, ()>(da, "setClearDepth:", s.clear_depth);

        if s.depth_format == MTL_PIXEL_FORMAT_DEPTH32_FLOAT_STENCIL8
            || s.depth_format == MTL_PIXEL_FORMAT_STENCIL8
            || s.depth_format == MTL_PIXEL_FORMAT_X32_STENCIL8
        {
            let sa: Id = msg0(p, "stencilAttachment");
            msg1::<Id, ()>(sa, "setTexture:", depth);
            msg1::<usize, ()>(sa, "setLoadAction:", MTL_LOAD_ACTION_CLEAR);
            msg1::<usize, ()>(sa, "setStoreAction:", MTL_STORE_ACTION_DONT_CARE);
            msg1::<u32, ()>(sa, "setClearStencil:", s.clear_stencil);
        }
    }
    set_retained(&mut s.pass_descriptor, p);
    p
}

#[no_mangle]
pub extern "C" fn pear_metalkit_register_classes() {
    unsafe {
        let mtk_cls: Class = cls("MTKView");
        if mtk_cls.is_null() {
            return;
        }
        let meta: Class = object_getClass(mtk_cls);
        add_method(meta, "layerClass", mtk_layer_class as Imp, "#@:");

        add_method(mtk_cls, "initWithFrame:", mtk_init_with_frame as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(mtk_cls, "initWithFrame:device:", mtk_init_with_frame_device as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}@");
        add_method(mtk_cls, "initWithCoder:", mtk_init_with_coder as Imp, "@@:@");
        add_method(mtk_cls, "dealloc", mtk_dealloc as Imp, "v@:");

        add_method(mtk_cls, "device", mtk_device as Imp, "@@:");
        add_method(mtk_cls, "setDevice:", mtk_set_device as Imp, "v@:@");
        add_method(mtk_cls, "preferredDevice", mtk_preferred_device as Imp, "@@:");
        add_method(mtk_cls, "delegate", mtk_delegate as Imp, "@@:");
        add_method(mtk_cls, "setDelegate:", mtk_set_delegate as Imp, "v@:@");

        add_method(mtk_cls, "colorPixelFormat", mtk_color_pixel_format as Imp, "Q@:");
        add_method(mtk_cls, "setColorPixelFormat:", mtk_set_color_pixel_format as Imp, "v@:Q");
        add_method(mtk_cls, "depthStencilPixelFormat", mtk_depth_pixel_format as Imp, "Q@:");
        add_method(mtk_cls, "setDepthStencilPixelFormat:", mtk_set_depth_pixel_format as Imp, "v@:Q");
        add_method(mtk_cls, "sampleCount", mtk_sample_count as Imp, "Q@:");
        add_method(mtk_cls, "setSampleCount:", mtk_set_sample_count as Imp, "v@:Q");
        add_method(mtk_cls, "clearColor", mtk_clear_color as Imp, "{?=dddd}@:");
        add_method(mtk_cls, "setClearColor:", mtk_set_clear_color as Imp, "v@:{?=dddd}");
        add_method(mtk_cls, "clearDepth", mtk_clear_depth as Imp, "d@:");
        add_method(mtk_cls, "setClearDepth:", mtk_set_clear_depth as Imp, "v@:d");
        add_method(mtk_cls, "clearStencil", mtk_clear_stencil as Imp, "I@:");
        add_method(mtk_cls, "setClearStencil:", mtk_set_clear_stencil as Imp, "v@:I");
        add_method(mtk_cls, "framebufferOnly", mtk_framebuffer_only as Imp, "B@:");
        add_method(mtk_cls, "setFramebufferOnly:", mtk_set_framebuffer_only as Imp, "v@:B");
        add_method(mtk_cls, "presentsWithTransaction", mtk_presents_with_transaction as Imp, "B@:");
        add_method(mtk_cls, "setPresentsWithTransaction:", mtk_set_presents_with_transaction as Imp, "v@:B");
        add_method(mtk_cls, "depthStencilAttachmentTextureUsage", mtk_depth_usage as Imp, "Q@:");
        add_method(mtk_cls, "preferredFramesPerSecond", mtk_preferred_fps as Imp, "q@:");
        add_method(mtk_cls, "setPreferredFramesPerSecond:", mtk_set_preferred_fps as Imp, "v@:q");
        add_method(mtk_cls, "isPaused", mtk_is_paused as Imp, "B@:");
        add_method(mtk_cls, "setPaused:", mtk_set_paused as Imp, "v@:B");
        add_method(mtk_cls, "enableSetNeedsDisplay", mtk_enable_set_needs_display as Imp, "B@:");
        add_method(mtk_cls, "setEnableSetNeedsDisplay:", mtk_set_enable_set_needs_display as Imp, "v@:B");
        add_method(mtk_cls, "autoResizeDrawable", mtk_auto_resize as Imp, "B@:");
        add_method(mtk_cls, "setAutoResizeDrawable:", mtk_set_auto_resize as Imp, "v@:B");
        add_method(mtk_cls, "drawableSize", mtk_drawable_size as Imp, "{CGSize=dd}@:");
        add_method(mtk_cls, "setDrawableSize:", mtk_set_drawable_size as Imp, "v@:{CGSize=dd}");
        add_method(mtk_cls, "preferredDrawableSize", mtk_preferred_drawable_size as Imp, "{CGSize=dd}@:");

        add_method(mtk_cls, "layoutSubviews", mtk_layout_subviews as Imp, "v@:");
        add_method(mtk_cls, "setFrame:", mtk_set_frame as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(mtk_cls, "setBounds:", mtk_set_bounds as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(mtk_cls, "setContentScaleFactor:", mtk_set_content_scale as Imp, "v@:d");
        add_method(mtk_cls, "colorspace", mtk_colorspace as Imp, "^{CGColorSpace=}@:");
        add_method(mtk_cls, "setColorspace:", mtk_set_colorspace as Imp, "v@:^{CGColorSpace=}");
        add_method(mtk_cls, "setNeedsDisplay", mtk_set_needs_display as Imp, "v@:");
        add_method(mtk_cls, "didMoveToWindow", mtk_did_move_to_window as Imp, "v@:");
        add_method(mtk_cls, "pear_tick:", mtk_tick as Imp, "v@:@");
        add_method(mtk_cls, "draw", mtk_draw as Imp, "v@:");
        add_method(mtk_cls, "releaseDrawables", mtk_release_drawables as Imp, "v@:");
        add_method(mtk_cls, "currentDrawable", mtk_current_drawable as Imp, "@@:");
        add_method(mtk_cls, "depthStencilTexture", mtk_depth_stencil_texture as Imp, "@@:");
        add_method(mtk_cls, "multisampleColorTexture", mtk_multisample_color_texture as Imp, "@@:");
        add_method(mtk_cls, "currentRenderPassDescriptor", mtk_current_render_pass_descriptor as Imp, "@@:");
    }
}

extern "C" {
    fn PearMissingInstallForImage(address_in_image: *const c_void);
}

/// Image initializer: methods onto the class symbols, then the missing-call reporter.
extern "C" fn image_load() {
    pear_metalkit_register_classes();
    unsafe { PearMissingInstallForImage(image_load as *const c_void) };
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_MTK_INIT: extern "C" fn() = image_load;

// ---- MetalKit.framework ABI -------------------------------------------------
// The MTKView class symbol iOS binaries bind to: a UIView subclass (UIView
// from PearHost UIKit.framework). Every method is registered on it by
// `pear_metalkit_register_classes`.
mod abi {
    extern "C" {
        #[link_name = "OBJC_CLASS_$_UIView"]
        static UIView_CLASS: std::ffi::c_void;
        #[link_name = "OBJC_METACLASS_$_UIView"]
        static UIView_META: std::ffi::c_void;
    }

    pear_objc::image_info!();
    pear_objc::empty_class!(MTKView : &raw const super::UIView_CLASS, &raw const super::UIView_META);
}
