//! Clean-room: portable CoreGraphics-compatible RGBA bitmap context, color,
//! image, and basic glyph rasterizer in pure Rust (zero Apple framework deps)
//! for non-macOS hosts and `pear-runtime`.

use std::ffi::c_void;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CGColorRgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl CGColorRgba {
    pub fn to_u8(&self) -> [u8; 4] {
        let clamp = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [clamp(self.r), clamp(self.g), clamp(self.b), clamp(self.a)]
    }
}

#[derive(Clone, Debug)]
pub struct CGImageBuffer {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl CGImageBuffer {
    pub fn from_png_or_jpeg(bytes: &[u8]) -> Option<Self> {
        if let Ok(png) = pear_image::png::decode(bytes) {
            return Some(Self {
                width: png.width as usize,
                height: png.height as usize,
                rgba: png.pixels,
            });
        }
        if let Ok(jpg) = pear_image::jpeg::decode(bytes) {
            return Some(Self {
                width: jpg.width as usize,
                height: jpg.height as usize,
                rgba: jpg.pixels,
            });
        }
        None
    }
}

pub struct CGBitmapCanvas {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub fill: CGColorRgba,
}

impl CGBitmapCanvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; width * height * 4],
            fill: CGColorRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        }
    }

    pub fn set_fill_color(&mut self, color: CGColorRgba) {
        self.fill = color;
    }

    pub fn fill_rect(&mut self, x: isize, y: isize, w: usize, h: usize) {
        let px = self.fill.to_u8();
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + w as isize).max(0).min(self.width as isize) as usize;
        let y1 = (y + h as isize).max(0).min(self.height as isize) as usize;
        for yy in y0..y1 {
            for xx in x0..x1 {
                let idx = (yy * self.width + xx) * 4;
                self.rgba[idx..idx + 4].copy_from_slice(&px);
            }
        }
    }

    pub fn into_image(self) -> CGImageBuffer {
        CGImageBuffer {
            width: self.width,
            height: self.height,
            rgba: self.rgba,
        }
    }

    pub fn draw_text(&mut self, text: &str, x: isize, y: isize, scale: usize) {
        let s = scale.max(1);
        let px = self.fill.to_u8();
        let mut cx = x;
        let mut cy = y;
        for ch in text.chars() {
            if ch == '\n' {
                cx = x;
                cy += (8 * s) as isize;
                continue;
            }
            let glyph = glyph_5x7(ch);
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if (bits >> (4 - col)) & 1 != 0 {
                        for sy in 0..s {
                            for sx in 0..s {
                                let px_x = cx + (col * s + sx) as isize;
                                let px_y = cy + (row * s + sy) as isize;
                                if px_x >= 0
                                    && px_y >= 0
                                    && (px_x as usize) < self.width
                                    && (px_y as usize) < self.height
                                {
                                    let idx = (px_y as usize * self.width + px_x as usize) * 4;
                                    self.rgba[idx..idx + 4].copy_from_slice(&px);
                                }
                            }
                        }
                    }
                }
            }
            cx += (6 * s) as isize;
        }
    }
}

pub fn measure_text(text: &str, scale: usize) -> CGSize {
    let s = scale.max(1);
    let mut max_w = 0usize;
    let mut cur_w = 0usize;
    let mut lines = 1usize;
    for ch in text.chars() {
        if ch == '\n' {
            max_w = max_w.max(cur_w);
            cur_w = 0;
            lines += 1;
        } else {
            cur_w += 6 * s;
        }
    }
    max_w = max_w.max(cur_w);
    CGSize {
        width: max_w as f64,
        height: (lines * 8 * s) as f64,
    }
}

fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ',' => [0, 0, 0, 0, 0, 0b00100, 0b01000],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00010, 0b00100, 0b00100, 0, 0b00100],
        '/' => [0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111],
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

pub static CG_POINT_ZERO: CGPoint = CGPoint { x: 0.0, y: 0.0 };
pub static CG_SIZE_ZERO: CGSize = CGSize { width: 0.0, height: 0.0 };
pub static CG_RECT_ZERO: CGRect = CGRect {
    origin: CGPoint { x: 0.0, y: 0.0 },
    size: CGSize { width: 0.0, height: 0.0 },
};
pub static CG_RECT_NULL: CGRect = CGRect {
    origin: CGPoint { x: f64::INFINITY, y: f64::INFINITY },
    size: CGSize { width: 0.0, height: 0.0 },
};
pub static CG_RECT_INFINITE: CGRect = CGRect {
    origin: CGPoint { x: -f64::MAX / 2.0, y: -f64::MAX / 2.0 },
    size: CGSize { width: f64::MAX, height: f64::MAX },
};

