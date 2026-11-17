//! UIColor, UIImage, UIImagePNGRepresentation/JPEGRepresentation, UIGraphics*, UIFont, UILabel, UIProgressView.

use pear_objc::{
    cls, msg0, msg1, msg2, object_getClass, objc_getAssociatedObject, objc_release,
    objc_retain, objc_setAssociatedObject, replace_method as add_method, CGRect, CGSize, Class, Id,
    Imp, Sel, NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::ffi::{c_double, c_float, c_void};
use std::sync::Mutex;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorCreateSRGB(r: f64, g: f64, b: f64, a: f64) -> *mut c_void;
    fn CGColorRelease(color: *mut c_void);
    fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
    fn CGColorSpaceRelease(space: *mut c_void);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: *mut c_void,
        bitmap_info: u32,
    ) -> *mut c_void;
    fn CGBitmapContextCreateImage(ctx: *mut c_void) -> *mut c_void;
    fn CGContextRelease(ctx: *mut c_void);
    fn CGContextRetain(ctx: *mut c_void) -> *mut c_void;
    fn CGContextTranslateCTM(ctx: *mut c_void, tx: f64, ty: f64);
    fn CGContextScaleCTM(ctx: *mut c_void, sx: f64, sy: f64);
    fn CGContextSaveGState(ctx: *mut c_void);
    fn CGContextRestoreGState(ctx: *mut c_void);
    fn CGContextDrawImage(ctx: *mut c_void, rect: CGRect, image: *mut c_void);
    fn CGContextSetTextMatrix(ctx: *mut c_void, t: CGAffineTransform);
    fn CGImageGetWidth(image: *mut c_void) -> usize;
    fn CGImageGetHeight(image: *mut c_void) -> usize;
    fn CGImageRelease(image: *mut c_void);
    fn CGImageRetain(image: *mut c_void) -> *mut c_void;
    fn CGPDFContextCreateWithURL(url: *mut c_void, media_box: *const CGRect, aux_info: *mut c_void) -> *mut c_void;
    fn CGPDFContextCreate(consumer: *mut c_void, media_box: *const CGRect, aux_info: *mut c_void) -> *mut c_void;
    fn CGPDFContextBeginPage(ctx: *mut c_void, page_info: *mut c_void);
    fn CGPDFContextEndPage(ctx: *mut c_void);
    fn CGPDFContextClose(ctx: *mut c_void);
    fn CGDataConsumerCreateWithCFData(data: *mut c_void) -> *mut c_void;
    fn CGDataConsumerRelease(consumer: *mut c_void);
    fn CGPathCreateWithRect(rect: CGRect, transform: *const c_void) -> *mut c_void;
    fn CGPathRelease(path: *mut c_void);
}

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    fn CGImageSourceCreateWithData(data: *mut c_void, options: *mut c_void) -> *mut c_void;
    fn CGImageSourceCreateImageAtIndex(isrc: *mut c_void, index: usize, options: *mut c_void) -> *mut c_void;
    fn CGImageDestinationCreateWithData(
        data: *mut c_void,
        uti: *mut c_void,
        count: usize,
        options: *mut c_void,
    ) -> *mut c_void;
    fn CGImageDestinationAddImage(dest: *mut c_void, image: *mut c_void, properties: *mut c_void);
    fn CGImageDestinationFinalize(dest: *mut c_void) -> bool;
}

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontCopyFamilyName(font: *mut c_void) -> *mut c_void;
    fn CTFontCopyPostScriptName(font: *mut c_void) -> *mut c_void;
    fn CTFontGetSize(font: *mut c_void) -> f64;
    fn CTFontGetAscent(font: *mut c_void) -> f64;
    fn CTFontGetDescent(font: *mut c_void) -> f64;
    fn CTFontGetCapHeight(font: *mut c_void) -> f64;
    fn CTFontGetXHeight(font: *mut c_void) -> f64;
    fn CTFontGetLeading(font: *mut c_void) -> f64;
    fn CTFontCreateCopyWithAttributes(
        font: *mut c_void,
        size: f64,
        matrix: *const c_void,
        attrs: *mut c_void,
    ) -> *mut c_void;
    fn CTFramesetterCreateWithAttributedString(attr_str: *mut c_void) -> *mut c_void;
    fn CTFramesetterSuggestFrameSizeWithConstraints(
        framesetter: *mut c_void,
        range: CFRange,
        frame_attrs: *mut c_void,
        constraints: CGSize,
        fit_range: *mut CFRange,
    ) -> CGSize;
    fn CTFramesetterCreateFrame(
        framesetter: *mut c_void,
        range: CFRange,
        path: *mut c_void,
        frame_attrs: *mut c_void,
    ) -> *mut c_void;
    fn CTFrameDraw(frame: *mut c_void, ctx: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *mut c_void);
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct CGAffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

