//! PNG (ISO/IEC 15948) and Apple's CgBI variant that Xcode writes into iOS
//! app bundles ("crushed" PNGs: a CgBI chunk before IHDR, raw DEFLATE data
//! without the zlib wrapper, blue and red swapped, alpha premultiplied).

use crate::{inflate, Error, Image, Source};

pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

struct Header {
    width: u32,
    height: u32,
    depth: u8,
    color: u8,
    interlaced: bool,
}

impl Header {
    fn channels(&self) -> usize {
        match self.color {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => 0,
        }
    }
    /// Bytes per complete pixel, at least 1 (the filter unit).
    fn filter_bpp(&self) -> usize {
        ((self.channels() * self.depth as usize) + 7) / 8
    }
    fn row_bytes(&self, width: u32) -> usize {
        (width as usize * self.channels() * self.depth as usize + 7) / 8
    }
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

pub fn decode(data: &[u8]) -> Result<Image, Error> {
    if data.len() < 8 || data[..8] != SIGNATURE {
        return Err(Error::Corrupt("PNG signature"));
    }
    let mut pos = 8;
    let mut header: Option<Header> = None;
    let mut cgbi = false;
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut transparent: Option<[u16; 3]> = None; // gray or RGB key colour
    let mut idat = Vec::new();
    let mut gamma: Option<u32> = None; // gAMA × 100000
    let mut colour_managed = false; // sRGB or iCCP present: gAMA is a fallback only
    let mut icc = false;
    while pos + 8 <= data.len() {
        let len = be32(&data[pos..]) as usize;
        let kind = &data[pos + 4..pos + 8];
        let body = data.get(pos + 8..pos + 8 + len).ok_or(Error::Truncated)?;
        pos += 12 + len; // length, type, data, CRC (not checked, as ImageIO)
        match kind {
            b"CgBI" => cgbi = true,
            b"IHDR" => {
                if body.len() < 13 {
                    return Err(Error::Corrupt("IHDR"));
                }
                let h = Header {
                    width: be32(body),
                    height: be32(&body[4..]),
                    depth: body[8],
                    color: body[9],
                    interlaced: body[12] == 1,
                };
                let ok_depth = match h.color {
                    0 => matches!(h.depth, 1 | 2 | 4 | 8 | 16),
                    3 => matches!(h.depth, 1 | 2 | 4 | 8),
                    2 | 4 | 6 => matches!(h.depth, 8 | 16),
                    _ => false,
                };
                if !ok_depth || h.width == 0 || h.height == 0 {
                    return Err(Error::Corrupt("IHDR colour type / depth"));
                }
                if h.width as u64 * h.height as u64 > crate::MAX_PIXELS {
                    return Err(Error::Unsupported("image too large"));
                }
                header = Some(h);
            }
            b"PLTE" => {
                palette = body.chunks_exact(3).map(|c| [c[0], c[1], c[2], 255]).collect();
            }
            b"tRNS" => match header.as_ref().map(|h| h.color) {
                Some(3) => {
                    for (entry, &a) in palette.iter_mut().zip(body) {
                        entry[3] = a;
                    }
                }
                Some(0) if body.len() >= 2 => {
                    let g = u16::from_be_bytes([body[0], body[1]]);
                    transparent = Some([g, g, g]);
                }
                Some(2) if body.len() >= 6 => {
                    transparent = Some([
                        u16::from_be_bytes([body[0], body[1]]),
                        u16::from_be_bytes([body[2], body[3]]),
                        u16::from_be_bytes([body[4], body[5]]),
                    ]);
                }
                _ => {}
            },
            b"IDAT" => idat.extend_from_slice(body),
            b"gAMA" if body.len() >= 4 => gamma = Some(be32(body)),
            b"sRGB" => colour_managed = true,
            b"iCCP" => {
                colour_managed = true;
                icc = true;
            }
            b"IEND" => break,
            _ => {}
        }
    }
    let h = header.ok_or(Error::Corrupt("no IHDR"))?;
    if h.color == 3 && palette.is_empty() {
        return Err(Error::Corrupt("palette image without PLTE"));
    }

    let passes: Vec<(u32, u32, u32, u32, u32, u32)> = if h.interlaced {
        // (x0, y0, dx, dy) of the seven Adam7 passes, with their sizes.
        [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]
            .iter()
            .map(|&(x0, y0, dx, dy)| {
                let w = (h.width + dx - 1 - x0) / dx;
                let hh = (h.height + dy - 1 - y0) / dy;
                (x0, y0, dx, dy, if h.width > x0 { w } else { 0 }, if h.height > y0 { hh } else { 0 })
            })
            .collect()
    } else {
        vec![(0, 0, 1, 1, h.width, h.height)]
    };
    let expected: usize = passes
        .iter()
        .filter(|p| p.4 > 0 && p.5 > 0)
        .map(|p| (h.row_bytes(p.4) + 1) * p.5 as usize)
        .sum();
    let raw = if cgbi { inflate::inflate(&idat, expected)? } else { inflate::zlib_decompress(&idat, expected)? };
    if raw.len() < expected {
        return Err(Error::Truncated);
    }

    let mut image = Image::new(h.width, h.height);
    image.premultiplied = cgbi && h.color == 6;
    image.notes = crate::Notes { bit_depth: h.depth, icc_profile: icc, cgbi, gamma_converted: false };
    let has_key = transparent.is_some();
    let palette_alpha = palette.iter().any(|p| p[3] != 255);
    image.source = match h.color {
        0 if !has_key => Source::Gray,
        0 | 4 => Source::GrayAlpha,
        2 if !has_key => Source::Rgb,
        3 if !palette_alpha => Source::Indexed {
            palette: palette.iter().map(|p| [p[0], p[1], p[2]]).collect(),
            indices: vec![0; h.width as usize * h.height as usize],
        },
        _ => Source::Rgba,
    };
    let mut offset = 0;
    for &(x0, y0, dx, dy, pw, ph) in &passes {
        if pw == 0 || ph == 0 {
            continue;
        }
        let rb = h.row_bytes(pw);
        let mut prev = vec![0u8; rb];
        let mut cur = vec![0u8; rb];
        for py in 0..ph {
            let filter = raw[offset];
            cur.copy_from_slice(&raw[offset + 1..offset + 1 + rb]);
            offset += rb + 1;
            unfilter(filter, &mut cur, &prev, h.filter_bpp())?;
            let y = y0 + py * dy;
            if !h.interlaced && h.depth == 8 && transparent.is_none() && fast_row(&h, &cur, &palette, cgbi, &mut image, y) {
                std::mem::swap(&mut prev, &mut cur);
                continue;
            }
            for px in 0..pw {
                let x = x0 + px * dx;
                let rgba = pixel(&h, &cur, px as usize, &palette, transparent, cgbi);
                let at = y as usize * h.width as usize + x as usize;
                if let Source::Indexed { indices, .. } = &mut image.source {
                    indices[at] = sample(&cur, px as usize, h.depth) as u8;
                }
                let o = at * 4;
                image.pixels[o..o + 4].copy_from_slice(&rgba);
            }
            std::mem::swap(&mut prev, &mut cur);
        }
    }
    if let (Some(g), false) = (gamma, colour_managed) {
        image.notes.gamma_converted = !(g == 0 || (45400..=45500).contains(&g));
        apply_gamma(&mut image, g);
        if let Source::Indexed { palette, .. } = &mut image.source {
            // Keep the palette consistent with the corrected pixels.
            let mut fixed = Image::new(palette.len() as u32, 1);
            for (i, c) in palette.iter().enumerate() {
                fixed.pixels[i * 4..i * 4 + 3].copy_from_slice(c);
            }
            apply_gamma(&mut fixed, g);
            for (i, c) in palette.iter_mut().enumerate() {
                c.copy_from_slice(&fixed.pixels[i * 4..i * 4 + 3]);
            }
        }
    }
    Ok(image)
}

/// Converts samples encoded with file gamma `g` (gAMA, ×100000) to sRGB, as
/// ImageIO does: decode to linear light with exponent 1/g, re-encode with the
/// sRGB transfer curve. Values near sRGB's own 0.45455 are left alone.
fn apply_gamma(image: &mut Image, g: u32) {
    if g == 0 || (45400..=45500).contains(&g) {
        return;
    }
    let decode = 100000.0 / g as f64;
    let table: Vec<u8> = (0..256)
        .map(|v| {
            let linear = (v as f64 / 255.0).powf(decode);
            let srgb = if linear <= 0.0031308 { 12.92 * linear } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };
            (srgb * 255.0).round().clamp(0.0, 255.0) as u8
        })
        .collect();
    for p in image.pixels.chunks_exact_mut(4) {
        for c in &mut p[..3] {
            *c = table[*c as usize];
        }
    }
}

