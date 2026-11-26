//! Clean-room CoreGraphics 2D transforms/contexts/paths/colors, ImageIO,
//! CoreText, and extended QuartzCore 3D transforms & constants.

use crate::{
    cg_color_create_srgb, cg_rect_standardize, CGBitmapCanvas, CGColorRgba, CGImageBuffer,
    CGPoint, CGRect, CATransform3D, CA_TRANSFORM_3D_IDENTITY, CG_RECT_NULL,
};
use pear_foundation::{
    const_nsstr_ptr, nsarray_from_ids, nsdict_from_pairs, nsstring_from_rust,
    rust_bytes_from_nsdata, rust_str_from_nsstring,
};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::{LazyLock, Mutex};

// ---- CGAffineTransform -----------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CGAffineTransform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

pub static CG_AFFINE_TRANSFORM_IDENTITY: CGAffineTransform = CGAffineTransform {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    tx: 0.0,
    ty: 0.0,
};

pub extern "C" fn cg_affine_make_translation(tx: f64, ty: f64) -> CGAffineTransform {
    CGAffineTransform { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx, ty }
}

pub extern "C" fn cg_affine_make_scale(sx: f64, sy: f64) -> CGAffineTransform {
    CGAffineTransform { a: sx, b: 0.0, c: 0.0, d: sy, tx: 0.0, ty: 0.0 }
}