// ---- UIColor ----
static COLOR_KEY: u8 = 0;

unsafe extern "C" fn color_init_rgba(
    self_obj: Id,
    _cmd: Sel,
    r: c_double,
    g: c_double,
    b: c_double,
    a: c_double,
) -> Id {
    let obj: Id = msg0(self_obj, "init");
    if !obj.is_null() {
        let cg = CGColorCreateSRGB(r, g, b, a);
        let val: Id = msg1::<*mut c_void, Id>(cls("NSValue"), "valueWithPointer:", cg);
        objc_setAssociatedObject(obj, &COLOR_KEY as *const u8 as *const c_void, val, OBJC_ASSOCIATION_RETAIN);
    }
    obj
}

unsafe extern "C" fn color_with_rgba(
    self_cls: Class,
    _cmd: Sel,
    r: c_double,
    g: c_double,
    b: c_double,
    a: c_double,
) -> Id {
    let alloc: Id = msg0(self_cls, "alloc");
    let f: unsafe extern "C" fn(Id, Sel, f64, f64, f64, f64) -> Id =
        std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
    f(alloc, pear_objc::sel("initWithRed:green:blue:alpha:"), r, g, b, a)
}

unsafe extern "C" fn color_with_white(self_cls: Class, _cmd: Sel, w: c_double, a: c_double) -> Id {
    color_with_rgba(self_cls, _cmd, w, w, w, a)
}

unsafe extern "C" fn color_clear(self_cls: Class, _cmd: Sel) -> Id {
    color_with_rgba(self_cls, _cmd, 0.0, 0.0, 0.0, 0.0)
}
unsafe extern "C" fn color_black(self_cls: Class, _cmd: Sel) -> Id {
    color_with_rgba(self_cls, _cmd, 0.0, 0.0, 0.0, 1.0)
}
unsafe extern "C" fn color_white(self_cls: Class, _cmd: Sel) -> Id {
    color_with_rgba(self_cls, _cmd, 1.0, 1.0, 1.0, 1.0)
}

unsafe extern "C" fn color_cgcolor(self_obj: Id, _cmd: Sel) -> *mut c_void {
    let val = objc_getAssociatedObject(self_obj, &COLOR_KEY as *const u8 as *const c_void);
    if val.is_null() {
        std::ptr::null_mut()
    } else {
        msg0(val, "pointerValue")
    }
}

unsafe extern "C" fn color_dealloc(self_obj: Id, _cmd: Sel) {
    let cg = color_cgcolor(self_obj, _cmd);
    if !cg.is_null() {
        CGColorRelease(cg);
    }
}

unsafe extern "C" fn copy_self(self_obj: Id, _cmd: Sel, _zone: *mut c_void) -> Id {
    objc_retain(self_obj)
}

// ---- UIImage ----
struct ImageState {
    cg_image: *mut c_void,
    scale: f64,
    orientation: isize,
    rendering_mode: isize,
}
static IMAGE_STATE_KEY: u8 = 0;

unsafe fn image_state(obj: Id) -> Option<&'static mut ImageState> {
    if obj.is_null() {
        return None;
    }
    let val = objc_getAssociatedObject(obj, &IMAGE_STATE_KEY as *const u8 as *const c_void);
    if val.is_null() {
        return None;
    }
    let ptr: *mut c_void = msg0(val, "pointerValue");
    if ptr.is_null() {
        None
    } else {
        Some(&mut *(ptr as *mut ImageState))
    }
}

unsafe fn make_uiimage(cg: *mut c_void, scale: f64, orientation: isize, rendering_mode: isize) -> Id {
    let obj: Id = msg0(cls("UIImage"), "new");
    if obj.is_null() {
        return NIL;
    }
    if !cg.is_null() {
        CGImageRetain(cg);
    }
    let st = Box::new(ImageState {
        cg_image: cg,
        scale: if scale > 0.0 { scale } else { 1.0 },
        orientation,
        rendering_mode,
    });
    let val: Id = msg1::<*mut c_void, Id>(cls("NSValue"), "valueWithPointer:", Box::into_raw(st) as *mut c_void);
    objc_setAssociatedObject(obj, &IMAGE_STATE_KEY as *const u8 as *const c_void, val, OBJC_ASSOCIATION_RETAIN);
    obj
}

unsafe extern "C" fn image_dealloc(self_obj: Id, _cmd: Sel) {
    let val = objc_getAssociatedObject(self_obj, &IMAGE_STATE_KEY as *const u8 as *const c_void);
    if !val.is_null() {
        let ptr: *mut c_void = msg0(val, "pointerValue");
        if !ptr.is_null() {
            let st = Box::from_raw(ptr as *mut ImageState);
            if !st.cg_image.is_null() {
                CGImageRelease(st.cg_image);
            }
        }
        objc_setAssociatedObject(self_obj, &IMAGE_STATE_KEY as *const u8 as *const c_void, NIL, OBJC_ASSOCIATION_RETAIN);
    }
}