/// One 8-bit, non-interlaced row without a colour key, converted in bulk
/// (the common cases); false when the generic per-pixel path is needed.
fn fast_row(h: &Header, row: &[u8], palette: &[[u8; 4]], cgbi: bool, image: &mut Image, y: u32) -> bool {
    let w = h.width as usize;
    let start = y as usize * w;
    let out = &mut image.pixels[start * 4..(start + w) * 4];
    match h.color {
        6 if !cgbi => out.copy_from_slice(&row[..w * 4]),
        6 => {
            for (o, i) in out.chunks_exact_mut(4).zip(row.chunks_exact(4)) {
                o.copy_from_slice(&[i[2], i[1], i[0], i[3]]);
            }
        }
        2 => {
            for (o, i) in out.chunks_exact_mut(4).zip(row.chunks_exact(3)) {
                let (r, b) = if cgbi { (i[2], i[0]) } else { (i[0], i[2]) };
                o.copy_from_slice(&[r, i[1], b, 255]);
            }
        }
        3 => {
            for (o, &i) in out.chunks_exact_mut(4).zip(&row[..w]) {
                o.copy_from_slice(&palette.get(i as usize).copied().unwrap_or([0, 0, 0, 255]));
            }
            if let Source::Indexed { indices, .. } = &mut image.source {
                indices[start..start + w].copy_from_slice(&row[..w]);
            }
        }
        0 => {
            for (o, &g) in out.chunks_exact_mut(4).zip(&row[..w]) {
                o.copy_from_slice(&[g, g, g, 255]);
            }
        }
        4 => {
            for (o, i) in out.chunks_exact_mut(4).zip(row.chunks_exact(2)) {
                o.copy_from_slice(&[i[0], i[0], i[0], i[1]]);
            }
        }
        _ => return false,
    }
    true
}