pub extern "C" fn cg_affine_make_rotation(angle: f64) -> CGAffineTransform {
    let (s, c) = angle.sin_cos();
    CGAffineTransform { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
}

pub extern "C" fn cg_affine_concat(t1: CGAffineTransform, t2: CGAffineTransform) -> CGAffineTransform {
    CGAffineTransform {
        a: t1.a * t2.a + t1.b * t2.c,
        b: t1.a * t2.b + t1.b * t2.d,
        c: t1.c * t2.a + t1.d * t2.c,
        d: t1.c * t2.b + t1.d * t2.d,
        tx: t1.tx * t2.a + t1.ty * t2.c + t2.tx,
        ty: t1.tx * t2.b + t1.ty * t2.d + t2.ty,
    }
}

pub extern "C" fn cg_affine_invert(t: CGAffineTransform) -> CGAffineTransform {
    let det = t.a * t.d - t.b * t.c;
    if det.abs() < 1e-12 {
        return t;
    }
    let inv = 1.0 / det;
    CGAffineTransform {
        a: t.d * inv,
        b: -t.b * inv,
        c: -t.c * inv,
        d: t.a * inv,
        tx: (t.c * t.ty - t.d * t.tx) * inv,
        ty: (t.b * t.tx - t.a * t.ty) * inv,
    }
}

pub extern "C" fn cg_affine_translate(t: CGAffineTransform, tx: f64, ty: f64) -> CGAffineTransform {
    cg_affine_concat(cg_affine_make_translation(tx, ty), t)
}

pub extern "C" fn cg_affine_scale(t: CGAffineTransform, sx: f64, sy: f64) -> CGAffineTransform {
    cg_affine_concat(cg_affine_make_scale(sx, sy), t)
}

pub extern "C" fn cg_affine_rotate(t: CGAffineTransform, angle: f64) -> CGAffineTransform {
    cg_affine_concat(cg_affine_make_rotation(angle), t)
}

pub extern "C" fn cg_rect_apply_affine(r: CGRect, t: CGAffineTransform) -> CGRect {
    let pts = [
        (r.origin.x, r.origin.y),
        (r.origin.x + r.size.width, r.origin.y),
        (r.origin.x, r.origin.y + r.size.height),
        (r.origin.x + r.size.width, r.origin.y + r.size.height),
    ];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in pts {
        let nx = t.a * x + t.c * y + t.tx;
        let ny = t.b * x + t.d * y + t.ty;
        min_x = min_x.min(nx);
        min_y = min_y.min(ny);
        max_x = max_x.max(nx);
        max_y = max_y.max(ny);
    }
    CGRect {
        origin: CGPoint { x: min_x, y: min_y },
        size: crate::CGSize { width: max_x - min_x, height: max_y - min_y },
    }
}

pub extern "C" fn cg_rect_integral(r: CGRect) -> CGRect {
    let s = cg_rect_standardize(r);
    let x0 = s.origin.x.floor();
    let y0 = s.origin.y.floor();
    let x1 = (s.origin.x + s.size.width).ceil();
    let y1 = (s.origin.y + s.size.height).ceil();
    CGRect {
        origin: CGPoint { x: x0, y: y0 },
        size: crate::CGSize { width: (x1 - x0).max(0.0), height: (y1 - y0).max(0.0) },
    }
}

pub extern "C" fn cg_rect_intersects(a: CGRect, b: CGRect) -> bool {
    crate::cg_rect_intersection(a, b) != CG_RECT_NULL
}

pub extern "C" fn cg_rect_get_width(r: CGRect) -> f64 {
    cg_rect_standardize(r).size.width
}

pub extern "C" fn cg_rect_get_height(r: CGRect) -> f64 {
    cg_rect_standardize(r).size.height
}

// ---- CGColorSpace / CGColor / CGContext / CGPath / CGImage / Fonts ---------

struct ContextState {
    canvas: CGBitmapCanvas,
    external_buf: *mut u8,
    bytes_per_row: usize,
    ctm: CGAffineTransform,
    saved: Vec<(CGAffineTransform, CGColorRgba, CGColorRgba)>,
    stroke: CGColorRgba,
    line_width: f64,
    pending_rect: Option<CGRect>,
    pending_path: Vec<(f64, f64)>,
}
unsafe impl Send for ContextState {}

struct PathState {
    points: Vec<(f64, f64)>,
}

struct ImageSourceState {
    bytes: Vec<u8>,
}

struct ImageDestState {
    data_obj: *mut c_void,
    image_bytes: Option<Vec<u8>>,
}
unsafe impl Send for ImageDestState {}

static CONTEXTS: LazyLock<Mutex<HashMap<usize, ContextState>>> = LazyLock::new(Default::default);
static PATHS: LazyLock<Mutex<HashMap<usize, PathState>>> = LazyLock::new(Default::default);
static DATA_PROVIDERS: LazyLock<Mutex<HashMap<usize, Vec<u8>>>> = LazyLock::new(Default::default);
static FONTS: LazyLock<Mutex<HashMap<usize, String>>> = LazyLock::new(Default::default);
static IMAGE_SOURCES: LazyLock<Mutex<HashMap<usize, ImageSourceState>>> = LazyLock::new(Default::default);
static IMAGE_DESTS: LazyLock<Mutex<HashMap<usize, ImageDestState>>> = LazyLock::new(Default::default);
static DYNAMIC_CONSTS: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(Default::default);

fn cached_nsstr_const(name: &'static str) -> usize {
    let mut g = DYNAMIC_CONSTS.lock().unwrap();
    if let Some(&p) = g.get(name) {
        return p;
    }
    let val = pear_foundation::nsstring_from_rust(name) as usize;
    let slot = Box::into_raw(Box::new(val)) as usize;
    g.insert(name.to_string(), slot);
    slot
}

pub extern "C" fn cg_color_space_create_device_gray() -> *mut c_void {
    2usize as *mut c_void
}
pub extern "C" fn cg_color_space_create_with_name(_name: *mut c_void) -> *mut c_void {
    3usize as *mut c_void
}
pub extern "C" fn cg_color_space_get_model(cs: *mut c_void) -> i32 {
    if (cs as usize) == 2 { 0 } else { 1 } // Monochrome vs RGB
}

pub unsafe extern "C" fn cg_color_create(cs: *mut c_void, components: *const f64) -> *mut c_void {
    if components.is_null() {
        return cg_color_create_srgb(0.0, 0.0, 0.0, 1.0);
    }
    if (cs as usize) == 2 {
        let g = *components;
        let a = *components.add(1);
        cg_color_create_srgb(g, g, g, a)
    } else {
        cg_color_create_srgb(*components, *components.add(1), *components.add(2), *components.add(3))
    }
}

pub unsafe extern "C" fn cg_color_get_alpha(c: *const c_void) -> f64 {
    if c.is_null() { 1.0 } else { (*(c as *const CGColorRgba)).a }
}
pub extern "C" fn cg_color_get_color_space(_c: *const c_void) -> *mut c_void {
    1usize as *mut c_void
}
pub unsafe extern "C" fn cg_color_get_components(c: *const c_void) -> *const f64 {
    if c.is_null() { std::ptr::null() } else { &(*(c as *const CGColorRgba)).r as *const f64 }
}
pub extern "C" fn cg_color_get_num_components(_c: *const c_void) -> usize {
    4
}

pub extern "C" fn cg_bitmap_context_create(
    data: *mut u8,
    width: usize,
    height: usize,
    _bits_per_component: usize,
    bytes_per_row: usize,
    _space: *mut c_void,
    _bitmap_info: u32,
) -> *mut c_void {
    let w = width.clamp(1, 8192);
    let h = height.clamp(1, 8192);
    let handle = Box::into_raw(Box::new(0usize)) as *mut c_void;
    let st = ContextState {
        canvas: CGBitmapCanvas::new(w, h),
        external_buf: data,
        bytes_per_row: if bytes_per_row == 0 { w * 4 } else { bytes_per_row },
        ctm: CG_AFFINE_TRANSFORM_IDENTITY,
        saved: Vec::new(),
        stroke: CGColorRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        line_width: 1.0,
        pending_rect: None,
        pending_path: Vec::new(),
    };
    CONTEXTS.lock().unwrap().insert(handle as usize, st);
    handle
}

fn sync_context_to_external(st: &ContextState) {
    if st.external_buf.is_null() {
        return;
    }
    let row_bytes = (st.canvas.width * 4).min(st.bytes_per_row);
    for y in 0..st.canvas.height {
        unsafe {
            std::ptr::copy_nonoverlapping(
                st.canvas.rgba[y * st.canvas.width * 4..].as_ptr(),
                st.external_buf.add(y * st.bytes_per_row),
                row_bytes,
            );
        }
    }
}

pub extern "C" fn cg_bitmap_context_create_image(ctx: *mut c_void) -> *mut c_void {
    let map = CONTEXTS.lock().unwrap();
    if let Some(st) = map.get(&(ctx as usize)) {
        let img = CGImageBuffer {
            width: st.canvas.width,
            height: st.canvas.height,
            rgba: st.canvas.rgba.clone(),
        };
        return Box::into_raw(Box::new(img)) as *mut c_void;
    }
    Box::into_raw(Box::new(CGImageBuffer { width: 1, height: 1, rgba: vec![0, 0, 0, 255] })) as *mut c_void
}

pub extern "C" fn cg_context_release(ctx: *mut c_void) {
    CONTEXTS.lock().unwrap().remove(&(ctx as usize));
}

pub extern "C" fn cg_context_save(ctx: *mut c_void) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.saved.push((st.ctm, st.canvas.fill, st.stroke));
    }
}