unsafe extern "C" fn image_with_cgimage(self_cls: Class, _cmd: Sel, cg: *mut c_void) -> Id {
    let _ = self_cls;
    make_uiimage(cg, 1.0, 0, 0)
}

unsafe extern "C" fn image_with_cgimage_scale_ori(
    _self_cls: Class,
    _cmd: Sel,
    cg: *mut c_void,
    scale: f64,
    ori: isize,
) -> Id {
    make_uiimage(cg, scale, ori, 0)
}

unsafe extern "C" fn image_with_data_scale(_self_cls: Class, _cmd: Sel, data: Id, scale: f64) -> Id {
    if data.is_null() {
        return NIL;
    }
    let src = CGImageSourceCreateWithData(data, std::ptr::null_mut());
    if src.is_null() {
        return NIL;
    }
    let cg = CGImageSourceCreateImageAtIndex(src, 0, std::ptr::null_mut());
    CFRelease(src);
    if cg.is_null() {
        return NIL;
    }
    let img = make_uiimage(cg, scale, 0, 0);
    CGImageRelease(cg);
    img
}

unsafe extern "C" fn image_with_data(self_cls: Class, _cmd: Sel, data: Id) -> Id {
    image_with_data_scale(self_cls, _cmd, data, 1.0)
}

unsafe extern "C" fn image_with_file(_self_cls: Class, _cmd: Sel, path: Id) -> Id {
    if path.is_null() {
        return NIL;
    }
    let data: Id = msg1::<Id, Id>(cls("NSData"), "dataWithContentsOfFile:", path);
    if data.is_null() {
        return NIL;
    }
    let s = pear_objc::nsstring_to_string(path).unwrap_or_default();
    let scale = if s.contains("@3x") {
        3.0
    } else if s.contains("@2x") {
        2.0
    } else {
        1.0
    };
    image_with_data_scale(cls("UIImage"), _cmd, data, scale)
}

unsafe extern "C" fn image_named_in_bundle(_self_cls: Class, _cmd: Sel, name: Id, bundle: Id, _trait: Id) -> Id {
    let Some(name_str) = pear_objc::nsstring_to_string(name) else { return NIL };
    let b = if bundle.is_null() { msg0(cls("NSBundle"), "mainBundle") } else { bundle };
    let (base, ext) = match name_str.rsplit_once('.') {
        Some((ba, ex)) => (ba, vec![ex]),
        None => (name_str.as_str(), vec!["png", "jpg", "jpeg", "webp"]),
    };
    for (suffix, scale) in [("@3x", 3.0), ("@2x", 2.0), ("", 1.0)] {
        for ex in &ext {
            let res_name = pear_objc::nsstring(&format!("{base}{suffix}"));
            let res_ext = pear_objc::nsstring(ex);
            let path: Id = msg2::<Id, Id, Id>(b, "pathForResource:ofType:", res_name, res_ext);
            if !path.is_null() {
                let data: Id = msg1::<Id, Id>(cls("NSData"), "dataWithContentsOfFile:", path);
                if !data.is_null() {
                    let img = image_with_data_scale(cls("UIImage"), _cmd, data, scale);
                    if !img.is_null() {
                        return img;
                    }
                }
            }
        }
    }
    NIL
}

unsafe extern "C" fn image_named(self_cls: Class, _cmd: Sel, name: Id) -> Id {
    image_named_in_bundle(self_cls, _cmd, name, NIL, NIL)
}

unsafe extern "C" fn image_cgimage(self_obj: Id, _cmd: Sel) -> *mut c_void {
    image_state(self_obj).map(|s| s.cg_image).unwrap_or(std::ptr::null_mut())
}

unsafe extern "C" fn image_scale(self_obj: Id, _cmd: Sel) -> f64 {
    image_state(self_obj).map(|s| s.scale).unwrap_or(1.0)
}

unsafe extern "C" fn image_size(self_obj: Id, _cmd: Sel) -> CGSize {
    let Some(st) = image_state(self_obj) else { return CGSize::default() };
    if st.cg_image.is_null() {
        return CGSize::default();
    }
    CGSize {
        width: CGImageGetWidth(st.cg_image) as f64 / st.scale,
        height: CGImageGetHeight(st.cg_image) as f64 / st.scale,
    }
}

unsafe extern "C" fn image_orientation(self_obj: Id, _cmd: Sel) -> isize {
    image_state(self_obj).map(|s| s.orientation).unwrap_or(0)
}

unsafe extern "C" fn image_rendering_mode(self_obj: Id, _cmd: Sel) -> isize {
    image_state(self_obj).map(|s| s.rendering_mode).unwrap_or(0)
}