pub extern "C" fn cg_rect_standardize(r: CGRect) -> CGRect {
    let mut out = r;
    if out.size.width < 0.0 {
        out.origin.x += out.size.width;
        out.size.width = -out.size.width;
    }
    if out.size.height < 0.0 {
        out.origin.y += out.size.height;
        out.size.height = -out.size.height;
    }
    out
}

pub extern "C" fn cg_rect_is_empty(r: CGRect) -> bool {
    r.size.width <= 0.0 || r.size.height <= 0.0 || r.origin.x.is_infinite() || r.origin.y.is_infinite()
}

pub extern "C" fn cg_rect_get_min_x(r: CGRect) -> f64 { cg_rect_standardize(r).origin.x }
pub extern "C" fn cg_rect_get_min_y(r: CGRect) -> f64 { cg_rect_standardize(r).origin.y }
pub extern "C" fn cg_rect_get_max_x(r: CGRect) -> f64 { let s = cg_rect_standardize(r); s.origin.x + s.size.width }
pub extern "C" fn cg_rect_get_max_y(r: CGRect) -> f64 { let s = cg_rect_standardize(r); s.origin.y + s.size.height }
pub extern "C" fn cg_rect_get_mid_x(r: CGRect) -> f64 { let s = cg_rect_standardize(r); s.origin.x + s.size.width * 0.5 }
pub extern "C" fn cg_rect_get_mid_y(r: CGRect) -> f64 { let s = cg_rect_standardize(r); s.origin.y + s.size.height * 0.5 }

pub extern "C" fn cg_rect_inset(r: CGRect, dx: f64, dy: f64) -> CGRect {
    let mut s = cg_rect_standardize(r);
    s.origin.x += dx;
    s.origin.y += dy;
    s.size.width -= 2.0 * dx;
    s.size.height -= 2.0 * dy;
    if s.size.width < 0.0 || s.size.height < 0.0 {
        return CG_RECT_NULL;
    }
    s
}

pub extern "C" fn cg_rect_offset(r: CGRect, dx: f64, dy: f64) -> CGRect {
    CGRect {
        origin: CGPoint { x: r.origin.x + dx, y: r.origin.y + dy },
        size: r.size,
    }
}

pub extern "C" fn cg_rect_equal_to_rect(a: CGRect, b: CGRect) -> bool {
    let sa = cg_rect_standardize(a);
    let sb = cg_rect_standardize(b);
    sa == sb
}

pub extern "C" fn cg_rect_contains_point(r: CGRect, p: CGPoint) -> bool {
    if cg_rect_is_empty(r) {
        return false;
    }
    let s = cg_rect_standardize(r);
    p.x >= s.origin.x && p.x < s.origin.x + s.size.width && p.y >= s.origin.y && p.y < s.origin.y + s.size.height
}

pub extern "C" fn cg_rect_intersection(a: CGRect, b: CGRect) -> CGRect {
    if cg_rect_is_empty(a) || cg_rect_is_empty(b) {
        return CG_RECT_NULL;
    }
    let sa = cg_rect_standardize(a);
    let sb = cg_rect_standardize(b);
    let x0 = sa.origin.x.max(sb.origin.x);
    let y0 = sa.origin.y.max(sb.origin.y);
    let x1 = (sa.origin.x + sa.size.width).min(sb.origin.x + sb.size.width);
    let y1 = (sa.origin.y + sa.size.height).min(sb.origin.y + sb.size.height);
    if x1 <= x0 || y1 <= y0 {
        CG_RECT_NULL
    } else {
        CGRect {
            origin: CGPoint { x: x0, y: y0 },
            size: CGSize { width: x1 - x0, height: y1 - y0 },
        }
    }
}

pub extern "C" fn cg_rect_union(a: CGRect, b: CGRect) -> CGRect {
    if cg_rect_is_empty(a) { return b; }
    if cg_rect_is_empty(b) { return a; }
    let sa = cg_rect_standardize(a);
    let sb = cg_rect_standardize(b);
    let x0 = sa.origin.x.min(sb.origin.x);
    let y0 = sa.origin.y.min(sb.origin.y);
    let x1 = (sa.origin.x + sa.size.width).max(sb.origin.x + sb.size.width);
    let y1 = (sa.origin.y + sa.size.height).max(sb.origin.y + sb.size.height);
    CGRect {
        origin: CGPoint { x: x0, y: y0 },
        size: CGSize { width: x1 - x0, height: y1 - y0 },
    }
}