pub extern "C" fn cg_context_restore(ctx: *mut c_void) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        if let Some((ctm, fill, stroke)) = st.saved.pop() {
            st.ctm = ctm;
            st.canvas.set_fill_color(fill);
            st.stroke = stroke;
        }
    }
}

pub extern "C" fn cg_context_translate_ctm(ctx: *mut c_void, tx: f64, ty: f64) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.ctm = cg_affine_translate(st.ctm, tx, ty);
    }
}
pub extern "C" fn cg_context_scale_ctm(ctx: *mut c_void, sx: f64, sy: f64) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.ctm = cg_affine_scale(st.ctm, sx, sy);
    }
}
pub extern "C" fn cg_context_rotate_ctm(ctx: *mut c_void, angle: f64) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.ctm = cg_affine_rotate(st.ctm, angle);
    }
}
pub extern "C" fn cg_context_concat_ctm(ctx: *mut c_void, t: CGAffineTransform) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.ctm = cg_affine_concat(t, st.ctm);
    }
}

pub unsafe extern "C" fn cg_context_set_fill_color(ctx: *mut c_void, color: *const c_void) {
    if color.is_null() { return; }
    let c = *(color as *const CGColorRgba);
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.canvas.set_fill_color(c);
    }
}

pub unsafe extern "C" fn cg_context_set_stroke_color(ctx: *mut c_void, color: *const c_void) {
    if color.is_null() { return; }
    let c = *(color as *const CGColorRgba);
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.stroke = c;
    }
}

pub extern "C" fn cg_context_set_line_width(ctx: *mut c_void, w: f64) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.line_width = w;
    }
}

pub extern "C" fn cg_context_noop_i32(_ctx: *mut c_void, _v: i32) {}
pub extern "C" fn cg_context_noop_bool(_ctx: *mut c_void, _v: bool) {}

pub extern "C" fn cg_context_fill_rect(ctx: *mut c_void, r: CGRect) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        let tr = cg_rect_apply_affine(r, st.ctm);
        st.canvas.fill_rect(
            tr.origin.x.round() as isize,
            tr.origin.y.round() as isize,
            tr.size.width.max(0.0).round() as usize,
            tr.size.height.max(0.0).round() as usize,
        );
        sync_context_to_external(st);
    }
}

pub extern "C" fn cg_context_stroke_rect(ctx: *mut c_void, r: CGRect) {
    cg_context_fill_rect(ctx, r);
}

pub extern "C" fn cg_context_fill_ellipse(ctx: *mut c_void, r: CGRect) {
    cg_context_fill_rect(ctx, r);
}

pub extern "C" fn cg_context_add_rect(ctx: *mut c_void, r: CGRect) {
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        st.pending_rect = Some(r);
    }
}

pub extern "C" fn cg_context_add_path(ctx: *mut c_void, path: *mut c_void) {
    let pts = PATHS.lock().unwrap().get(&(path as usize)).map(|p| p.points.clone());
    if let Some(pts) = pts {
        if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
            st.pending_path = pts;
        }
    }
}