unsafe extern "C" fn image_with_rendering_mode(self_obj: Id, _cmd: Sel, mode: isize) -> Id {
    let Some(st) = image_state(self_obj) else { return self_obj };
    make_uiimage(st.cg_image, st.scale, st.orientation, mode)
}

unsafe extern "C" fn image_resizable(self_obj: Id, _cmd: Sel, _insets: pear_objc::UIEdgeInsets) -> Id {
    self_obj
}

unsafe extern "C" fn image_draw_in_rect(self_obj: Id, _cmd: Sel, rect: CGRect) {
    let Some(st) = image_state(self_obj) else { return };
    let ctx = UIGraphicsGetCurrentContext();
    if st.cg_image.is_null() || ctx.is_null() {
        return;
    }
    CGContextSaveGState(ctx);
    CGContextTranslateCTM(ctx, rect.origin.x, rect.origin.y + rect.size.height);
    CGContextScaleCTM(ctx, 1.0, -1.0);
    CGContextDrawImage(
        ctx,
        CGRect {
            origin: pear_objc::CGPoint::default(),
            size: rect.size,
        },
        st.cg_image,
    );
    CGContextRestoreGState(ctx);
}

unsafe fn encode_image(image: Id, uti: &str, props: *mut c_void) -> Id {
    let Some(st) = image_state(image) else { return NIL };
    if st.cg_image.is_null() {
        return NIL;
    }
    let data: Id = msg0(cls("NSMutableData"), "data");
    let dest = CGImageDestinationCreateWithData(data, pear_objc::nsstring(uti), 1, std::ptr::null_mut());
    if dest.is_null() {
        return NIL;
    }
    CGImageDestinationAddImage(dest, st.cg_image, props);
    let ok = CGImageDestinationFinalize(dest);
    CFRelease(dest);
    if ok {
        data
    } else {
        NIL
    }
}

#[no_mangle]
pub extern "C" fn UIImagePNGRepresentation(image: Id) -> Id {
    unsafe { encode_image(image, "public.png", std::ptr::null_mut()) }
}

#[no_mangle]
pub extern "C" fn UIImageJPEGRepresentation(image: Id, quality: f64) -> Id {
    unsafe {
        let key = pear_objc::nsstring("kCGImageDestinationLossyCompressionQuality");
        let val = pear_objc::nsnumber_f64(quality);
        let dict: Id = msg2::<Id, Id, Id>(cls("NSDictionary"), "dictionaryWithObject:forKey:", val, key);
        encode_image(image, "public.jpeg", dict)
    }
}

// ---- UIGraphics* ----
enum GraphicsEntry {
    Image { ctx: usize, scale: f64 },
    Pdf { ctx: usize, in_page: bool },
    External { ctx: usize },
}

impl GraphicsEntry {
    fn ctx(&self) -> *mut c_void {
        match *self {
            Self::Image { ctx, .. } | Self::Pdf { ctx, .. } | Self::External { ctx } => ctx as *mut c_void,
        }
    }
}

static CTX_STACK: Mutex<Vec<GraphicsEntry>> = Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn UIGraphicsGetCurrentContext() -> *mut c_void {
    CTX_STACK
        .lock()
        .unwrap()
        .last()
        .map(|e| e.ctx())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn UIGraphicsPushContext(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    unsafe { CGContextRetain(ctx) };
    CTX_STACK.lock().unwrap().push(GraphicsEntry::External { ctx: ctx as usize });
}

#[no_mangle]
pub extern "C" fn UIGraphicsPopContext() {
    if let Some(entry) = CTX_STACK.lock().unwrap().pop() {
        unsafe { CGContextRelease(entry.ctx()) };
    }
}

#[no_mangle]
pub extern "C" fn UIGraphicsBeginImageContextWithOptions(size: CGSize, opaque: bool, scale: f64) {
    let resolved_scale = if scale > 0.0 {
        scale
    } else {
        unsafe { msg0::<f64>(msg0(cls("UIScreen"), "mainScreen"), "scale") }
    };
    let width = ((size.width * resolved_scale).ceil() as usize).max(1);
    let height = ((size.height * resolved_scale).ceil() as usize).max(1);
    unsafe {
        let cs = CGColorSpaceCreateDeviceRGB();
        let alpha: u32 = if opaque { 5 } else { 1 }; // noneSkipLast : premultipliedLast
        let bitmap_info = (2u32 << 12) | alpha; // byteOrder32Big | alpha
        let ctx = CGBitmapContextCreate(std::ptr::null_mut(), width, height, 8, width * 4, cs, bitmap_info);
        CGColorSpaceRelease(cs);
        if ctx.is_null() {
            return;
        }
        CGContextTranslateCTM(ctx, 0.0, height as f64);
        CGContextScaleCTM(ctx, resolved_scale, -resolved_scale);
        CTX_STACK.lock().unwrap().push(GraphicsEntry::Image {
            ctx: ctx as usize,
            scale: resolved_scale,
        });
    }
}

#[no_mangle]
pub extern "C" fn UIGraphicsBeginImageContext(size: CGSize) {
    UIGraphicsBeginImageContextWithOptions(size, false, 1.0);
}

#[no_mangle]
pub extern "C" fn UIGraphicsGetImageFromCurrentImageContext() -> Id {
    let guard = CTX_STACK.lock().unwrap();
    let Some(GraphicsEntry::Image { ctx, scale }) = guard.last() else { return NIL };
    unsafe {
        let cg = CGBitmapContextCreateImage(*ctx as *mut c_void);
        if cg.is_null() {
            return NIL;
        }
        let img = make_uiimage(cg, *scale, 0, 0);
        CGImageRelease(cg);
        img
    }
}

#[no_mangle]
pub extern "C" fn UIGraphicsEndImageContext() {
    let mut guard = CTX_STACK.lock().unwrap();
    if matches!(guard.last(), Some(GraphicsEntry::Image { .. })) {
        if let Some(entry) = guard.pop() {
            unsafe { CGContextRelease(entry.ctx()) };
        }
    }
}

#[no_mangle]
pub extern "C" fn UIGraphicsBeginPDFContextToFile(path: Id, bounds: CGRect, doc_info: Id) -> bool {
    if path.is_null() {
        return false;
    }
    unsafe {
        let url: Id = msg1::<Id, Id>(cls("NSURL"), "fileURLWithPath:", path);
        let box_rect = if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            CGRect {
                origin: pear_objc::CGPoint::default(),
                size: CGSize { width: 612.0, height: 792.0 },
            }
        } else {
            bounds
        };
        let ctx = CGPDFContextCreateWithURL(url, &box_rect, doc_info);
        if ctx.is_null() {
            return false;
        }
        CTX_STACK.lock().unwrap().push(GraphicsEntry::Pdf {
            ctx: ctx as usize,
            in_page: false,
        });
        true
    }
}