pub extern "C" fn cg_color_space_create_device_rgb() -> *mut c_void {
    1usize as *mut c_void
}

pub extern "C" fn cg_color_space_release(_cs: *mut c_void) {}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CATransform3D {
    pub m11: f64, pub m12: f64, pub m13: f64, pub m14: f64,
    pub m21: f64, pub m22: f64, pub m23: f64, pub m24: f64,
    pub m31: f64, pub m32: f64, pub m33: f64, pub m34: f64,
    pub m41: f64, pub m42: f64, pub m43: f64, pub m44: f64,
}

pub static CA_TRANSFORM_3D_IDENTITY: CATransform3D = CATransform3D {
    m11: 1.0, m12: 0.0, m13: 0.0, m14: 0.0,
    m21: 0.0, m22: 1.0, m23: 0.0, m24: 0.0,
    m31: 0.0, m32: 0.0, m33: 1.0, m34: 0.0,
    m41: 0.0, m42: 0.0, m43: 0.0, m44: 1.0,
};

pub extern "C" fn ca_transform_3d_is_identity(t: CATransform3D) -> bool {
    t == CA_TRANSFORM_3D_IDENTITY
}

pub extern "C" fn ca_transform_3d_make_scale(sx: f64, sy: f64, sz: f64) -> CATransform3D {
    let mut t = CA_TRANSFORM_3D_IDENTITY;
    t.m11 = sx;
    t.m22 = sy;
    t.m33 = sz;
    t
}

pub extern "C" fn ca_transform_3d_make_translation(tx: f64, ty: f64, tz: f64) -> CATransform3D {
    let mut t = CA_TRANSFORM_3D_IDENTITY;
    t.m41 = tx;
    t.m42 = ty;
    t.m43 = tz;
    t
}

pub extern "C" fn ca_transform_3d_scale(mut t: CATransform3D, sx: f64, sy: f64, sz: f64) -> CATransform3D {
    t.m11 *= sx; t.m12 *= sx; t.m13 *= sx; t.m14 *= sx;
    t.m21 *= sy; t.m22 *= sy; t.m23 *= sy; t.m24 *= sy;
    t.m31 *= sz; t.m32 *= sz; t.m33 *= sz; t.m34 *= sz;
    t
}

pub extern "C" fn ca_transform_3d_translate(mut t: CATransform3D, tx: f64, ty: f64, tz: f64) -> CATransform3D {
    t.m41 += t.m11 * tx + t.m21 * ty + t.m31 * tz;
    t.m42 += t.m12 * tx + t.m22 * ty + t.m32 * tz;
    t.m43 += t.m13 * tx + t.m23 * ty + t.m33 * tz;
    t.m44 += t.m14 * tx + t.m24 * ty + t.m34 * tz;
    t
}

pub fn quartz_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CATransform3DIdentity" => Some(&raw const CA_TRANSFORM_3D_IDENTITY as usize),
        "_CATransform3DIsIdentity" => f(ca_transform_3d_is_identity as *const ()),
        "_CATransform3DMakeScale" => f(ca_transform_3d_make_scale as *const ()),
        "_CATransform3DMakeTranslation" => f(ca_transform_3d_make_translation as *const ()),
        "_CATransform3DScale" => f(ca_transform_3d_scale as *const ()),
        "_CATransform3DTranslate" => f(ca_transform_3d_translate as *const ()),
        _ => None,
    }
}