pub extern "C" fn cg_context_fill_path(ctx: *mut c_void) {
    let rect = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)).and_then(|st| st.pending_rect.take());
    if let Some(r) = rect {
        cg_context_fill_rect(ctx, r);
    }
}

pub extern "C" fn cg_context_stroke_path(ctx: *mut c_void) {
    cg_context_fill_path(ctx);
}

pub extern "C" fn cg_context_clip(_ctx: *mut c_void) {}

pub unsafe extern "C" fn cg_context_draw_image(ctx: *mut c_void, r: CGRect, image: *const c_void) {
    if image.is_null() { return; }
    let img = &*(image as *const CGImageBuffer);
    if let Some(st) = CONTEXTS.lock().unwrap().get_mut(&(ctx as usize)) {
        let tr = cg_rect_apply_affine(r, st.ctm);
        let dst_x0 = tr.origin.x.max(0.0) as usize;
        let dst_y0 = tr.origin.y.max(0.0) as usize;
        let dst_w = (tr.size.width.max(0.0) as usize).min(st.canvas.width.saturating_sub(dst_x0));
        let dst_h = (tr.size.height.max(0.0) as usize).min(st.canvas.height.saturating_sub(dst_y0));
        for dy in 0..dst_h {
            let sy = (dy * img.height) / dst_h.max(1);
            for dx in 0..dst_w {
                let sx = (dx * img.width) / dst_w.max(1);
                let s_idx = (sy * img.width + sx) * 4;
                let d_idx = ((dst_y0 + dy) * st.canvas.width + (dst_x0 + dx)) * 4;
                if s_idx + 4 <= img.rgba.len() && d_idx + 4 <= st.canvas.rgba.len() {
                    st.canvas.rgba[d_idx..d_idx + 4].copy_from_slice(&img.rgba[s_idx..s_idx + 4]);
                }
            }
        }
        sync_context_to_external(st);
    }
}

pub extern "C" fn cg_context_draw_linear_gradient(
    _ctx: *mut c_void,
    _grad: *mut c_void,
    _start: CGPoint,
    _end: CGPoint,
    _opts: u32,
) {}

pub extern "C" fn cg_context_draw_radial_gradient(
    _ctx: *mut c_void,
    _grad: *mut c_void,
    _sc: CGPoint,
    _sr: f64,
    _ec: CGPoint,
    _er: f64,
    _opts: u32,
) {}

pub extern "C" fn cg_path_create_mutable() -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    PATHS.lock().unwrap().insert(h as usize, PathState { points: Vec::new() });
    h
}
pub extern "C" fn cg_path_move_to(p: *mut c_void, _m: *const CGAffineTransform, x: f64, y: f64) {
    if let Some(st) = PATHS.lock().unwrap().get_mut(&(p as usize)) {
        st.points.push((x, y));
    }
}
pub extern "C" fn cg_path_line_to(p: *mut c_void, _m: *const CGAffineTransform, x: f64, y: f64) {
    cg_path_move_to(p, std::ptr::null(), x, y);
}
pub extern "C" fn cg_path_arc_to(
    p: *mut c_void,
    _m: *const CGAffineTransform,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    _r: f64,
) {
    if let Some(st) = PATHS.lock().unwrap().get_mut(&(p as usize)) {
        st.points.push((x1, y1));
        st.points.push((x2, y2));
    }
}
pub extern "C" fn cg_path_close(_p: *mut c_void) {}
pub extern "C" fn cg_path_release(p: *mut c_void) {
    PATHS.lock().unwrap().remove(&(p as usize));
}

pub extern "C" fn cg_gradient_create_with_colors(_space: *mut c_void, _colors: *mut c_void, _locs: *const f64) -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}

pub extern "C" fn cg_data_provider_create_with_url(url: *mut c_void) -> *mut c_void {
    let bytes = rust_str_from_nsstring(url)
        .and_then(|s| std::fs::read(s.strip_prefix("file://").unwrap_or(&s)).ok())
        .unwrap_or_default();
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    DATA_PROVIDERS.lock().unwrap().insert(h as usize, bytes);
    h
}
pub extern "C" fn cg_data_provider_copy_data(prov: *mut c_void) -> *mut c_void {
    let bytes = DATA_PROVIDERS.lock().unwrap().get(&(prov as usize)).cloned().unwrap_or_default();
    pear_foundation::nsdata_from_bytes(&bytes)
}
pub extern "C" fn cg_data_provider_release(prov: *mut c_void) {
    DATA_PROVIDERS.lock().unwrap().remove(&(prov as usize));
}

pub extern "C" fn cg_font_create_with_dp(_dp: *mut c_void) -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    FONTS.lock().unwrap().insert(h as usize, "PearSans".to_string());
    h
}
pub extern "C" fn cg_font_copy_ps_name(f: *mut c_void) -> *mut c_void {
    let n = FONTS.lock().unwrap().get(&(f as usize)).cloned().unwrap_or_else(|| "Helvetica".into());
    nsstring_from_rust(&n)
}
pub extern "C" fn cg_font_release(f: *mut c_void) {
    FONTS.lock().unwrap().remove(&(f as usize));
}