#[no_mangle]
pub extern "C" fn UIGraphicsBeginPDFContextToData(data: Id, bounds: CGRect, doc_info: Id) {
    if data.is_null() {
        return;
    }
    unsafe {
        let consumer = CGDataConsumerCreateWithCFData(data);
        if consumer.is_null() {
            return;
        }
        let box_rect = if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            CGRect {
                origin: pear_objc::CGPoint::default(),
                size: CGSize { width: 612.0, height: 792.0 },
            }
        } else {
            bounds
        };
        let ctx = CGPDFContextCreate(consumer, &box_rect, doc_info);
        CGDataConsumerRelease(consumer);
        if !ctx.is_null() {
            CTX_STACK.lock().unwrap().push(GraphicsEntry::Pdf {
                ctx: ctx as usize,
                in_page: false,
            });
        }
    }
}

#[no_mangle]
pub extern "C" fn UIGraphicsBeginPDFPageWithInfo(_bounds: CGRect, page_info: Id) {
    let mut guard = CTX_STACK.lock().unwrap();
    let Some(GraphicsEntry::Pdf { ctx, in_page }) = guard.last_mut() else { return };
    unsafe {
        let c = *ctx as *mut c_void;
        if *in_page {
            CGPDFContextEndPage(c);
        }
        CGPDFContextBeginPage(c, page_info);
        *in_page = true;
    }
}

#[no_mangle]
pub extern "C" fn UIGraphicsBeginPDFPage() {
    UIGraphicsBeginPDFPageWithInfo(CGRect::default(), NIL);
}

#[no_mangle]
pub extern "C" fn UIGraphicsEndPDFContext() {
    let mut guard = CTX_STACK.lock().unwrap();
    if matches!(guard.last(), Some(GraphicsEntry::Pdf { .. })) {
        if let Some(GraphicsEntry::Pdf { ctx, in_page }) = guard.pop() {
            unsafe {
                let c = ctx as *mut c_void;
                if in_page {
                    CGPDFContextEndPage(c);
                }
                CGPDFContextClose(c);
                CGContextRelease(c);
            }
        }
    }
}

// ---- UIFont ----
static FONT_KEY: u8 = 0;

unsafe fn uifont_cls() -> Class {
    extern "C" {
        fn pear_uikit_uifont_class() -> Class;
    }
    pear_uikit_uifont_class()
}

unsafe fn make_uifont(nsfont: Id) -> Id {
    if nsfont.is_null() {
        return NIL;
    }
    let obj: Id = msg0(uifont_cls() as Id, "new");
    objc_setAssociatedObject(obj, &FONT_KEY as *const u8 as *const c_void, nsfont, OBJC_ASSOCIATION_RETAIN);
    obj
}

unsafe fn font_ct(obj: Id) -> *mut c_void {
    objc_getAssociatedObject(obj, &FONT_KEY as *const u8 as *const c_void)
}

