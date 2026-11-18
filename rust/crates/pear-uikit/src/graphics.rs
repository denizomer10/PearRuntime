//! UIColor, UIImage, UIImagePNGRepresentation/JPEGRepresentation, UIGraphics*, UIFont, UILabel, UIProgressView.

use pear_objc::{
    assoc_get, assoc_i64, assoc_set, assoc_set_i64, autoreleased, cls, is_kind_of, msg0, msg1,
    msg2, msg_super0, msg_super1, nsstring, object_getClass, objc_getAssociatedObject,
    objc_release, objc_retain, objc_setAssociatedObject, replace_method as add_method,
    responds_to, sel, super_dealloc, CGPoint, CGRect, CGSize, Class, Id, Imp, Sel, NIL,
    OBJC_ASSOCIATION_RETAIN,
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
    fn CGContextSetFillColorWithColor(ctx: *mut c_void, color: *mut c_void);
    fn CGContextFillRect(ctx: *mut c_void, rect: CGRect);
    fn CGContextClearRect(ctx: *mut c_void, rect: CGRect);
    fn CGContextSetTextPosition(ctx: *mut c_void, x: f64, y: f64);
}

extern "C" {
    fn class_getMethodImplementation(cls: Class, name: Sel) -> Imp;
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
    fn CTFrameGetLines(frame: *mut c_void) -> Id;
    fn CTFrameGetLineOrigins(frame: *mut c_void, range: CFRange, origins: *mut CGPoint);
    fn CTLineDraw(line: *mut c_void, ctx: *mut c_void);
    fn CTLineGetStringRange(line: *mut c_void) -> CFRange;
    fn CTParagraphStyleCreate(settings: *const CTParagraphStyleSetting, count: usize) -> *mut c_void;
    fn CTFontCreateWithFontDescriptor(desc: *mut c_void, size: f64, matrix: *const c_void) -> *mut c_void;
    fn CTFontCreateCopyWithSymbolicTraits(
        font: *mut c_void,
        size: f64,
        matrix: *const c_void,
        value: u32,
        mask: u32,
    ) -> *mut c_void;
    fn CTFontManagerCopyAvailableFontFamilyNames() -> Id;
}