pub extern "C" fn cg_image_retain(img: *mut c_void) -> *mut c_void {
    img
}

pub unsafe extern "C" fn cg_image_create_with_rect(img: *const c_void, rect: CGRect) -> *mut c_void {
    if img.is_null() {
        return std::ptr::null_mut();
    }
    let src = &*(img as *const CGImageBuffer);
    let r = cg_rect_standardize(rect);
    let x0 = (r.origin.x.max(0.0) as usize).min(src.width);
    let y0 = (r.origin.y.max(0.0) as usize).min(src.height);
    let w = (r.size.width.max(1.0) as usize).min(src.width.saturating_sub(x0)).max(1);
    let h = (r.size.height.max(1.0) as usize).min(src.height.saturating_sub(y0)).max(1);
    let mut rgba = vec![0u8; w * h * 4];
    for yy in 0..h {
        let sy = (y0 + yy).min(src.height.saturating_sub(1));
        for xx in 0..w {
            let sx = (x0 + xx).min(src.width.saturating_sub(1));
            let si = (sy * src.width + sx) * 4;
            let di = (yy * w + xx) * 4;
            if si + 4 <= src.rgba.len() {
                rgba[di..di + 4].copy_from_slice(&src.rgba[si..si + 4]);
            }
        }
    }
    Box::into_raw(Box::new(CGImageBuffer { width: w, height: h, rgba })) as *mut c_void
}

pub extern "C" fn cg_image_get_alpha_info(_img: *const c_void) -> u32 { 1 } // kCGImageAlphaPremultipliedLast
pub extern "C" fn cg_image_get_bitmap_info(_img: *const c_void) -> u32 { 1 }
pub extern "C" fn cg_image_get_bpc(_img: *const c_void) -> usize { 8 }
pub extern "C" fn cg_image_get_bpp(_img: *const c_void) -> usize { 32 }
pub unsafe extern "C" fn cg_image_get_bytes_per_row(img: *const c_void) -> usize {
    if img.is_null() { 0 } else { (*(img as *const CGImageBuffer)).width * 4 }
}
pub unsafe extern "C" fn cg_image_get_dp(img: *const c_void) -> *mut c_void {
    let bytes = if img.is_null() { Vec::new() } else { (*(img as *const CGImageBuffer)).rgba.clone() };
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    DATA_PROVIDERS.lock().unwrap().insert(h as usize, bytes);
    h
}
pub extern "C" fn cg_image_get_uttype(_img: *const c_void) -> *mut c_void {
    nsstring_from_rust("public.png")
}