unsafe extern "C" fn font_system(_cls: Class, _cmd: Sel, size: f64) -> Id {
    make_uifont(msg1::<f64, Id>(cls("NSFont"), "systemFontOfSize:", size))
}
unsafe extern "C" fn font_system_weight(_cls: Class, _cmd: Sel, size: f64, weight: f64) -> Id {
    make_uifont(msg2::<f64, f64, Id>(cls("NSFont"), "systemFontOfSize:weight:", size, weight))
}
unsafe extern "C" fn font_bold(_cls: Class, _cmd: Sel, size: f64) -> Id {
    make_uifont(msg1::<f64, Id>(cls("NSFont"), "boldSystemFontOfSize:", size))
}
unsafe extern "C" fn font_with_name(_cls: Class, _cmd: Sel, name: Id, size: f64) -> Id {
    make_uifont(msg2::<Id, f64, Id>(cls("NSFont"), "fontWithName:size:", name, size))
}
unsafe extern "C" fn font_point_size(self_obj: Id, _cmd: Sel) -> f64 {
    let ct = font_ct(self_obj);
    if ct.is_null() { 0.0 } else { CTFontGetSize(ct) }
}
unsafe extern "C" fn font_family_name(self_obj: Id, _cmd: Sel) -> Id {
    let ct = font_ct(self_obj);
    if ct.is_null() { pear_objc::nsstring("") } else { CTFontCopyFamilyName(ct) }
}
unsafe extern "C" fn font_name(self_obj: Id, _cmd: Sel) -> Id {
    let ct = font_ct(self_obj);
    if ct.is_null() { pear_objc::nsstring("") } else { CTFontCopyPostScriptName(ct) }
}
unsafe extern "C" fn font_ascender(self_obj: Id, _cmd: Sel) -> f64 {
    let ct = font_ct(self_obj);
    if ct.is_null() { 0.0 } else { CTFontGetAscent(ct) }
}
unsafe extern "C" fn font_descender(self_obj: Id, _cmd: Sel) -> f64 {
    let ct = font_ct(self_obj);
    if ct.is_null() { 0.0 } else { -CTFontGetDescent(ct) }
}
unsafe extern "C" fn font_cap_height(self_obj: Id, _cmd: Sel) -> f64 {
    let ct = font_ct(self_obj);
    if ct.is_null() { 0.0 } else { CTFontGetCapHeight(ct) }
}
unsafe extern "C" fn font_x_height(self_obj: Id, _cmd: Sel) -> f64 {
    let ct = font_ct(self_obj);
    if ct.is_null() { 0.0 } else { CTFontGetXHeight(ct) }
}
unsafe extern "C" fn font_leading(self_obj: Id, _cmd: Sel) -> f64 {
    let ct = font_ct(self_obj);
    if ct.is_null() { 0.0 } else { CTFontGetLeading(ct) }
}
unsafe extern "C" fn font_line_height(self_obj: Id, _cmd: Sel) -> f64 {
    let ct = font_ct(self_obj);
    if ct.is_null() { 0.0 } else { CTFontGetAscent(ct) + CTFontGetDescent(ct) + CTFontGetLeading(ct) }
}
unsafe extern "C" fn font_with_size(self_obj: Id, _cmd: Sel, size: f64) -> Id {
    let ct = font_ct(self_obj);
    if ct.is_null() {
        return self_obj;
    }
    let copy = CTFontCreateCopyWithAttributes(ct, size, std::ptr::null(), std::ptr::null_mut());
    let f = make_uifont(copy);
    if !copy.is_null() {
        CFRelease(copy);
    }
    f
}

// ---- UILabel ----
static LABEL_TEXT_KEY: u8 = 0;
static LABEL_FONT_KEY: u8 = 0;
static LABEL_COLOR_KEY: u8 = 0;

unsafe extern "C" fn label_text(self_obj: Id, _cmd: Sel) -> Id {
    objc_getAssociatedObject(self_obj, &LABEL_TEXT_KEY as *const u8 as *const c_void)
}
unsafe extern "C" fn label_set_text(self_obj: Id, _cmd: Sel, text: Id) {
    objc_setAssociatedObject(self_obj, &LABEL_TEXT_KEY as *const u8 as *const c_void, text, OBJC_ASSOCIATION_RETAIN);
    msg0::<()>(self_obj, "setNeedsDisplay");
}
unsafe extern "C" fn label_font(self_obj: Id, _cmd: Sel) -> Id {
    let f = objc_getAssociatedObject(self_obj, &LABEL_FONT_KEY as *const u8 as *const c_void);
    if f.is_null() {
        msg1::<f64, Id>(uifont_cls() as Id, "systemFontOfSize:", 17.0)
    } else {
        f
    }
}
unsafe extern "C" fn label_set_font(self_obj: Id, _cmd: Sel, font: Id) {
    objc_setAssociatedObject(self_obj, &LABEL_FONT_KEY as *const u8 as *const c_void, font, OBJC_ASSOCIATION_RETAIN);
    msg0::<()>(self_obj, "setNeedsDisplay");
}
unsafe extern "C" fn label_text_color(self_obj: Id, _cmd: Sel) -> Id {
    let c = objc_getAssociatedObject(self_obj, &LABEL_COLOR_KEY as *const u8 as *const c_void);
    if c.is_null() {
        msg0(cls("UIColor"), "whiteColor")
    } else {
        c
    }
}
unsafe extern "C" fn label_set_text_color(self_obj: Id, _cmd: Sel, color: Id) {
    objc_setAssociatedObject(self_obj, &LABEL_COLOR_KEY as *const u8 as *const c_void, color, OBJC_ASSOCIATION_RETAIN);
    msg0::<()>(self_obj, "setNeedsDisplay");
}

