//! Clean-room Accelerate.framework (`vDSP_*`, `vvexpf`, `vImage*`)
//! pure-Rust implementations.

use pear_graphics::CGImageBuffer;
use std::ffi::c_void;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VImageBuffer {
    pub data: *mut u8,
    pub height: usize,
    pub width: usize,
    pub row_bytes: usize,
}

pub unsafe extern "C" fn vdsp_dotpr(
    a: *const f32,
    ia: isize,
    b: *const f32,
    ib: isize,
    c: *mut f32,
    n: usize,
) {
    if a.is_null() || b.is_null() || c.is_null() {
        return;
    }
    let mut sum = 0.0f32;
    for i in 0..n {
        sum += *a.offset(i as isize * ia) * *b.offset(i as isize * ib);
    }
    *c = sum;
}

pub unsafe extern "C" fn vdsp_maxv(a: *const f32, ia: isize, c: *mut f32, n: usize) {
    if a.is_null() || c.is_null() || n == 0 {
        return;
    }
    let mut m = *a;
    for i in 1..n {
        let v = *a.offset(i as isize * ia);
        if v > m {
            m = v;
        }
    }
    *c = m;
}

pub unsafe extern "C" fn vdsp_sve(a: *const f32, ia: isize, c: *mut f32, n: usize) {
    if a.is_null() || c.is_null() {
        return;
    }
    let mut sum = 0.0f32;
    for i in 0..n {
        sum += *a.offset(i as isize * ia);
    }
    *c = sum;
}

pub unsafe extern "C" fn vdsp_vclip(
    a: *const f32,
    ia: isize,
    low: *const f32,
    high: *const f32,
    c: *mut f32,
    ic: isize,
    n: usize,
) {
    if a.is_null() || low.is_null() || high.is_null() || c.is_null() {
        return;
    }
    let lo = *low;
    let hi = *high;
    for i in 0..n {
        let v = *a.offset(i as isize * ia);
        *c.offset(i as isize * ic) = v.clamp(lo, hi);
    }
}

pub unsafe extern "C" fn vdsp_vsadd(
    a: *const f32,
    ia: isize,
    b: *const f32,
    c: *mut f32,
    ic: isize,
    n: usize,
) {
    if a.is_null() || b.is_null() || c.is_null() {
        return;
    }
    let scalar = *b;
    for i in 0..n {
        *c.offset(i as isize * ic) = *a.offset(i as isize * ia) + scalar;
    }
}

pub unsafe extern "C" fn vdsp_vsdiv(
    a: *const f32,
    ia: isize,
    b: *const f32,
    c: *mut f32,
    ic: isize,
    n: usize,
) {
    if a.is_null() || b.is_null() || c.is_null() {
        return;
    }
    let denom = *b;
    for i in 0..n {
        *c.offset(i as isize * ic) = *a.offset(i as isize * ia) / denom;
    }
}

/// Matrix multiply `C[m x n] = A[m x p] * B[p x n]` with arbitrary strides.
pub unsafe extern "C" fn vdsp_mmul(
    a: *const f32,
    ia: isize,
    b: *const f32,
    ib: isize,
    c: *mut f32,
    ic: isize,
    m: usize,
    n: usize,
    p: usize,
) {
    if a.is_null() || b.is_null() || c.is_null() {
        return;
    }
    for row in 0..m {
        for col in 0..n {
            let mut acc = 0.0f32;
            for k in 0..p {
                let va = *a.offset(((row * p + k) as isize) * ia);
                let vb = *b.offset(((k * n + col) as isize) * ib);
                acc += va * vb;
            }
            *c.offset(((row * n + col) as isize) * ic) = acc;
        }
    }
}

pub unsafe extern "C" fn vvexpf(z: *mut f32, x: *const f32, n_ptr: *const i32) {
    if z.is_null() || x.is_null() || n_ptr.is_null() {
        return;
    }
    let n = (*n_ptr).max(0) as usize;
    for i in 0..n {
        *z.add(i) = (*x.add(i)).exp();
    }
}

pub unsafe extern "C" fn vimage_buffer_init(
    buf: *mut VImageBuffer,
    height: usize,
    width: usize,
    pixel_bits: u32,
    _flags: u32,
) -> isize {
    if buf.is_null() {
        return -50;
    }
    let row_bytes = width.saturating_mul(((pixel_bits as usize) + 7) / 8);
    let total = row_bytes.saturating_mul(height).max(1);
    let data = Box::leak(vec![0u8; total].into_boxed_slice()).as_mut_ptr();
    *buf = VImageBuffer {
        data,
        height,
        width,
        row_bytes,
    };
    0
}