pub fn cg_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CGAffineTransformIdentity" => Some(&raw const CG_AFFINE_TRANSFORM_IDENTITY as usize),
        "_CGAffineTransformMakeTranslation" => f(cg_affine_make_translation as *const ()),
        "_CGAffineTransformMakeScale" => f(cg_affine_make_scale as *const ()),
        "_CGAffineTransformMakeRotation" => f(cg_affine_make_rotation as *const ()),
        "_CGAffineTransformConcat" => f(cg_affine_concat as *const ()),
        "_CGAffineTransformInvert" => f(cg_affine_invert as *const ()),
        "_CGAffineTransformTranslate" => f(cg_affine_translate as *const ()),
        "_CGAffineTransformScale" => f(cg_affine_scale as *const ()),
        "_CGAffineTransformRotate" => f(cg_affine_rotate as *const ()),
        "_CGRectApplyAffineTransform" => f(cg_rect_apply_affine as *const ()),
        "_CGRectIntegral" => f(cg_rect_integral as *const ()),
        "_CGRectIntersectsRect" => f(cg_rect_intersects as *const ()),
        "_CGRectGetWidth" => f(cg_rect_get_width as *const ()),
        "_CGRectGetHeight" => f(cg_rect_get_height as *const ()),
        "_CGColorSpaceCreateDeviceGray" => f(cg_color_space_create_device_gray as *const ()),
        "_CGColorSpaceCreateWithName" => f(cg_color_space_create_with_name as *const ()),
        "_CGColorSpaceGetModel" => f(cg_color_space_get_model as *const ()),
        "_kCGColorSpaceSRGB" => Some(const_nsstr_ptr("kCGColorSpaceSRGB")),
        "_kCGColorSpaceExtendedSRGB" => Some(const_nsstr_ptr("kCGColorSpaceExtendedSRGB")),
        "_kCGColorSpaceExtendedLinearSRGB" => Some(const_nsstr_ptr("kCGColorSpaceExtendedLinearSRGB")),
        "_CGColorCreate" => f(cg_color_create as *const ()),
        "_CGColorGetAlpha" => f(cg_color_get_alpha as *const ()),
        "_CGColorGetColorSpace" => f(cg_color_get_color_space as *const ()),
        "_CGColorGetComponents" => f(cg_color_get_components as *const ()),
        "_CGColorGetNumberOfComponents" => f(cg_color_get_num_components as *const ()),
        "_CGBitmapContextCreate" => f(cg_bitmap_context_create as *const ()),
        "_CGBitmapContextCreateImage" => f(cg_bitmap_context_create_image as *const ()),
        "_CGContextSaveGState" => f(cg_context_save as *const ()),
        "_CGContextRestoreGState" => f(cg_context_restore as *const ()),
        "_CGContextTranslateCTM" => f(cg_context_translate_ctm as *const ()),
        "_CGContextScaleCTM" => f(cg_context_scale_ctm as *const ()),
        "_CGContextRotateCTM" => f(cg_context_rotate_ctm as *const ()),
        "_CGContextConcatCTM" => f(cg_context_concat_ctm as *const ()),
        "_CGContextSetFillColorWithColor" => f(cg_context_set_fill_color as *const ()),
        "_CGContextSetStrokeColorWithColor" => f(cg_context_set_stroke_color as *const ()),
        "_CGContextSetLineWidth" => f(cg_context_set_line_width as *const ()),
        "_CGContextSetBlendMode" | "_CGContextSetInterpolationQuality" => f(cg_context_noop_i32 as *const ()),
        "_CGContextSetAllowsAntialiasing" | "_CGContextSetShouldAntialias" => f(cg_context_noop_bool as *const ()),
        "_CGContextFillRect" => f(cg_context_fill_rect as *const ()),
        "_CGContextStrokeRect" => f(cg_context_stroke_rect as *const ()),
        "_CGContextFillEllipseInRect" => f(cg_context_fill_ellipse as *const ()),
        "_CGContextAddRect" => f(cg_context_add_rect as *const ()),
        "_CGContextAddPath" => f(cg_context_add_path as *const ()),
        "_CGContextFillPath" => f(cg_context_fill_path as *const ()),
        "_CGContextStrokePath" => f(cg_context_stroke_path as *const ()),
        "_CGContextClip" => f(cg_context_clip as *const ()),
        "_CGContextDrawImage" => f(cg_context_draw_image as *const ()),
        "_CGContextDrawLinearGradient" => f(cg_context_draw_linear_gradient as *const ()),
        "_CGContextDrawRadialGradient" => f(cg_context_draw_radial_gradient as *const ()),
        "_CGContextRelease" => f(cg_context_release as *const ()),
        "_CGPathCreateMutable" => f(cg_path_create_mutable as *const ()),
        "_CGPathMoveToPoint" => f(cg_path_move_to as *const ()),
        "_CGPathAddLineToPoint" => f(cg_path_line_to as *const ()),
        "_CGPathAddArcToPoint" => f(cg_path_arc_to as *const ()),
        "_CGPathCloseSubpath" => f(cg_path_close as *const ()),
        "_CGPathRelease" => f(cg_path_release as *const ()),
        "_CGGradientCreateWithColors" => f(cg_gradient_create_with_colors as *const ()),
        "_CGDataProviderCreateWithURL" => f(cg_data_provider_create_with_url as *const ()),
        "_CGDataProviderCopyData" => f(cg_data_provider_copy_data as *const ()),
        "_CGDataProviderRelease" => f(cg_data_provider_release as *const ()),
        "_CGFontCreateWithDataProvider" => f(cg_font_create_with_dp as *const ()),
        "_CGFontCopyPostScriptName" => f(cg_font_copy_ps_name as *const ()),
        "_CGFontRelease" => f(cg_font_release as *const ()),
        "_CGImageRetain" => f(cg_image_retain as *const ()),
        "_CGImageCreateWithImageInRect" => f(cg_image_create_with_rect as *const ()),
        "_CGImageGetAlphaInfo" => f(cg_image_get_alpha_info as *const ()),
        "_CGImageGetBitmapInfo" => f(cg_image_get_bitmap_info as *const ()),
        "_CGImageGetBitsPerComponent" => f(cg_image_get_bpc as *const ()),
        "_CGImageGetBitsPerPixel" => f(cg_image_get_bpp as *const ()),
        "_CGImageGetBytesPerRow" => f(cg_image_get_bytes_per_row as *const ()),
        "_CGImageGetDataProvider" => f(cg_image_get_dp as *const ()),
        "_CGImageGetUTType" => f(cg_image_get_uttype as *const ()),
        _ => None,
    }
}