fn unfilter(filter: u8, cur: &mut [u8], prev: &[u8], bpp: usize) -> Result<(), Error> {
    match filter {
        0 => {}
        1 => {
            for i in bpp..cur.len() {
                cur[i] = cur[i].wrapping_add(cur[i - bpp]);
            }
        }
        2 => {
            for i in 0..cur.len() {
                cur[i] = cur[i].wrapping_add(prev[i]);
            }
        }
        3 => {
            for i in 0..cur.len() {
                let left = if i >= bpp { cur[i - bpp] as u16 } else { 0 };
                cur[i] = cur[i].wrapping_add(((left + prev[i] as u16) / 2) as u8);
            }
        }
        4 => {
            for i in 0..cur.len() {
                let a = if i >= bpp { cur[i - bpp] as i16 } else { 0 };
                let b = prev[i] as i16;
                let c = if i >= bpp { prev[i - bpp] as i16 } else { 0 };
                let p = a + b - c;
                let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                let pred = if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c };
                cur[i] = cur[i].wrapping_add(pred as u8);
            }
        }
        _ => return Err(Error::Corrupt("PNG filter type")),
    }
    Ok(())
}

/// Sample `index` (of `depth` bits) of a row, as stored.
fn sample(row: &[u8], index: usize, depth: u8) -> u16 {
    match depth {
        16 => u16::from_be_bytes([row[index * 2], row[index * 2 + 1]]),
        8 => row[index] as u16,
        d => {
            let per_byte = 8 / d as usize;
            let byte = row[index / per_byte];
            let shift = 8 - d as usize * (index % per_byte + 1);
            ((byte >> shift) & ((1u8 << d) - 1)) as u16
        }
    }
}

/// A sample scaled to 8 bits (low depths replicate, 16-bit rounds).
fn to8(v: u16, depth: u8) -> u8 {
    match depth {
        16 => ((v as u32 * 255 + 32767) / 65535) as u8,
        8 => v as u8,
        d => (v as u32 * 255 / ((1u32 << d) - 1)) as u8,
    }
}