pub unsafe extern "C" fn vimage_buffer_init_with_cgimage(
    buf: *mut VImageBuffer,
    _format: *const c_void,
    _bg: *const f32,
    image: *const c_void,
    _flags: u32,
) -> isize {
    if buf.is_null() || image.is_null() {
        return -50;
    }
    let img = &*(image as *const CGImageBuffer);
    let row_bytes = img.width * 4;
    let data = Box::leak(img.rgba.clone().into_boxed_slice()).as_mut_ptr();
    *buf = VImageBuffer {
        data,
        height: img.height,
        width: img.width,
        row_bytes,
    };
    0
}

pub unsafe extern "C" fn vimage_create_cgimage_from_buffer(
    buf: *const VImageBuffer,
    _format: *const c_void,
    _cb: *const c_void,
    _user_data: *mut c_void,
    _flags: u32,
    error: *mut isize,
) -> *mut c_void {
    if !error.is_null() {
        *error = 0;
    }
    if buf.is_null() || (*buf).data.is_null() {
        return std::ptr::null_mut();
    }
    let b = &*buf;
    let mut rgba = vec![0u8; b.width * b.height * 4];
    let copy_w = (b.width * 4).min(b.row_bytes);
    for y in 0..b.height {
        std::ptr::copy_nonoverlapping(
            b.data.add(y * b.row_bytes),
            rgba[y * b.width * 4..].as_mut_ptr(),
            copy_w,
        );
    }
    Box::into_raw(Box::new(CGImageBuffer {
        width: b.width,
        height: b.height,
        rgba,
    })) as *mut c_void
}

pub unsafe extern "C" fn vimage_scale_argb8888(
    src: *const VImageBuffer,
    dest: *const VImageBuffer,
    _temp_buffer: *mut c_void,
    _flags: u32,
) -> isize {
    if src.is_null() || dest.is_null() {
        return -50;
    }
    let (s, d) = (&*src, &*dest);
    if s.data.is_null() || d.data.is_null() || s.width == 0 || s.height == 0 {
        return -50;
    }
    for dy in 0..d.height {
        let sy = (dy * s.height) / d.height.max(1);
        for dx in 0..d.width {
            let sx = (dx * s.width) / d.width.max(1);
            let sp = s.data.add(sy * s.row_bytes + sx * 4);
            let dp = d.data.add(dy * d.row_bytes + dx * 4);
            std::ptr::copy_nonoverlapping(sp, dp, 4);
        }
    }
    0
}

pub unsafe extern "C" fn vimage_box_convolve_argb8888(
    src: *const VImageBuffer,
    dest: *const VImageBuffer,
    _temp_buffer: *mut c_void,
    _src_x: u32,
    _src_y: u32,
    _kw: u32,
    _kh: u32,
    _bg: *const u8,
    _flags: u32,
) -> isize {
    vimage_scale_argb8888(src, dest, std::ptr::null_mut(), 0)
}

pub fn symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_vDSP_dotpr" => f(vdsp_dotpr as *const ()),
        "_vDSP_maxv" => f(vdsp_maxv as *const ()),
        "_vDSP_sve" => f(vdsp_sve as *const ()),
        "_vDSP_vclip" => f(vdsp_vclip as *const ()),
        "_vDSP_vsadd" => f(vdsp_vsadd as *const ()),
        "_vDSP_vsdiv" => f(vdsp_vsdiv as *const ()),
        "_vDSP_mmul" => f(vdsp_mmul as *const ()),
        "_vvexpf" => f(vvexpf as *const ()),
        "_vImageBuffer_Init" => f(vimage_buffer_init as *const ()),
        "_vImageBuffer_InitWithCGImage" => f(vimage_buffer_init_with_cgimage as *const ()),
        "_vImageCreateCGImageFromBuffer" => f(vimage_create_cgimage_from_buffer as *const ()),
        "_vImageScale_ARGB8888" => f(vimage_scale_argb8888 as *const ()),
        "_vImageBoxConvolve_ARGB8888" => f(vimage_box_convolve_argb8888 as *const ()),
        _ => None,
    }
}