unsafe fn label_build_attr_string(self_obj: Id) -> Id {
    let text = label_text(self_obj, pear_objc::sel("text"));
    if text.is_null() || msg0::<usize>(text, "length") == 0 {
        return NIL;
    }
    let font = label_font(self_obj, pear_objc::sel("font"));
    let ct = font_ct(font);
    let color = label_text_color(self_obj, pear_objc::sel("textColor"));
    let cg_color: *mut c_void = msg0(color, "CGColor");
    let dict: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    if !ct.is_null() {
        msg2::<Id, Id, ()>(dict, "setObject:forKey:", ct, pear_objc::nsstring("NSFont"));
    }
    if !cg_color.is_null() {
        msg2::<Id, Id, ()>(dict, "setObject:forKey:", cg_color, pear_objc::nsstring("CTForegroundColor"));
    }
    let alloc: Id = msg0(cls("NSAttributedString"), "alloc");
    msg2::<Id, Id, Id>(alloc, "initWithString:attributes:", text, dict)
}

unsafe extern "C" fn label_size_that_fits(self_obj: Id, _cmd: Sel, size: CGSize) -> CGSize {
    let attr = label_build_attr_string(self_obj);
    if attr.is_null() {
        return CGSize::default();
    }
    let fs = CTFramesetterCreateWithAttributedString(attr);
    let constraint = CGSize {
        width: if size.width > 0.0 { size.width } else { f64::MAX },
        height: f64::MAX,
    };
    let res = CTFramesetterSuggestFrameSizeWithConstraints(fs, CFRange::default(), std::ptr::null_mut(), constraint, std::ptr::null_mut());
    CFRelease(fs);
    res
}

unsafe extern "C" fn label_draw_rect(self_obj: Id, _cmd: Sel, _rect: CGRect) {
    let attr = label_build_attr_string(self_obj);
    let ctx = UIGraphicsGetCurrentContext();
    if attr.is_null() || ctx.is_null() {
        return;
    }
    let bounds: CGRect = msg0(self_obj, "bounds");
    CGContextSaveGState(ctx);
    CGContextSetTextMatrix(ctx, CGAffineTransform { a: 1.0, d: 1.0, ..Default::default() });
    CGContextTranslateCTM(ctx, 0.0, bounds.size.height);
    CGContextScaleCTM(ctx, 1.0, -1.0);
    let path = CGPathCreateWithRect(bounds, std::ptr::null());
    let fs = CTFramesetterCreateWithAttributedString(attr);
    let frame = CTFramesetterCreateFrame(fs, CFRange::default(), path, std::ptr::null_mut());
    CTFrameDraw(frame, ctx);
    CFRelease(frame);
    CFRelease(fs);
    CGPathRelease(path);
    CGContextRestoreGState(ctx);
}

// ---- UIProgressView ----
static PROG_VAL_KEY: u8 = 0;

unsafe extern "C" fn prog_value(self_obj: Id, _cmd: Sel) -> c_float {
    let v = objc_getAssociatedObject(self_obj, &PROG_VAL_KEY as *const u8 as *const c_void);
    if v.is_null() { 0.0 } else { msg0(v, "floatValue") }
}
unsafe extern "C" fn prog_set_value(self_obj: Id, _cmd: Sel, p: c_float) {
    let clamped = p.clamp(0.0, 1.0);
    let num: Id = msg1::<f32, Id>(cls("NSNumber"), "numberWithFloat:", clamped);
    objc_setAssociatedObject(self_obj, &PROG_VAL_KEY as *const u8 as *const c_void, num, OBJC_ASSOCIATION_RETAIN);
    msg0::<()>(self_obj, "setNeedsDisplay");
}
unsafe extern "C" fn prog_set_animated(self_obj: Id, _cmd: Sel, p: c_float, _anim: bool) {
    prog_set_value(self_obj, _cmd, p);
}

