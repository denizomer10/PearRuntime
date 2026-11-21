//! Clean-room: from the public specifications (RFC 1950/1951, PNG ISO/IEC
//! 15948, JPEG ITU T.81 with JFIF/Adobe markers); Apple's CgBI PNG variant
//! from its observed file structure in app bundles. Pixel output compared
//! with ImageIO and Chromium as black boxes.
//!
//! PearRuntime's own image decoders, in portable Rust with no dependencies:
//! what UIImage needs from ImageIO, decoded the same way on every host.
//!
//! - PNG: every colour type and bit depth, palettes and `tRNS`, Adam7, and
//!   Apple's CgBI ("crushed") variant found in iOS app bundles;
//! - JPEG: baseline and progressive, Huffman coded, any sampling factors,
//!   grayscale, YCbCr and Adobe CMYK/YCCK.
//!
//! Output is 8-bit RGBA ([`Image`]). Embedded colour profiles and gamma are
//! not applied (sRGB is assumed), and EXIF orientation is left to the caller,
//! as `CGImageSourceCreateImageAtIndex` leaves it.

pub mod inflate;
pub mod jpeg;
pub mod png;

/// Decoders refuse images above this many pixels (16384²).
pub const MAX_PIXELS: u64 = 1 << 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The data ends before the image does.
    Truncated,
    /// The data is not valid for its format.
    Corrupt(&'static str),
    /// Valid, but a feature this decoder does not implement.
    Unsupported(&'static str),
    /// Not a format this crate decodes.
    UnknownFormat,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Truncated => write!(f, "truncated image data"),
            Error::Corrupt(what) => write!(f, "corrupt image data: {what}"),
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
            Error::UnknownFormat => write!(f, "unknown image format"),
        }
    }
}

impl std::error::Error for Error {}

/// How the file stores its pixels, so a caller can present the image the
/// way the platform's own decoder would (CoreGraphics keeps gray images gray
/// and palette images indexed, for instance).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Gray,
    GrayAlpha,
    Rgb,
    Rgba,
    /// Palette colours and one index per pixel (palette PNGs without
    /// transparency; with `tRNS` they are reported as `Rgba`).
    Indexed { palette: Vec<[u8; 3]>, indices: Vec<u8> },
    /// Adobe CMYK or YCCK JPEG (converted to RGB in `pixels`).
    Cmyk,
}

/// What the file says about colour beyond its samples.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notes {
    /// Bits per sample in the file (16-bit PNGs are reduced to 8 here).
    pub bit_depth: u8,
    /// An embedded ICC profile (PNG iCCP, JPEG APP2), not applied.
    pub icc_profile: bool,
    /// Apple's CgBI PNG variant.
    pub cgbi: bool,
    /// A PNG gAMA other than sRGB's was converted to sRGB in `pixels`.
    pub gamma_converted: bool,
}

impl Notes {
    /// Whether `pixels` are the file's own samples in sRGB with nothing left
    /// unapplied, so presenting them as sRGB loses nothing.
    pub fn plain_srgb(&self) -> bool {
        self.bit_depth <= 8 && !self.icc_profile && !self.gamma_converted
    }
}

/// A decoded image: `width * height` pixels, row by row from the top, four
/// bytes each (R, G, B, A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Colour channels are already multiplied by alpha (CgBI PNGs store
    /// them so); otherwise alpha is straight.
    pub premultiplied: bool,
    pub source: Source,
    pub notes: Notes,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Image {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
            premultiplied: false,
            source: Source::Rgba,
            notes: Notes { bit_depth: 8, ..Notes::default() },
        }
    }

    /// Whether any pixel is not fully opaque.
    pub fn has_alpha(&self) -> bool {
        self.pixels.chunks_exact(4).any(|p| p[3] != 255)
    }

    /// Converts straight alpha to premultiplied (rounding as CoreGraphics).
    pub fn premultiply(&mut self) {
        if self.premultiplied {
            return;
        }
        for p in self.pixels.chunks_exact_mut(4) {
            let a = p[3] as u32;
            if a != 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
        }
        self.premultiplied = true;
    }
}

/// The format of `data`, by signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
}

pub fn sniff(data: &[u8]) -> Option<Format> {
    if data.starts_with(&png::SIGNATURE) {
        Some(Format::Png)
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(Format::Jpeg)
    } else {
        None
    }
}

/// Decodes a PNG or JPEG image.
pub fn decode(data: &[u8]) -> Result<Image, Error> {
    match sniff(data) {
        Some(Format::Png) => png::decode(data),
        Some(Format::Jpeg) => jpeg::decode(data),
        None => Err(Error::UnknownFormat),
    }
}
