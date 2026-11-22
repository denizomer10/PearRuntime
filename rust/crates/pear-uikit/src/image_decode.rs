//! UIImage's decoder: PearRuntime's own (pear-image), presented as a CGImage
//! with the layout ImageIO gives the same file (bits per pixel, alpha info,
//! colour space), so apps that inspect or upload a CGImage's bytes see what
//! they would on iOS. Files whose colour information pear-image does not
//! apply (ICC profiles, PNG gamma, 16-bit samples, CMYK) still go to ImageIO
//! on macOS, the only host with it for now.

use pear_image::{Image, Source};
use std::ffi::c_void;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorSpaceCreateWithName(name: *const c_void) -> *mut c_void;
    fn CGColorSpaceCreateIndexed(base: *mut c_void, last_index: usize, table: *const u8) -> *mut c_void;
    fn CGColorSpaceRelease(space: *mut c_void);
    fn CGDataProviderCreateWithData(
        info: *mut c_void,
        data: *const c_void,
        size: usize,
        release: extern "C" fn(*mut c_void, *const c_void, usize),
    ) -> *mut c_void;
    fn CGDataProviderRelease(provider: *mut c_void);
    fn CGImageCreate(
        width: usize,
        height: usize,
        bits_per_component: usize,
        bits_per_pixel: usize,
        bytes_per_row: usize,
        space: *mut c_void,
        bitmap_info: u32,
        provider: *mut c_void,
        decode: *const f64,
        should_interpolate: bool,
        intent: i32,
    ) -> *mut c_void;
    static kCGColorSpaceSRGB: *const c_void;
    static kCGColorSpaceGenericGrayGamma2_2: *const c_void;
}

const ALPHA_NONE: u32 = 0;
const ALPHA_PREMULTIPLIED_LAST: u32 = 1;
const ALPHA_LAST: u32 = 3;
const ALPHA_NONE_SKIP_LAST: u32 = 5;

extern "C" fn free_pixels(info: *mut c_void, _data: *const c_void, _size: usize) {
    drop(unsafe { Box::from_raw(info as *mut Vec<u8>) });
}

/// Which decoder UIImage uses: ours unless PEAR_IMAGE_DECODER=imageio.
pub fn use_imageio_only() -> bool {
    static ONLY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ONLY.get_or_init(|| std::env::var("PEAR_IMAGE_DECODER").map_or(false, |v| v == "imageio"))
}

/// A +1 CGImage of `bytes`, or None when the file needs ImageIO (or is not
/// a PNG/JPEG pear-image decodes).
pub unsafe fn cgimage(bytes: &[u8]) -> Option<*mut c_void> {
    pear_image::sniff(bytes)?;
    let image = match pear_image::decode(bytes) {
        Ok(image) => image,
        Err(e) => {
            debug(&format!("pear-image: {e}; using ImageIO"));
            return None;
        }
    };
    if !image.notes.plain_srgb() || image.source == Source::Cmyk {
        debug(&format!("{:?} {:?}: using ImageIO", image.source, image.notes));
        return None;
    }
    Some(present(image))
}

fn debug(message: &str) {
    if std::env::var_os("PEAR_DEBUG_IMAGE").is_some() {
        let m = std::ffi::CString::new(message).unwrap_or_default();
        unsafe { crate::PearHostLogMessage(c"image".as_ptr(), m.as_ptr()) };
    }
}

unsafe fn present(image: Image) -> *mut c_void {
    let (w, h) = (image.width as usize, image.height as usize);
    let opaque = !image.has_alpha();
    let srgb = || CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    // (bytes, bits per pixel, alpha info, colour space)
    let (bytes, bpp, info, space): (Vec<u8>, usize, u32, *mut c_void) = match &image.source {
        Source::Gray => {
            (image.pixels.chunks_exact(4).map(|p| p[0]).collect(), 8, ALPHA_NONE, CGColorSpaceCreateWithName(kCGColorSpaceGenericGrayGamma2_2))
        }
        Source::GrayAlpha => (
            image.pixels.chunks_exact(4).flat_map(|p| [p[0], p[3]]).collect(),
            16,
            ALPHA_LAST,
            CGColorSpaceCreateWithName(kCGColorSpaceGenericGrayGamma2_2),
        ),
        Source::Indexed { palette, indices } => {
            let table: Vec<u8> = palette.iter().flatten().copied().collect();
            let base = srgb();
            let space = CGColorSpaceCreateIndexed(base, palette.len().saturating_sub(1), table.as_ptr());
            CGColorSpaceRelease(base);
            (indices.clone(), 8, ALPHA_NONE, space)
        }
        Source::Rgb | Source::Cmyk => (image.pixels, 32, ALPHA_NONE_SKIP_LAST, srgb()),
        Source::Rgba if image.premultiplied => {
            // CgBI: ImageIO drops the alpha channel of images that are opaque.
            (image.pixels, 32, if opaque { ALPHA_NONE_SKIP_LAST } else { ALPHA_PREMULTIPLIED_LAST }, srgb())
        }
        Source::Rgba => (image.pixels, 32, ALPHA_LAST, srgb()),
    };
    let row = w * bpp / 8;
    let boxed = Box::new(bytes);
    let (ptr, len) = (boxed.as_ptr() as *const c_void, boxed.len());
    let provider = CGDataProviderCreateWithData(Box::into_raw(boxed) as *mut c_void, ptr, len, free_pixels);
    let cg = CGImageCreate(w, h, 8, bpp, row, space, info, provider, std::ptr::null(), true, 0);
    CGDataProviderRelease(provider);
    CGColorSpaceRelease(space);
    cg
}