pub unsafe fn register() {
    let col = cls("UIColor");
    if !col.is_null() {
        let meta = object_getClass(col);
        add_method(meta, "colorWithRed:green:blue:alpha:", color_with_rgba as Imp, "@@:dddd");
        add_method(meta, "colorWithWhite:alpha:", color_with_white as Imp, "@@:dd");
        add_method(meta, "clearColor", color_clear as Imp, "@@:");
        add_method(meta, "blackColor", color_black as Imp, "@@:");
        add_method(meta, "whiteColor", color_white as Imp, "@@:");
        add_method(col, "initWithRed:green:blue:alpha:", color_init_rgba as Imp, "@@:dddd");
        add_method(col, "CGColor", color_cgcolor as Imp, "^{CGColor=}@:");
        add_method(col, "dealloc", color_dealloc as Imp, "v@:");
        add_method(col, "copyWithZone:", copy_self as Imp, "@@:^{_NSZone=}");
    }

    let img = cls("UIImage");
    if !img.is_null() {
        let meta = object_getClass(img);
        add_method(meta, "imageWithCGImage:", image_with_cgimage as Imp, "@@:^{CGImage=}");
        add_method(meta, "imageWithCGImage:scale:orientation:", image_with_cgimage_scale_ori as Imp, "@@:^{CGImage=}dq");
        add_method(meta, "imageWithData:", image_with_data as Imp, "@@:@");
        add_method(meta, "imageWithData:scale:", image_with_data_scale as Imp, "@@:@d");
        add_method(meta, "imageWithContentsOfFile:", image_with_file as Imp, "@@:@");
        add_method(meta, "imageNamed:", image_named as Imp, "@@:@");
        add_method(meta, "imageNamed:inBundle:compatibleWithTraitCollection:", image_named_in_bundle as Imp, "@@:@@@");
        add_method(img, "dealloc", image_dealloc as Imp, "v@:");
        add_method(img, "CGImage", image_cgimage as Imp, "^{CGImage=}@:");
        add_method(img, "scale", image_scale as Imp, "d@:");
        add_method(img, "size", image_size as Imp, "{CGSize=dd}@:");
        add_method(img, "imageOrientation", image_orientation as Imp, "q@:");
        add_method(img, "renderingMode", image_rendering_mode as Imp, "q@:");
        add_method(img, "imageWithRenderingMode:", image_with_rendering_mode as Imp, "@@:q");
        add_method(img, "resizableImageWithCapInsets:", image_resizable as Imp, "@@:{UIEdgeInsets=dddd}");
        add_method(img, "drawInRect:", image_draw_in_rect as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
    }

    let fnt = uifont_cls();
    if !fnt.is_null() {
        let meta = object_getClass(fnt);
        add_method(meta, "systemFontOfSize:", font_system as Imp, "@@:d");
        add_method(meta, "systemFontOfSize:weight:", font_system_weight as Imp, "@@:dd");
        add_method(meta, "boldSystemFontOfSize:", font_bold as Imp, "@@:d");
        add_method(meta, "fontWithName:size:", font_with_name as Imp, "@@:@d");
        add_method(fnt, "pointSize", font_point_size as Imp, "d@:");
        add_method(fnt, "familyName", font_family_name as Imp, "@@:");
        add_method(fnt, "fontName", font_name as Imp, "@@:");
        add_method(fnt, "ascender", font_ascender as Imp, "d@:");
        add_method(fnt, "descender", font_descender as Imp, "d@:");
        add_method(fnt, "capHeight", font_cap_height as Imp, "d@:");
        add_method(fnt, "xHeight", font_x_height as Imp, "d@:");
        add_method(fnt, "leading", font_leading as Imp, "d@:");
        add_method(fnt, "lineHeight", font_line_height as Imp, "d@:");
        add_method(fnt, "fontWithSize:", font_with_size as Imp, "@@:d");
        add_method(fnt, "copyWithZone:", copy_self as Imp, "@@:^{_NSZone=}");
    }

    let lbl = cls("UILabel");
    if !lbl.is_null() {
        add_method(lbl, "text", label_text as Imp, "@@:");
        add_method(lbl, "setText:", label_set_text as Imp, "v@:@");
        add_method(lbl, "font", label_font as Imp, "@@:");
        add_method(lbl, "setFont:", label_set_font as Imp, "v@:@");
        add_method(lbl, "textColor", label_text_color as Imp, "@@:");
        add_method(lbl, "setTextColor:", label_set_text_color as Imp, "v@:@");
        add_method(lbl, "sizeThatFits:", label_size_that_fits as Imp, "{CGSize=dd}@:{CGSize=dd}");
        add_method(lbl, "drawRect:", label_draw_rect as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
    }

    let prog = cls("UIProgressView");
    if !prog.is_null() {
        add_method(prog, "progress", prog_value as Imp, "f@:");
        add_method(prog, "setProgress:", prog_set_value as Imp, "v@:f");
        add_method(prog, "setProgress:animated:", prog_set_animated as Imp, "v@:fB");
    }
}