pub fn symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CGPointZero" => Some(&raw const CG_POINT_ZERO as usize),
        "_CGSizeZero" => Some(&raw const CG_SIZE_ZERO as usize),
        "_CGRectZero" => Some(&raw const CG_RECT_ZERO as usize),
        "_CGRectNull" => Some(&raw const CG_RECT_NULL as usize),
        "_CGRectInfinite" => Some(&raw const CG_RECT_INFINITE as usize),
        "_CGRectStandardize" => f(cg_rect_standardize as *const ()),
        "_CGRectIsEmpty" => f(cg_rect_is_empty as *const ()),
        "_CGRectGetMinX" => f(cg_rect_get_min_x as *const ()),
        "_CGRectGetMinY" => f(cg_rect_get_min_y as *const ()),
        "_CGRectGetMaxX" => f(cg_rect_get_max_x as *const ()),
        "_CGRectGetMaxY" => f(cg_rect_get_max_y as *const ()),
        "_CGRectGetMidX" => f(cg_rect_get_mid_x as *const ()),
        "_CGRectGetMidY" => f(cg_rect_get_mid_y as *const ()),
        "_CGRectInset" => f(cg_rect_inset as *const ()),
        "_CGRectOffset" => f(cg_rect_offset as *const ()),
        "_CGRectEqualToRect" => f(cg_rect_equal_to_rect as *const ()),
        "_CGRectContainsPoint" => f(cg_rect_contains_point as *const ()),
        "_CGRectIntersection" => f(cg_rect_intersection as *const ()),
        "_CGRectUnion" => f(cg_rect_union as *const ()),
        "_CGColorSpaceCreateDeviceRGB" => f(cg_color_space_create_device_rgb as *const ()),
        "_CGColorSpaceRelease" => f(cg_color_space_release as *const ()),
        "_CGColorCreateSRGB" => f(cg_color_create_srgb as *const ()),
        "_CGColorRelease" => f(cg_color_release as *const ()),
        "_CGImageGetWidth" => f(cg_image_get_width as *const ()),
        "_CGImageGetHeight" => f(cg_image_get_height as *const ()),
        "_CGImageRelease" => f(cg_image_release as *const ()),
        _ => None,
    }
}

pub extern "C" fn cg_color_create_srgb(r: f64, g: f64, b: f64, a: f64) -> *mut c_void {
    Box::into_raw(Box::new(CGColorRgba { r, g, b, a })) as *mut c_void
}

pub unsafe extern "C" fn cg_color_release(p: *mut c_void) {
    if !p.is_null() {
        drop(Box::from_raw(p as *mut CGColorRgba));
    }
}

pub unsafe extern "C" fn cg_image_get_width(p: *const c_void) -> usize {
    if p.is_null() { 0 } else { (*(p as *const CGImageBuffer)).width }
}

pub unsafe extern "C" fn cg_image_get_height(p: *const c_void) -> usize {
    if p.is_null() { 0 } else { (*(p as *const CGImageBuffer)).height }
}

pub unsafe extern "C" fn cg_image_release(p: *mut c_void) {
    if !p.is_null() {
        drop(Box::from_raw(p as *mut CGImageBuffer));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_canvas_fills_and_exports_cgimage() {
        let mut c = CGBitmapCanvas::new(16, 16);
        c.set_fill_color(CGColorRgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
        c.fill_rect(2, 2, 4, 4);
        let img = c.into_image();
        assert_eq!(img.width, 16);
        let idx = (3 * 16 + 3) * 4;
        assert_eq!(&img.rgba[idx..idx + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn cgrect_and_catransform3d_math() {
        let r = CGRect {
            origin: CGPoint { x: 10.0, y: 20.0 },
            size: CGSize { width: 100.0, height: 50.0 },
        };
        let inset = cg_rect_inset(r, 5.0, 10.0);
        assert_eq!(inset.origin, CGPoint { x: 15.0, y: 30.0 });
        assert_eq!(inset.size, CGSize { width: 90.0, height: 30.0 });
        assert!(cg_rect_contains_point(inset, CGPoint { x: 50.0, y: 40.0 }));
        assert_eq!(cg_rect_get_mid_x(r), 60.0);
        assert_eq!(cg_rect_get_max_y(r), 70.0);

        assert!(ca_transform_3d_is_identity(CA_TRANSFORM_3D_IDENTITY));
        let scaled = ca_transform_3d_make_scale(2.0, 3.0, 1.0);
        assert!(!ca_transform_3d_is_identity(scaled));
        let moved = ca_transform_3d_translate(scaled, 4.0, 5.0, 0.0);
        assert_eq!(moved.m41, 8.0);
        assert_eq!(moved.m42, 15.0);
    }

    #[test]
    fn bitmap_text_rasterizer_measures_and_draws_pixels() {
        let sz = measure_text("HI!", 2);
        assert_eq!(sz, CGSize { width: 36.0, height: 16.0 });
        let mut c = CGBitmapCanvas::new(40, 20);
        c.set_fill_color(CGColorRgba { r: 0.0, g: 1.0, b: 0.0, a: 1.0 });
        c.draw_text("HI!", 2, 2, 2);
        let inked = c.rgba.chunks_exact(4).filter(|p| p[1] == 255).count();
        assert!(inked > 40);
    }
}