#[repr(C)]
struct CTParagraphStyleSetting {
    spec: u32,
    value_size: usize,
    value: *const c_void,
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
    autoreleased(f(alloc, pear_objc::sel("initWithRed:green:blue:alpha:"), r, g, b, a))
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
    super_dealloc(self_obj, "UIColor");
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

/// Attaches image state to an initialized UIImage (takes its own CGImage reference).
unsafe fn attach_image(obj: Id, cg: *mut c_void, scale: f64, orientation: isize, rendering_mode: isize) -> Id {
    if obj.is_null() {
        return NIL;
    }
    if image_state(obj).is_some() {
        free_image_state(obj);
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

/// New autoreleased UIImage (of `class`, or UIImage) around `cg`.
unsafe fn make_uiimage_of(class: Class, cg: *mut c_void, scale: f64, orientation: isize, rendering_mode: isize) -> Id {
    let c = if class.is_null() { cls("UIImage") } else { class };
    let obj: Id = msg0(msg0::<Id>(c, "alloc"), "init");
    autoreleased(attach_image(obj, cg, scale, orientation, rendering_mode))
}

unsafe fn make_uiimage(cg: *mut c_void, scale: f64, orientation: isize, rendering_mode: isize) -> Id {
    make_uiimage_of(NIL, cg, scale, orientation, rendering_mode)
}

/// Decodes image data into a +1 CGImage (null on failure).
unsafe fn cgimage_from_data(data: Id) -> *mut c_void {
    if data.is_null() {
        return std::ptr::null_mut();
    }
    let src = CGImageSourceCreateWithData(data, std::ptr::null_mut());
    if src.is_null() {
        return std::ptr::null_mut();
    }
    let cg = CGImageSourceCreateImageAtIndex(src, 0, std::ptr::null_mut());
    CFRelease(src);
    cg
}

fn scale_for_path(path: &str) -> f64 {
    let stem = path.rsplit('/').next().unwrap_or(path);
    let stem = stem.rsplit_once('.').map(|(a, _)| a).unwrap_or(stem);
    if stem.ends_with("@3x") || stem.contains("@3x~") {
        3.0
    } else if stem.ends_with("@2x") || stem.contains("@2x~") {
        2.0
    } else {
        1.0
    }
}

unsafe fn free_image_state(self_obj: Id) {
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

unsafe extern "C" fn image_dealloc(self_obj: Id, _cmd: Sel) {
    free_image_state(self_obj);
    super_dealloc(self_obj, "UIImage");
}

unsafe extern "C" fn image_with_cgimage(self_cls: Class, _cmd: Sel, cg: *mut c_void) -> Id {
    make_uiimage_of(self_cls, cg, 1.0, 0, 0)
}

unsafe extern "C" fn image_with_cgimage_scale_ori(
    self_cls: Class,
    _cmd: Sel,
    cg: *mut c_void,
    scale: f64,
    ori: isize,
) -> Id {
    make_uiimage_of(self_cls, cg, scale, ori, 0)
}

unsafe extern "C" fn image_with_data_scale(self_cls: Class, _cmd: Sel, data: Id, scale: f64) -> Id {
    let cg = cgimage_from_data(data);
    if cg.is_null() {
        return NIL;
    }
    let img = make_uiimage_of(self_cls, cg, scale, 0, 0);
    CGImageRelease(cg);
    img
}

unsafe extern "C" fn image_with_data(self_cls: Class, _cmd: Sel, data: Id) -> Id {
    image_with_data_scale(self_cls, _cmd, data, 1.0)
}

unsafe extern "C" fn image_with_file(self_cls: Class, _cmd: Sel, path: Id) -> Id {
    if path.is_null() {
        return NIL;
    }
    let data: Id = msg1::<Id, Id>(cls("NSData"), "dataWithContentsOfFile:", path);
    if data.is_null() {
        return NIL;
    }
    let s = pear_objc::nsstring_to_string(path).unwrap_or_default();
    image_with_data_scale(self_cls, _cmd, data, scale_for_path(&s))
}

// ---- UIImage initializers (alloc/init family: +1 result, self consumed) ----
unsafe fn image_init_or_fail(self_obj: Id, cg: *mut c_void, scale: f64, ori: isize) -> Id {
    if cg.is_null() {
        objc_release(self_obj);
        return NIL;
    }
    let obj: Id = msg_super0(self_obj, "UIImage", "init");
    attach_image(obj, cg, scale, ori, 0)
}

unsafe extern "C" fn image_init(self_obj: Id, _cmd: Sel) -> Id {
    msg_super0(self_obj, "UIImage", "init")
}

unsafe extern "C" fn image_init_cgimage(self_obj: Id, _cmd: Sel, cg: *mut c_void) -> Id {
    image_init_or_fail(self_obj, cg, 1.0, 0)
}

unsafe extern "C" fn image_init_cgimage_scale_ori(self_obj: Id, _cmd: Sel, cg: *mut c_void, scale: f64, ori: isize) -> Id {
    image_init_or_fail(self_obj, cg, scale, ori)
}

unsafe extern "C" fn image_init_data_scale(self_obj: Id, _cmd: Sel, data: Id, scale: f64) -> Id {
    let cg = cgimage_from_data(data);
    let obj = image_init_or_fail(self_obj, cg, scale, 0);
    if !cg.is_null() {
        CGImageRelease(cg);
    }
    obj
}

unsafe extern "C" fn image_init_data(self_obj: Id, _cmd: Sel, data: Id) -> Id {
    image_init_data_scale(self_obj, _cmd, data, 1.0)
}

unsafe extern "C" fn image_init_file(self_obj: Id, _cmd: Sel, path: Id) -> Id {
    let data: Id = if path.is_null() { NIL } else { msg1(cls("NSData"), "dataWithContentsOfFile:", path) };
    let s = pear_objc::nsstring_to_string(path).unwrap_or_default();
    image_init_data_scale(self_obj, _cmd, data, scale_for_path(&s))
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
                    let img = image_with_data_scale(_self_cls, _cmd, data, scale);
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
    make_uiimage_of(object_getClass(self_obj), st.cg_image, st.scale, st.orientation, mode)
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
    autoreleased(obj)
}

/// Wraps a +1 CTFont/NSFont, consuming that reference.
unsafe fn make_uifont_owned(ct: *mut c_void) -> Id {
    if ct.is_null() {
        return NIL;
    }
    let f = make_uifont(ct);
    CFRelease(ct);
    f
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
    if ct.is_null() { pear_objc::nsstring("") } else { autoreleased(CTFontCopyFamilyName(ct)) }
}
unsafe extern "C" fn font_name(self_obj: Id, _cmd: Sel) -> Id {
    let ct = font_ct(self_obj);
    if ct.is_null() { pear_objc::nsstring("") } else { autoreleased(CTFontCopyPostScriptName(ct)) }
}
unsafe extern "C" fn font_italic(_cls: Class, _cmd: Sel, size: f64) -> Id {
    const K_CT_FONT_ITALIC_TRAIT: u32 = 1 << 0;
    let base: Id = msg1::<f64, Id>(cls("NSFont"), "systemFontOfSize:", size);
    if base.is_null() {
        return NIL;
    }
    let italic = CTFontCreateCopyWithSymbolicTraits(base, size, std::ptr::null(), K_CT_FONT_ITALIC_TRAIT, K_CT_FONT_ITALIC_TRAIT);
    if italic.is_null() {
        make_uifont(base)
    } else {
        make_uifont_owned(italic)
    }
}
unsafe extern "C" fn font_with_descriptor(_cls: Class, _cmd: Sel, desc: Id, size: f64) -> Id {
    if !desc.is_null() {
        // NSFontDescriptor and CTFontDescriptor are toll-free bridged.
        let ct = CTFontCreateWithFontDescriptor(desc, size, std::ptr::null());
        if !ct.is_null() {
            return make_uifont_owned(ct);
        }
    }
    font_system(_cls, _cmd, size)
}
unsafe extern "C" fn font_family_names(_cls: Class, _cmd: Sel) -> Id {
    let names = CTFontManagerCopyAvailableFontFamilyNames();
    if names.is_null() {
        msg0(cls("NSArray"), "array")
    } else {
        autoreleased(names)
    }
}
unsafe extern "C" fn font_names_for_family(_cls: Class, _cmd: Sel, family: Id) -> Id {
    let out: Id = msg0(cls("NSMutableArray"), "array");
    if family.is_null() {
        return out;
    }
    let mgr: Id = msg0(cls("NSFontManager"), "sharedFontManager");
    let members: Id = msg1(mgr, "availableMembersOfFontFamily:", family);
    for i in 0..pear_objc::nsarray_count(members) {
        let m = pear_objc::nsarray_object_at(members, i);
        if pear_objc::nsarray_count(m) > 0 {
            msg1::<Id, ()>(out, "addObject:", pear_objc::nsarray_object_at(m, 0));
        }
    }
    out
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
    make_uifont_owned(CTFontCreateCopyWithAttributes(ct, size, std::ptr::null(), std::ptr::null_mut()))
}

// ---- UILabel ----
static LABEL_TEXT_KEY: u8 = 0;
static LABEL_ATTR_KEY: u8 = 0;
static LABEL_FONT_KEY: u8 = 0;
static LABEL_COLOR_KEY: u8 = 0;
static LABEL_ALIGN_KEY: u8 = 0;
static LABEL_LINES_KEY: u8 = 0;
static LABEL_BREAK_KEY: u8 = 0;

const NS_LINE_BREAK_BY_TRUNCATING_TAIL: i64 = 4;

unsafe fn label_redisplay(self_obj: Id) {
    msg0::<()>(self_obj, "setNeedsDisplay");
}

unsafe fn label_setup(obj: Id) {
    msg1::<bool, ()>(obj, "setOpaque:", false);
    let layer: Id = msg0(obj, "layer");
    if !layer.is_null() {
        // UILabel redraws when resized (contentMode redraw).
        msg1::<bool, ()>(layer, "setNeedsDisplayOnBoundsChange:", true);
    }
}

unsafe extern "C" fn label_init_with_frame(self_obj: Id, _cmd: Sel, frame: CGRect) -> Id {
    let obj: Id = msg_super1(self_obj, "UILabel", "initWithFrame:", frame);
    if !obj.is_null() {
        label_setup(obj);
    }
    obj
}

unsafe extern "C" fn label_init_with_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = msg_super1(self_obj, "UILabel", "initWithCoder:", coder);
    if obj.is_null() {
        return NIL;
    }
    label_setup(obj);
    if !coder.is_null() {
        let has = |k: &str| msg1::<Id, bool>(coder, "containsValueForKey:", nsstring(k));
        let text: Id = msg1(coder, "decodeObjectForKey:", nsstring("UIText"));
        if !text.is_null() && is_kind_of(text, "NSString") {
            assoc_set(obj, &LABEL_TEXT_KEY, text);
        }
        if has("UINumberOfLines") {
            assoc_set_i64(obj, &LABEL_LINES_KEY, msg1::<Id, isize>(coder, "decodeIntegerForKey:", nsstring("UINumberOfLines")) as i64);
        }
        if has("UITextAlignment") {
            assoc_set_i64(obj, &LABEL_ALIGN_KEY, msg1::<Id, isize>(coder, "decodeIntegerForKey:", nsstring("UITextAlignment")) as i64);
        }
        if has("UILineBreakMode") {
            assoc_set_i64(obj, &LABEL_BREAK_KEY, msg1::<Id, isize>(coder, "decodeIntegerForKey:", nsstring("UILineBreakMode")) as i64);
        }
        let color: Id = msg1(coder, "decodeObjectForKey:", nsstring("UITextColor"));
        if !color.is_null() && is_kind_of(color, "UIColor") {
            assoc_set(obj, &LABEL_COLOR_KEY, color);
        }
    }
    label_redisplay(obj);
    obj
}

unsafe extern "C" fn label_text(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &LABEL_TEXT_KEY)
}
unsafe extern "C" fn label_set_text(self_obj: Id, _cmd: Sel, text: Id) {
    let copy: Id = if text.is_null() { NIL } else { msg0(text, "copy") };
    assoc_set(self_obj, &LABEL_TEXT_KEY, copy);
    if !copy.is_null() {
        objc_release(copy);
    }
    assoc_set(self_obj, &LABEL_ATTR_KEY, NIL);
    label_redisplay(self_obj);
}
unsafe extern "C" fn label_attributed_text(self_obj: Id, _cmd: Sel) -> Id {
    let attr = assoc_get(self_obj, &LABEL_ATTR_KEY);
    if !attr.is_null() {
        return attr;
    }
    let text = assoc_get(self_obj, &LABEL_TEXT_KEY);
    if text.is_null() {
        return NIL;
    }
    autoreleased(msg1(msg0::<Id>(cls("NSAttributedString"), "alloc"), "initWithString:", text))
}
unsafe extern "C" fn label_set_attributed_text(self_obj: Id, _cmd: Sel, attr: Id) {
    let copy: Id = if attr.is_null() { NIL } else { msg0(attr, "copy") };
    assoc_set(self_obj, &LABEL_ATTR_KEY, copy);
    if !copy.is_null() {
        assoc_set(self_obj, &LABEL_TEXT_KEY, msg0(copy, "string"));
        objc_release(copy);
    } else {
        assoc_set(self_obj, &LABEL_TEXT_KEY, NIL);
    }
    label_redisplay(self_obj);
}
unsafe extern "C" fn label_font(self_obj: Id, _cmd: Sel) -> Id {
    let f = assoc_get(self_obj, &LABEL_FONT_KEY);
    if f.is_null() {
        msg1::<f64, Id>(uifont_cls() as Id, "systemFontOfSize:", 17.0)
    } else {
        f
    }
}
unsafe extern "C" fn label_set_font(self_obj: Id, _cmd: Sel, font: Id) {
    assoc_set(self_obj, &LABEL_FONT_KEY, font);
    label_redisplay(self_obj);
}
unsafe extern "C" fn label_text_color(self_obj: Id, _cmd: Sel) -> Id {
    let c = assoc_get(self_obj, &LABEL_COLOR_KEY);
    if c.is_null() {
        msg0(cls("UIColor"), "whiteColor")
    } else {
        c
    }
}
unsafe extern "C" fn label_set_text_color(self_obj: Id, _cmd: Sel, color: Id) {
    assoc_set(self_obj, &LABEL_COLOR_KEY, color);
    label_redisplay(self_obj);
}
unsafe extern "C" fn label_text_alignment(self_obj: Id, _cmd: Sel) -> isize {
    assoc_i64(self_obj, &LABEL_ALIGN_KEY, 0) as isize
}
unsafe extern "C" fn label_set_text_alignment(self_obj: Id, _cmd: Sel, v: isize) {
    assoc_set_i64(self_obj, &LABEL_ALIGN_KEY, v as i64);
    label_redisplay(self_obj);
}
unsafe extern "C" fn label_number_of_lines(self_obj: Id, _cmd: Sel) -> isize {
    assoc_i64(self_obj, &LABEL_LINES_KEY, 1) as isize
}
unsafe extern "C" fn label_set_number_of_lines(self_obj: Id, _cmd: Sel, v: isize) {
    assoc_set_i64(self_obj, &LABEL_LINES_KEY, v.max(0) as i64);
    label_redisplay(self_obj);
}
unsafe extern "C" fn label_line_break_mode(self_obj: Id, _cmd: Sel) -> isize {
    assoc_i64(self_obj, &LABEL_BREAK_KEY, NS_LINE_BREAK_BY_TRUNCATING_TAIL) as isize
}
unsafe extern "C" fn label_set_line_break_mode(self_obj: Id, _cmd: Sel, v: isize) {
    assoc_set_i64(self_obj, &LABEL_BREAK_KEY, v as i64);
    label_redisplay(self_obj);
}

/// UIKit NSTextAlignment (left, center, right, justified, natural) to CTTextAlignment.
fn ct_alignment(ui: i64) -> u8 {
    match ui {
        1 => 2, // center
        2 => 1, // right
        3 => 3, // justified
        4 => 4, // natural
        _ => 0, // left
    }
}

/// Paragraph style for the label's alignment and line break mode (+1).
unsafe fn label_paragraph_style(self_obj: Id) -> *mut c_void {
    const K_ALIGNMENT: u32 = 0;
    const K_LINE_BREAK_MODE: u32 = 6;
    let align = ct_alignment(assoc_i64(self_obj, &LABEL_ALIGN_KEY, 0));
    let brk = assoc_i64(self_obj, &LABEL_BREAK_KEY, NS_LINE_BREAK_BY_TRUNCATING_TAIL).clamp(0, 5) as u8;
    let settings = [
        CTParagraphStyleSetting { spec: K_ALIGNMENT, value_size: 1, value: &align as *const u8 as *const c_void },
        CTParagraphStyleSetting { spec: K_LINE_BREAK_MODE, value_size: 1, value: &brk as *const u8 as *const c_void },
    ];
    CTParagraphStyleCreate(settings.as_ptr(), settings.len())
}

/// Converts UIKit attribute values (UIFont, UIColor) to their CoreText forms.
unsafe fn ct_attributes(attrs: Id, default_font: *mut c_void, default_color: *mut c_void, para: *mut c_void) -> Id {
    let dict: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    if !attrs.is_null() {
        msg1::<Id, ()>(dict, "addEntriesFromDictionary:", attrs);
    }
    let font_key = nsstring("NSFont");
    let fg_key = nsstring("NSColor");
    let ct_fg_key = nsstring("CTForegroundColor");
    let font: Id = msg1(dict, "objectForKey:", font_key);
    if !font.is_null() && is_kind_of(font, "UIFont") {
        let ct = font_ct(font);
        if ct.is_null() {
            msg1::<Id, ()>(dict, "removeObjectForKey:", font_key);
        } else {
            msg2::<Id, Id, ()>(dict, "setObject:forKey:", ct, font_key);
        }
    }
    if msg1::<Id, Id>(dict, "objectForKey:", font_key).is_null() && !default_font.is_null() {
        msg2::<Id, Id, ()>(dict, "setObject:forKey:", default_font, font_key);
    }
    let fg: Id = msg1(dict, "objectForKey:", fg_key);
    if !fg.is_null() && is_kind_of(fg, "UIColor") {
        let cg: *mut c_void = msg0(fg, "CGColor");
        msg1::<Id, ()>(dict, "removeObjectForKey:", fg_key);
        if !cg.is_null() {
            msg2::<Id, Id, ()>(dict, "setObject:forKey:", cg, ct_fg_key);
        }
    }
    if msg1::<Id, Id>(dict, "objectForKey:", ct_fg_key).is_null()
        && msg1::<Id, Id>(dict, "objectForKey:", fg_key).is_null()
        && !default_color.is_null()
    {
        msg2::<Id, Id, ()>(dict, "setObject:forKey:", default_color, ct_fg_key);
    }
    if !para.is_null() && msg1::<Id, Id>(dict, "objectForKey:", nsstring("NSParagraphStyle")).is_null() {
        msg2::<Id, Id, ()>(dict, "setObject:forKey:", para, nsstring("NSParagraphStyle"));
    }
    dict
}

/// CoreText-ready attributed string for the label (autoreleased), or nil.
unsafe fn label_build_attr_string(self_obj: Id) -> Id {
    let text = assoc_get(self_obj, &LABEL_TEXT_KEY);
    if text.is_null() || msg0::<usize>(text, "length") == 0 {
        return NIL;
    }
    let font = label_font(self_obj, sel("font"));
    let ct = font_ct(font);
    let color = label_text_color(self_obj, sel("textColor"));
    let cg_color: *mut c_void = msg0(color, "CGColor");
    let para = label_paragraph_style(self_obj);
    let attributed = assoc_get(self_obj, &LABEL_ATTR_KEY);
    let out: Id;
    if attributed.is_null() {
        let attrs = ct_attributes(NIL, ct, cg_color, para);
        out = msg2(msg0::<Id>(cls("NSAttributedString"), "alloc"), "initWithString:attributes:", text, attrs);
    } else {
        // Walk the runs and translate UIKit attribute values for CoreText.
        out = msg1(msg0::<Id>(cls("NSMutableAttributedString"), "alloc"), "initWithString:", text);
        let len: usize = msg0(attributed, "length");
        let mut i = 0usize;
        while i < len {
            let mut range = pear_objc::NSRange::default();
            let attrs: Id = msg2(attributed, "attributesAtIndex:effectiveRange:", i, &mut range as *mut pear_objc::NSRange);
            if range.length == 0 {
                break;
            }
            let converted = ct_attributes(attrs, ct, cg_color, para);
            msg2::<Id, pear_objc::NSRange, ()>(out, "setAttributes:range:", converted, range);
            i = range.location + range.length;
        }
    }
    if !para.is_null() {
        CFRelease(para);
    }
    autoreleased(out)
}

/// Lines CoreText lays out for `attr` in a box of `width` (+1 frame, or null).
unsafe fn label_frame(attr: Id, width: f64, height: f64) -> (*mut c_void, *mut c_void) {
    let fs = CTFramesetterCreateWithAttributedString(attr);
    if fs.is_null() {
        return (std::ptr::null_mut(), std::ptr::null_mut());
    }
    let path = CGPathCreateWithRect(
        CGRect { origin: CGPoint::default(), size: CGSize { width, height } },
        std::ptr::null(),
    );
    let frame = CTFramesetterCreateFrame(fs, CFRange::default(), path, std::ptr::null_mut());
    CGPathRelease(path);
    (fs, frame)
}

/// Size of the label's text, honouring numberOfLines.
unsafe fn label_text_size(self_obj: Id, attr: Id, width: f64) -> CGSize {
    const BIG: f64 = 1.0e7;
    let w = if width > 0.0 { width } else { BIG };
    let lines_wanted = assoc_i64(self_obj, &LABEL_LINES_KEY, 1);
    let (fs, frame) = label_frame(attr, w, BIG);
    if fs.is_null() {
        return CGSize::default();
    }
    let mut range = CFRange::default();
    if lines_wanted > 0 && !frame.is_null() {
        let lines = CTFrameGetLines(frame);
        let n = pear_objc::nsarray_count(lines);
        if n as i64 > lines_wanted {
            let last = pear_objc::nsarray_object_at(lines, lines_wanted as usize - 1);
            let r = CTLineGetStringRange(last);
            range = CFRange { location: 0, length: r.location + r.length };
        }
    }
    let size = CTFramesetterSuggestFrameSizeWithConstraints(
        fs,
        range,
        std::ptr::null_mut(),
        CGSize { width: w, height: BIG },
        std::ptr::null_mut(),
    );
    if !frame.is_null() {
        CFRelease(frame);
    }
    CFRelease(fs);
    CGSize { width: size.width.ceil(), height: size.height.ceil() }
}

unsafe extern "C" fn label_size_that_fits(self_obj: Id, _cmd: Sel, size: CGSize) -> CGSize {
    let attr = label_build_attr_string(self_obj);
    if attr.is_null() {
        return CGSize::default();
    }
    label_text_size(self_obj, attr, size.width)
}

unsafe extern "C" fn label_intrinsic_size(self_obj: Id, _cmd: Sel) -> CGSize {
    label_size_that_fits(self_obj, _cmd, CGSize::default())
}

unsafe extern "C" fn label_draw_rect(self_obj: Id, _cmd: Sel, _rect: CGRect) {
    let attr = label_build_attr_string(self_obj);
    let ctx = UIGraphicsGetCurrentContext();
    if attr.is_null() || ctx.is_null() {
        return;
    }
    let bounds: CGRect = msg0(self_obj, "bounds");
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return;
    }
    // UILabel centers its text block vertically.
    let text = label_text_size(self_obj, attr, bounds.size.width);
    let block_h = text.height.min(bounds.size.height);
    let top = bounds.origin.y + ((bounds.size.height - block_h) / 2.0).max(0.0);

    let (fs, frame) = label_frame(attr, bounds.size.width, block_h.max(1.0));
    if frame.is_null() {
        if !fs.is_null() {
            CFRelease(fs);
        }
        return;
    }
    CGContextSaveGState(ctx);
    CGContextSetTextMatrix(ctx, CGAffineTransform { a: 1.0, d: 1.0, ..Default::default() });
    // Flip into CoreText's y-up space, with the frame box at (origin.x, top).
    CGContextTranslateCTM(ctx, bounds.origin.x, top + block_h);
    CGContextScaleCTM(ctx, 1.0, -1.0);
    let lines_wanted = assoc_i64(self_obj, &LABEL_LINES_KEY, 1);
    let lines = CTFrameGetLines(frame);
    let n = pear_objc::nsarray_count(lines);
    if lines_wanted <= 0 || n as i64 <= lines_wanted {
        CTFrameDraw(frame, ctx);
    } else {
        let count = lines_wanted as usize;
        let mut origins = vec![CGPoint::default(); count];
        CTFrameGetLineOrigins(frame, CFRange { location: 0, length: count as isize }, origins.as_mut_ptr());
        for (i, o) in origins.iter().enumerate() {
            CGContextSetTextPosition(ctx, o.x, o.y);
            CTLineDraw(pear_objc::nsarray_object_at(lines, i), ctx);
        }
    }
    CGContextRestoreGState(ctx);
    CFRelease(frame);
    CFRelease(fs);
}

// ---- UIProgressView ----
static PROG_VAL_KEY: u8 = 0;
static PROG_STYLE_KEY: u8 = 0;
static PROG_TINT_KEY: u8 = 0;
static PROG_TRACK_KEY: u8 = 0;

unsafe fn prog_setup(obj: Id) {
    let layer: Id = msg0(obj, "layer");
    if !layer.is_null() {
        msg1::<bool, ()>(layer, "setNeedsDisplayOnBoundsChange:", true);
    }
    msg0::<()>(obj, "setNeedsDisplay");
}
unsafe extern "C" fn prog_init_with_style(self_obj: Id, _cmd: Sel, style: isize) -> Id {
    let frame = CGRect { origin: CGPoint::default(), size: CGSize { width: 100.0, height: 4.0 } };
    let obj: Id = msg1(self_obj, "initWithFrame:", frame);
    if !obj.is_null() {
        assoc_set_i64(obj, &PROG_STYLE_KEY, style as i64);
    }
    obj
}
unsafe extern "C" fn prog_init_with_frame(self_obj: Id, _cmd: Sel, frame: CGRect) -> Id {
    let obj: Id = msg_super1(self_obj, "UIProgressView", "initWithFrame:", frame);
    if !obj.is_null() {
        prog_setup(obj);
    }
    obj
}
unsafe extern "C" fn prog_init_with_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = msg_super1(self_obj, "UIProgressView", "initWithCoder:", coder);
    if obj.is_null() {
        return NIL;
    }
    if !coder.is_null() {
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIProgress")) {
            let v: c_float = msg1(coder, "decodeFloatForKey:", nsstring("UIProgress"));
            prog_set_value(obj, _cmd, v);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIProgressViewStyle")) {
            let st: isize = msg1(coder, "decodeIntegerForKey:", nsstring("UIProgressViewStyle"));
            assoc_set_i64(obj, &PROG_STYLE_KEY, st as i64);
        }
    }
    prog_setup(obj);
    obj
}
unsafe extern "C" fn prog_value(self_obj: Id, _cmd: Sel) -> c_float {
    let v = assoc_get(self_obj, &PROG_VAL_KEY);
    if v.is_null() { 0.0 } else { msg0(v, "floatValue") }
}
unsafe extern "C" fn prog_set_value(self_obj: Id, _cmd: Sel, p: c_float) {
    let clamped = if p.is_nan() { 0.0 } else { p.clamp(0.0, 1.0) };
    let num: Id = msg1::<f32, Id>(cls("NSNumber"), "numberWithFloat:", clamped);
    assoc_set(self_obj, &PROG_VAL_KEY, num);
    msg0::<()>(self_obj, "setNeedsDisplay");
}
unsafe extern "C" fn prog_set_animated(self_obj: Id, _cmd: Sel, p: c_float, _anim: bool) {
    prog_set_value(self_obj, _cmd, p);
}
unsafe extern "C" fn prog_style(self_obj: Id, _cmd: Sel) -> isize {
    assoc_i64(self_obj, &PROG_STYLE_KEY, 0) as isize
}
unsafe extern "C" fn prog_set_style(self_obj: Id, _cmd: Sel, v: isize) {
    assoc_set_i64(self_obj, &PROG_STYLE_KEY, v as i64);
    msg0::<()>(self_obj, "setNeedsDisplay");
}
unsafe extern "C" fn prog_tint(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &PROG_TINT_KEY)
}
unsafe extern "C" fn prog_set_tint(self_obj: Id, _cmd: Sel, c: Id) {
    assoc_set(self_obj, &PROG_TINT_KEY, c);
    msg0::<()>(self_obj, "setNeedsDisplay");
}
unsafe extern "C" fn prog_track(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &PROG_TRACK_KEY)
}
unsafe extern "C" fn prog_set_track(self_obj: Id, _cmd: Sel, c: Id) {
    assoc_set(self_obj, &PROG_TRACK_KEY, c);
    msg0::<()>(self_obj, "setNeedsDisplay");
}
unsafe fn fill_rect(ctx: *mut c_void, color: Id, fallback: (f64, f64, f64, f64), rect: CGRect) {
    if !color.is_null() {
        let cg: *mut c_void = msg0(color, "CGColor");
        if !cg.is_null() {
            CGContextSetFillColorWithColor(ctx, cg);
            CGContextFillRect(ctx, rect);
            return;
        }
    }
    let cg = CGColorCreateSRGB(fallback.0, fallback.1, fallback.2, fallback.3);
    CGContextSetFillColorWithColor(ctx, cg);
    CGContextFillRect(ctx, rect);
    CGColorRelease(cg);
}
unsafe extern "C" fn prog_draw_rect(self_obj: Id, _cmd: Sel, _rect: CGRect) {
    let ctx = UIGraphicsGetCurrentContext();
    if ctx.is_null() {
        return;
    }
    let b: CGRect = msg0(self_obj, "bounds");
    // Default style: light gray track, system blue progress; bar style: clear track.
    let bar_style = assoc_i64(self_obj, &PROG_STYLE_KEY, 0) == 1;
    let track_fallback = if bar_style { (0.0, 0.0, 0.0, 0.0) } else { (0.89, 0.89, 0.91, 1.0) };
    fill_rect(ctx, assoc_get(self_obj, &PROG_TRACK_KEY), track_fallback, b);
    let p = prog_value(self_obj, _cmd) as f64;
    let mut done = b;
    done.size.width = (b.size.width * p).max(0.0);
    if done.size.width > 0.0 {
        fill_rect(ctx, assoc_get(self_obj, &PROG_TINT_KEY), (0.0, 0.478, 1.0, 1.0), done);
    }
}