fn pixel(h: &Header, row: &[u8], x: usize, palette: &[[u8; 4]], key: Option<[u16; 3]>, cgbi: bool) -> [u8; 4] {
    let d = h.depth;
    match h.color {
        0 => {
            let g = sample(row, x, d);
            let a = if key.map_or(false, |k| k[0] == g) { 0 } else { 255 };
            let g8 = to8(g, d);
            [g8, g8, g8, a]
        }
        3 => {
            let i = sample(row, x, d) as usize;
            palette.get(i).copied().unwrap_or([0, 0, 0, 255])
        }
        4 => {
            let (g, a) = (to8(sample(row, x * 2, d), d), to8(sample(row, x * 2 + 1, d), d));
            [g, g, g, a]
        }
        2 => {
            let (r, g, b) = (sample(row, x * 3, d), sample(row, x * 3 + 1, d), sample(row, x * 3 + 2, d));
            let a = if key.map_or(false, |k| k == [r, g, b]) { 0 } else { 255 };
            let (r, b) = if cgbi { (b, r) } else { (r, b) };
            [to8(r, d), to8(g, d), to8(b, d), a]
        }
        _ => {
            let (r, g, b, a) =
                (sample(row, x * 4, d), sample(row, x * 4 + 1, d), sample(row, x * 4 + 2, d), sample(row, x * 4 + 3, d));
            let (r, b) = if cgbi { (b, r) } else { (r, b) };
            [to8(r, d), to8(g, d), to8(b, d), to8(a, d)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG of `raw` scanlines (filter bytes included), stored uncompressed.
    fn png(width: u32, height: u32, depth: u8, color: u8, interlace: u8, extra: &[(&[u8; 4], Vec<u8>)], raw: &[u8], cgbi: bool) -> Vec<u8> {
        let mut out = SIGNATURE.to_vec();
        let mut chunk = |kind: &[u8; 4], body: &[u8]| {
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(body);
            out.extend_from_slice(&[0; 4]); // CRC, not checked
        };
        if cgbi {
            chunk(b"CgBI", &[0x50, 0x00, 0x20, 0x06]);
        }
        let mut ihdr = width.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[depth, color, 0, 0, interlace]);
        chunk(b"IHDR", &ihdr);
        for (kind, body) in extra {
            chunk(kind, body);
        }
        // One stored DEFLATE block, with the zlib wrapper unless CgBI.
        let mut z = if cgbi { vec![] } else { vec![0x78, 0x01] };
        z.push(1);
        z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
        z.extend_from_slice(raw);
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        out
    }

    #[test]
    fn rgba_with_filters() {
        // Row 0 unfiltered, row 1 "up" (adds row 0).
        let raw = [0, 10, 20, 30, 255, 40, 50, 60, 128, 2, 1, 1, 1, 0, 1, 1, 1, 0];
        let img = decode(&png(2, 2, 8, 6, 0, &[], &raw, false)).unwrap();
        assert_eq!(img.source, Source::Rgba);
        assert_eq!(&img.pixels[..8], &[10, 20, 30, 255, 40, 50, 60, 128]);
        assert_eq!(&img.pixels[8..], &[11, 21, 31, 255, 41, 51, 61, 128]);
        assert!(!img.premultiplied);
    }

    #[test]
    fn palette_without_transparency_stays_indexed() {
        let plte = vec![255, 0, 0, 0, 255, 0, 0, 0, 255];
        // 2-bit indices 0,1,2 packed MSB first.
        let raw = [0, 0b0001_1000];
        let img = decode(&png(3, 1, 2, 3, 0, &[(b"PLTE", plte.clone())], &raw, false)).unwrap();
        assert_eq!(img.pixels, [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
        match img.source {
            Source::Indexed { palette, indices } => {
                assert_eq!(palette.len(), 3);
                assert_eq!(indices, [0, 1, 2]);
            }
            other => panic!("{other:?}"),
        }
        let with_alpha = decode(&png(3, 1, 2, 3, 0, &[(b"PLTE", plte), (b"tRNS", vec![255, 0])], &raw, false)).unwrap();
        assert_eq!(with_alpha.source, Source::Rgba);
        assert_eq!(with_alpha.pixels[7], 0);
    }

    #[test]
    fn gray_depths_and_sixteen_bit() {
        let img = decode(&png(4, 1, 1, 0, 0, &[], &[0, 0b1010_0000], false)).unwrap();
        assert_eq!(img.source, Source::Gray);
        let grays: Vec<u8> = img.pixels.chunks(4).map(|p| p[0]).collect();
        assert_eq!(grays, [255, 0, 255, 0]);
        let img = decode(&png(1, 1, 16, 2, 0, &[], &[0, 0xff, 0xff, 0x80, 0x00, 0x00, 0x01], false)).unwrap();
        assert_eq!(&img.pixels, &[255, 128, 0, 255]);
        assert_eq!(img.notes.bit_depth, 16);
    }

    #[test]
    fn adam7_places_every_pass() {
        // 3×3 gray, value = 10·y + x, written pass by pass.
        let v = |x: u8, y: u8| 10 * y + x;
        let raw = [
            0, v(0, 0), // pass 1: (0,0)
            0, v(2, 0), // pass 4: x=2,y=0  (passes 2 and 3 are empty at 3×3)
            0, v(0, 2), v(2, 2), // pass 5: y=2, x=0,2
            0, v(1, 0), 0, v(1, 2), // pass 6: x=1, y=0 and y=2
            0, v(0, 1), v(1, 1), v(2, 1), // pass 7: y=1
        ];
        let img = decode(&png(3, 3, 8, 0, 1, &[], &raw, false)).unwrap();
        let grays: Vec<u8> = img.pixels.chunks(4).map(|p| p[0]).collect();
        assert_eq!(grays, [0, 1, 2, 10, 11, 12, 20, 21, 22]);
    }

    #[test]
    fn cgbi_is_raw_deflate_bgra_premultiplied() {
        let raw = [0, 30, 20, 10, 128];
        let img = decode(&png(1, 1, 8, 6, 0, &[], &raw, true)).unwrap();
        assert_eq!(img.pixels, [10, 20, 30, 128]);
        assert!(img.premultiplied && img.notes.cgbi);
    }
}