// ---- ImageIO ---------------------------------------------------------------

pub extern "C" fn cg_img_src_create_with_data(data: *mut c_void, _opts: *mut c_void) -> *mut c_void {
    let bytes = rust_bytes_from_nsdata(data).unwrap_or_default();
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    IMAGE_SOURCES.lock().unwrap().insert(h as usize, ImageSourceState { bytes });
    h
}

pub extern "C" fn cg_img_src_create_with_url(url: *mut c_void, _opts: *mut c_void) -> *mut c_void {
    let bytes = rust_str_from_nsstring(url)
        .and_then(|s| std::fs::read(s.strip_prefix("file://").unwrap_or(&s)).ok())
        .unwrap_or_default();
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    IMAGE_SOURCES.lock().unwrap().insert(h as usize, ImageSourceState { bytes });
    h
}

pub extern "C" fn cg_img_src_create_incremental(_opts: *mut c_void) -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    IMAGE_SOURCES.lock().unwrap().insert(h as usize, ImageSourceState { bytes: Vec::new() });
    h
}

pub extern "C" fn cg_img_src_update_data(src: *mut c_void, data: *mut c_void, _final_flag: bool) {
    if let Some(bytes) = rust_bytes_from_nsdata(data) {
        if let Some(st) = IMAGE_SOURCES.lock().unwrap().get_mut(&(src as usize)) {
            st.bytes = bytes;
        }
    }
}

pub extern "C" fn cg_img_src_get_count(src: *mut c_void) -> usize {
    let empty = IMAGE_SOURCES.lock().unwrap().get(&(src as usize)).map_or(true, |s| s.bytes.is_empty());
    if empty { 0 } else { 1 }
}

pub extern "C" fn cg_img_src_get_type(src: *mut c_void) -> *mut c_void {
    let bytes = IMAGE_SOURCES.lock().unwrap().get(&(src as usize)).map(|s| s.bytes.clone()).unwrap_or_default();
    if bytes.starts_with(b"\x89PNG") {
        nsstring_from_rust("public.png")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        nsstring_from_rust("public.jpeg")
    } else {
        nsstring_from_rust("public.image")
    }
}

pub extern "C" fn cg_img_src_copy_type_ids() -> *mut c_void {
    nsarray_from_ids(vec![
        nsstring_from_rust("public.png"),
        nsstring_from_rust("public.jpeg"),
    ])
}

pub extern "C" fn cg_img_src_copy_props(src: *mut c_void, _idx: usize, _opts: *mut c_void) -> *mut c_void {
    let bytes = IMAGE_SOURCES.lock().unwrap().get(&(src as usize)).map(|s| s.bytes.clone()).unwrap_or_default();
    if let Some(img) = CGImageBuffer::from_png_or_jpeg(&bytes) {
        return nsdict_from_pairs(vec![
            ("PixelWidth".into(), img.width as *mut c_void),
            ("PixelHeight".into(), img.height as *mut c_void),
        ]);
    }
    nsdict_from_pairs(Vec::new())
}

pub extern "C" fn cg_img_src_copy_props_top(src: *mut c_void, opts: *mut c_void) -> *mut c_void {
    cg_img_src_copy_props(src, 0, opts)
}

pub extern "C" fn cg_img_src_create_image(src: *mut c_void, _idx: usize, _opts: *mut c_void) -> *mut c_void {
    let bytes = IMAGE_SOURCES.lock().unwrap().get(&(src as usize)).map(|s| s.bytes.clone()).unwrap_or_default();
    if let Some(img) = CGImageBuffer::from_png_or_jpeg(&bytes) {
        return Box::into_raw(Box::new(img)) as *mut c_void;
    }
    std::ptr::null_mut()
}

pub extern "C" fn cg_img_src_remove_cache(_src: *mut c_void, _idx: usize) {}

pub extern "C" fn cg_img_dest_create_with_data(
    data: *mut c_void,
    _type_str: *mut c_void,
    _count: usize,
    _opts: *mut c_void,
) -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    IMAGE_DESTS.lock().unwrap().insert(h as usize, ImageDestState { data_obj: data, image_bytes: None });
    h
}

pub extern "C" fn cg_img_dest_set_props(_dest: *mut c_void, _props: *mut c_void) {}

pub unsafe extern "C" fn cg_img_dest_add_image(dest: *mut c_void, img: *const c_void, _props: *mut c_void) {
    if img.is_null() { return; }
    let buf = &*(img as *const CGImageBuffer);
    if let Some(st) = IMAGE_DESTS.lock().unwrap().get_mut(&(dest as usize)) {
        st.image_bytes = Some(buf.rgba.clone());
    }
}

pub extern "C" fn cg_img_dest_finalize(dest: *mut c_void) -> bool {
    IMAGE_DESTS.lock().unwrap().remove(&(dest as usize)).is_some()
}