// ---- drawRect: rendering (UIView as its layer's delegate) ----
/// Renders an overridden -drawRect: into the layer's contents, as UIKit's
/// backing store does. Views that do not override drawRect: keep whatever
/// contents they have (images, Metal, GL).
unsafe extern "C" fn view_display_layer(self_obj: Id, _cmd: Sel, layer: Id) {
    if layer.is_null() || is_kind_of(layer, "CAMetalLayer") || responds_to(layer, "nextDrawable") {
        return;
    }
    let draw = sel("drawRect:");
    let own = class_getMethodImplementation(object_getClass(self_obj), draw);
    let base = class_getMethodImplementation(cls("UIView"), draw);
    if own == base {
        return;
    }
    let bounds: CGRect = msg0(self_obj, "bounds");
    let mut scale: f64 = msg0(layer, "contentsScale");
    if scale <= 0.0 {
        scale = 1.0;
    }
    let w = (bounds.size.width * scale).ceil();
    let h = (bounds.size.height * scale).ceil();
    if w < 1.0 || h < 1.0 || w > 16384.0 || h > 16384.0 {
        msg1::<Id, ()>(layer, "setContents:", NIL);
        return;
    }
    let (wpx, hpx) = (w as usize, h as usize);
    let cs = CGColorSpaceCreateDeviceRGB();
    // premultipliedFirst | byteOrder32Little: CoreAnimation's native layout.
    let ctx = CGBitmapContextCreate(std::ptr::null_mut(), wpx, hpx, 8, wpx * 4, cs, 2 | (2 << 12));
    CGColorSpaceRelease(cs);
    if ctx.is_null() {
        return;
    }
    // UIKit drawing space: origin top-left, y down, in points.
    CGContextTranslateCTM(ctx, 0.0, h);
    CGContextScaleCTM(ctx, scale, -scale);
    CGContextTranslateCTM(ctx, -bounds.origin.x, -bounds.origin.y);
    let opaque: bool = msg0(self_obj, "isOpaque");
    let bg: Id = msg0(self_obj, "backgroundColor");
    if opaque && !bg.is_null() {
        let cg: *mut c_void = msg0(bg, "CGColor");
        if !cg.is_null() {
            CGContextSetFillColorWithColor(ctx, cg);
            CGContextFillRect(ctx, bounds);
        }
    } else {
        CGContextClearRect(ctx, bounds);
    }
    UIGraphicsPushContext(ctx);
    msg1::<CGRect, ()>(self_obj, "drawRect:", bounds);
    UIGraphicsPopContext();
    let img = CGBitmapContextCreateImage(ctx);
    CGContextRelease(ctx);
    msg1::<Id, ()>(layer, "setContents:", img);
    if !img.is_null() {
        CGImageRelease(img);
    }
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
        add_method(img, "init", image_init as Imp, "@@:");
        add_method(img, "initWithCGImage:", image_init_cgimage as Imp, "@@:^{CGImage=}");
        add_method(img, "initWithCGImage:scale:orientation:", image_init_cgimage_scale_ori as Imp, "@@:^{CGImage=}dq");
        add_method(img, "initWithData:", image_init_data as Imp, "@@:@");
        add_method(img, "initWithData:scale:", image_init_data_scale as Imp, "@@:@d");
        add_method(img, "initWithContentsOfFile:", image_init_file as Imp, "@@:@");
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
        add_method(meta, "italicSystemFontOfSize:", font_italic as Imp, "@@:d");
        add_method(meta, "fontWithDescriptor:size:", font_with_descriptor as Imp, "@@:@d");
        add_method(meta, "familyNames", font_family_names as Imp, "@@:");
        add_method(meta, "fontNamesForFamilyName:", font_names_for_family as Imp, "@@:@");
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
        add_method(lbl, "initWithFrame:", label_init_with_frame as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(lbl, "initWithCoder:", label_init_with_coder as Imp, "@@:@");
        add_method(lbl, "attributedText", label_attributed_text as Imp, "@@:");
        add_method(lbl, "setAttributedText:", label_set_attributed_text as Imp, "v@:@");
        add_method(lbl, "textAlignment", label_text_alignment as Imp, "q@:");
        add_method(lbl, "setTextAlignment:", label_set_text_alignment as Imp, "v@:q");
        add_method(lbl, "numberOfLines", label_number_of_lines as Imp, "q@:");
        add_method(lbl, "setNumberOfLines:", label_set_number_of_lines as Imp, "v@:q");
        add_method(lbl, "lineBreakMode", label_line_break_mode as Imp, "q@:");
        add_method(lbl, "setLineBreakMode:", label_set_line_break_mode as Imp, "v@:q");
        add_method(lbl, "intrinsicContentSize", label_intrinsic_size as Imp, "{CGSize=dd}@:");
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
        add_method(prog, "initWithProgressViewStyle:", prog_init_with_style as Imp, "@@:q");
        add_method(prog, "initWithFrame:", prog_init_with_frame as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(prog, "initWithCoder:", prog_init_with_coder as Imp, "@@:@");
        add_method(prog, "progressViewStyle", prog_style as Imp, "q@:");
        add_method(prog, "setProgressViewStyle:", prog_set_style as Imp, "v@:q");
        add_method(prog, "progressTintColor", prog_tint as Imp, "@@:");
        add_method(prog, "setProgressTintColor:", prog_set_tint as Imp, "v@:@");
        add_method(prog, "trackTintColor", prog_track as Imp, "@@:");
        add_method(prog, "setTrackTintColor:", prog_set_track as Imp, "v@:@");
        add_method(prog, "drawRect:", prog_draw_rect as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
    }

    let view = cls("UIView");
    if !view.is_null() {
        add_method(view, "displayLayer:", view_display_layer as Imp, "v@:@");
    }
}
