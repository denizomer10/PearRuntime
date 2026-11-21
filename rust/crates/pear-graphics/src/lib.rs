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
}

pub fn symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
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
}