pub fn imageio_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CGImageSourceCreateWithData" => f(cg_img_src_create_with_data as *const ()),
        "_CGImageSourceCreateWithURL" => f(cg_img_src_create_with_url as *const ()),
        "_CGImageSourceCreateIncremental" => f(cg_img_src_create_incremental as *const ()),
        "_CGImageSourceUpdateData" => f(cg_img_src_update_data as *const ()),
        "_CGImageSourceGetCount" => f(cg_img_src_get_count as *const ()),
        "_CGImageSourceGetType" => f(cg_img_src_get_type as *const ()),
        "_CGImageSourceCopyTypeIdentifiers" => f(cg_img_src_copy_type_ids as *const ()),
        "_CGImageSourceCopyProperties" => f(cg_img_src_copy_props_top as *const ()),
        "_CGImageSourceCopyPropertiesAtIndex" => f(cg_img_src_copy_props as *const ()),
        "_CGImageSourceCreateImageAtIndex"
        | "_CGImageSourceCreateThumbnailAtIndex" => f(cg_img_src_create_image as *const ()),
        "_CGImageSourceRemoveCacheAtIndex" => f(cg_img_src_remove_cache as *const ()),
        "_CGImageDestinationCreateWithData" => f(cg_img_dest_create_with_data as *const ()),
        "_CGImageDestinationSetProperties" => f(cg_img_dest_set_props as *const ()),
        "_CGImageDestinationAddImage" => f(cg_img_dest_add_image as *const ()),
        "_CGImageDestinationFinalize" => f(cg_img_dest_finalize as *const ()),
        s if s.starts_with("_kCGImage") => {
            let leaked: &'static str = Box::leak(s.trim_start_matches('_').to_string().into_boxed_str());
            Some(cached_nsstr_const(leaked))
        }
        _ => None,
    }
}

// ---- CoreText --------------------------------------------------------------

pub extern "C" fn ct_font_desc_create_with_attrs(_attrs: *mut c_void) -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}
pub extern "C" fn ct_font_desc_create_matching(desc: *mut c_void, _mand: *mut c_void) -> *mut c_void {
    desc
}
pub extern "C" fn ct_font_desc_copy_attr(_desc: *mut c_void, _attr: *mut c_void) -> *mut c_void {
    nsstring_from_rust("Helvetica")
}
pub extern "C" fn ct_font_mgr_register_graphics_font(_font: *mut c_void, _err: *mut *mut c_void) -> bool {
    true
}

pub fn coretext_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CTFontDescriptorCreateWithAttributes" => f(ct_font_desc_create_with_attrs as *const ()),
        "_CTFontDescriptorCreateMatchingFontDescriptor" => f(ct_font_desc_create_matching as *const ()),
        "_CTFontDescriptorCopyAttribute" => f(ct_font_desc_copy_attr as *const ()),
        "_CTFontManagerRegisterGraphicsFont" => f(ct_font_mgr_register_graphics_font as *const ()),
        s if s.starts_with("_kCTFont") => {
            let leaked: &'static str = Box::leak(s.trim_start_matches('_').to_string().into_boxed_str());
            Some(cached_nsstr_const(leaked))
        }
        _ => None,
    }
}

// ---- Extended QuartzCore ---------------------------------------------------

pub extern "C" fn ca_transform_3d_make_rotation(angle: f64, x: f64, y: f64, z: f64) -> CATransform3D {
    let len = (x * x + y * y + z * z).sqrt();
    if len < 1e-12 {
        return CA_TRANSFORM_3D_IDENTITY;
    }
    let (ux, uy, uz) = (x / len, y / len, z / len);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    CATransform3D {
        m11: t * ux * ux + c,
        m12: t * ux * uy + s * uz,
        m13: t * ux * uz - s * uy,
        m14: 0.0,
        m21: t * ux * uy - s * uz,
        m22: t * uy * uy + c,
        m23: t * uy * uz + s * ux,
        m24: 0.0,
        m31: t * ux * uz + s * uy,
        m32: t * uy * uz - s * ux,
        m33: t * uz * uz + c,
        m34: 0.0,
        m41: 0.0,
        m42: 0.0,
        m43: 0.0,
        m44: 1.0,
    }
}

pub fn quartz_symbol_ext(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CATransform3DMakeRotation" => f(ca_transform_3d_make_rotation as *const ()),
        s if s.starts_with("_kCA") => {
            let leaked: &'static str = Box::leak(s.trim_start_matches('_').to_string().into_boxed_str());
            Some(cached_nsstr_const(leaked))
        }
        _ => None,
    }
}

#[allow(dead_code)]
fn _keep_c_types(_p: *const c_char, _cs: Option<&CStr>) {}
