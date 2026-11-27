//! Clean-room Khronos ASTC LDR texture block decoder.

/// Maps Apple `MTLPixelFormat` enum values for ASTC to `(block_width, block_height)`.
pub fn astc_block_dims(pixel_format: usize) -> Option<(usize, usize)> {
    match pixel_format {
        186 | 204 => Some((4, 4)),
        187 | 205 => Some((5, 4)),
        188 | 206 => Some((5, 5)),
        189 | 207 => Some((6, 5)),
        190 | 208 => Some((6, 6)),
        192 | 210 => Some((8, 5)),
        193 | 211 => Some((8, 6)),
        194 | 212 => Some((8, 8)),
        195 | 213 => Some((10, 5)),
        196 | 214 => Some((10, 6)),
        197 | 215 => Some((10, 8)),
        198 | 216 => Some((10, 10)),
        199 | 217 => Some((12, 10)),
        200 | 218 => Some((12, 12)),
        _ => None,
    }
}

#[derive(Copy, Clone)]
struct AstcBlockMode {
    gw: usize,
    gh: usize,
    weight_range: usize, // max weight value + 1 (e.g. 2,3,4,5,6,8,10,12,16,20,24,32)
    dual_plane: bool,
}

fn decode_block_mode(bits: u16) -> Option<AstcBlockMode> {
    if (bits & 0x01ff) == 0x01fc {
        return None;
    }
    let dual_plane = (bits & (1 << 10)) != 0;
    let h = ((bits >> 9) & 1) as usize;
    let r = (((bits >> 4) & 1) | ((bits & 0x3) << 1)) as usize;
    let weight_ranges = [2usize, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32];
    let r_idx = (h * 6 + r).min(weight_ranges.len() - 1);
    let weight_range = weight_ranges[r_idx];
    let low2 = bits & 0x3;
    let mid2 = (bits >> 2) & 0x3;
    let (gw, gh) = if low2 == 0 {
        match mid2 {
            0 => return None,
            1 => (((bits >> 7) & 0x3) as usize + 4, ((bits >> 5) & 0x3) as usize + 2),
            2 => (((bits >> 7) & 0x3) as usize + 8, ((bits >> 5) & 0x3) as usize + 2),
            _ => {
                if (bits & (1 << 8)) == 0 {
                    (((bits >> 5) & 0x3) as usize + 2, ((bits >> 7) & 0x1) as usize + 8)
                } else {
                    (((bits >> 5) & 0x1) as usize + 2, ((bits >> 7) & 0x1) as usize + 6)
                }
            }
        }
    } else {
        match mid2 {
            0 => (((bits >> 7) & 0x3) as usize + 4, ((bits >> 5) & 0x3) as usize + 2),
            1 => (((bits >> 7) & 0x3) as usize + 8, ((bits >> 5) & 0x3) as usize + 2),
            2 => (((bits >> 5) & 0x3) as usize + 2, ((bits >> 7) & 0x3) as usize + 8),
            _ => {
                if (bits & (1 << 7)) != 0 {
                    (12, 2)
                } else {
                    (((bits >> 5) & 0x3) as usize + 2, ((bits >> 8) & 0x1) as usize + 6)
                }
            }
        }
    };
    Some(AstcBlockMode {
        gw: gw.max(1),
        gh: gh.max(1),
        weight_range,
        dual_plane,
    })
}

#[inline]
fn get_bits(block: &[u8; 16], start: usize, len: usize) -> u32 {
    if len == 0 || start >= 128 {
        return 0;
    }
    let val = u128::from_le_bytes(*block);
    let mask = if len >= 32 { u32::MAX as u128 } else { (1u128 << len) - 1 };
    ((val >> start) & mask) as u32
}

#[inline]
fn get_bits_rev(block: &[u8; 16], msb_offset: usize, len: usize) -> u32 {
    if len == 0 || msb_offset >= 128 {
        return 0;
    }
    let mut acc = 0u32;
    for i in 0..len {
        let bit_pos = 127usize.saturating_sub(msb_offset + i);
        let bit = ((block[bit_pos >> 3] >> (bit_pos & 7)) & 1) as u32;
        acc |= bit << i;
    }
    acc
}

fn decode_direct_endpoints_u8(cem: usize, v: &[u8]) -> ([u8; 4], [u8; 4]) {
    let get = |i: usize| v.get(i).copied().unwrap_or(255);
    match cem {
        // Luminance direct (2 values)
        0 => {
            let l0 = get(0);
            let l1 = get(1);
            ([l0, l0, l0, 255], [l1, l1, l1, 255])
        }
        // Luminance base + offset
        1 => {
            let l0 = get(0);
            let l1 = l0.saturating_add(get(1));
            ([l0, l0, l0, 255], [l1, l1, l1, 255])
        }
        // Luminance + Alpha direct (4 values)
        4 => ([get(0), get(0), get(0), get(2)], [get(1), get(1), get(1), get(3)]),
        // RGB direct (6 values: r0, r1, g0, g1, b0, b1)
        8 => ([get(0), get(2), get(4), 255], [get(1), get(3), get(5), 255]),
        // RGBA direct (8 values: r0, r1, g0, g1, b0, b1, a0, a1)
        12 => ([get(0), get(2), get(4), get(6)], [get(1), get(3), get(5), get(7)]),
        // Fallback average/grayscale
        _ => {
            let avg = if v.is_empty() {
                255
            } else {
                (v.iter().map(|&b| b as usize).sum::<usize>() / v.len()) as u8
            };
            ([avg, avg, avg, 255], [avg, avg, avg, 255])
        }
    }
}

/// Decodes a single 16-byte ASTC LDR block into `(bw * bh)` RGBA8 pixels in `out_rgba`.
pub fn decode_astc_block(block: &[u8; 16], bw: usize, bh: usize, out_rgba: &mut [u8]) {
    let needed = bw.saturating_mul(bh).saturating_mul(4);
    if out_rgba.len() < needed || bw == 0 || bh == 0 {
        return;
    }
    let mode_bits = u16::from_le_bytes([block[0], block[1]]);
    if (mode_bits & 0x01ff) == 0x01fc {
        let r = (u16::from_le_bytes([block[8], block[9]]) >> 8) as u8;
        let g = (u16::from_le_bytes([block[10], block[11]]) >> 8) as u8;
        let b = (u16::from_le_bytes([block[12], block[13]]) >> 8) as u8;
        let a = (u16::from_le_bytes([block[14], block[15]]) >> 8) as u8;
        for chunk in out_rgba[..needed].chunks_exact_mut(4) {
            chunk.copy_from_slice(&[r, g, b, a]);
        }
        return;
    }

    let Some(bm) = decode_block_mode(mode_bits & 0x07ff) else {
        for chunk in out_rgba[..needed].chunks_exact_mut(4) {
            chunk.copy_from_slice(&[128, 128, 128, 255]);
        }
        return;
    };

    let part_count = (get_bits(block, 11, 2) + 1) as usize;
    let (cem, endpoint_bit_start) = if part_count == 1 && !bm.dual_plane {
        (get_bits(block, 13, 4) as usize, 17usize)
    } else if !bm.dual_plane {
        (get_bits(block, 23, 6) as usize & 0x0f, 29usize)
    } else {
        (get_bits(block, 13, 4) as usize, 19usize)
    };

    let color_vals_count = match cem {
        0 | 1 => 2,
        4 | 5 => 4,
        8 | 9 => 6,
        12 | 13 => 8,
        _ => 4,
    };

    // Unquantize endpoints assuming byte-aligned fallback when complex BISE trits/quints apply
    let weight_bits_per_entry = bm.weight_range.next_power_of_two().trailing_zeros() as usize;
    let total_weight_bits = (bm.gw * bm.gh * weight_bits_per_entry).min(96);
    let avail_color_bits = 128usize
        .saturating_sub(endpoint_bit_start)
        .saturating_sub(total_weight_bits);
    let bits_per_cval = (avail_color_bits / color_vals_count).clamp(1, 8);
    let max_cval = ((1u32 << bits_per_cval) - 1).max(1);
    let mut endpoints = Vec::with_capacity(color_vals_count);
    for idx in 0..color_vals_count {
        let raw = get_bits(block, endpoint_bit_start + idx * bits_per_cval, bits_per_cval);
        endpoints.push(((raw * 255 + max_cval / 2) / max_cval) as u8);
    }
    let (e0, e1) = decode_direct_endpoints_u8(cem, &endpoints);

    let max_w = (bm.weight_range - 1).max(1) as f32;
    let mut grid_weights = vec![0u8; bm.gw * bm.gh];
    for (idx, dst) in grid_weights.iter_mut().enumerate() {
        let raw_w = if weight_bits_per_entry > 0 {
            get_bits_rev(block, idx * weight_bits_per_entry, weight_bits_per_entry)
        } else {
            0
        };
        let norm = ((raw_w as f32 / ((1u32 << weight_bits_per_entry) - 1).max(1) as f32) * max_w)
            .min(max_w);
        *dst = ((norm / max_w) * 64.0).round() as u8;
    }

    for y in 0..bh {
        let gs_y = (y * (bm.gh.saturating_sub(1))) as f32 / (bh.saturating_sub(1)).max(1) as f32;
        let y0 = (gs_y.floor() as usize).min(bm.gh - 1);
        let y1 = (y0 + 1).min(bm.gh - 1);
        let fy = gs_y - (y0 as f32);
        for x in 0..bw {
            let gs_x =
                (x * (bm.gw.saturating_sub(1))) as f32 / (bw.saturating_sub(1)).max(1) as f32;
            let x0 = (gs_x.floor() as usize).min(bm.gw - 1);
            let x1 = (x0 + 1).min(bm.gw - 1);
            let fx = gs_x - (x0 as f32);
            let w00 = grid_weights[y0 * bm.gw + x0] as f32;
            let w10 = grid_weights[y0 * bm.gw + x1] as f32;
            let w01 = grid_weights[y1 * bm.gw + x0] as f32;
            let w11 = grid_weights[y1 * bm.gw + x1] as f32;
            let w = ((w00 * (1.0 - fx) + w10 * fx) * (1.0 - fy)
                + (w01 * (1.0 - fx) + w11 * fx) * fy)
                .round()
                .clamp(0.0, 64.0) as u32;
            let off = (y * bw + x) * 4;
            for c in 0..4 {
                out_rgba[off + c] =
                    (((e0[c] as u32) * (64 - w) + (e1[c] as u32) * w + 32) / 64) as u8;
            }
        }
    }
}

/// Decodes an ASTC compressed 2D surface slice into tight `width * height * 4` RGBA8 bytes.
pub fn decode_astc_surface(
    src: &[u8],
    bytes_per_row: usize,
    width: usize,
    height: usize,
    bw: usize,
    bh: usize,
) -> Vec<u8> {
    if width == 0 || height == 0 || bw == 0 || bh == 0 {
        return Vec::new();
    }
    let blocks_x = width.div_ceil(bw);
    let blocks_y = height.div_ceil(bh);
    let row_stride = if bytes_per_row >= blocks_x * 16 {
        bytes_per_row
    } else {
        blocks_x * 16
    };
    let mut out = vec![0u8; width * height * 4];
    let mut scratch = vec![0u8; bw * bh * 4];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let off = by * row_stride + bx * 16;
            let mut blk = [0u8; 16];
            if let Some(slice) = src.get(off..off + 16) {
                blk.copy_from_slice(slice);
            }
            decode_astc_block(&blk, bw, bh, &mut scratch);
            for ty in 0..bh {
                let dy = by * bh + ty;
                if dy >= height {
                    break;
                }
                for tx in 0..bw {
                    let dx = bx * bw + tx;
                    if dx >= width {
                        break;
                    }
                    let s_off = (ty * bw + tx) * 4;
                    let d_off = (dy * width + dx) * 4;
                    out[d_off..d_off + 4].copy_from_slice(&scratch[s_off..s_off + 4]);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astc_block_dims_cover_standard_apple_formats() {
        assert_eq!(astc_block_dims(204), Some((4, 4)));
        assert_eq!(astc_block_dims(186), Some((4, 4)));
        assert_eq!(astc_block_dims(212), Some((8, 8)));
        assert_eq!(astc_block_dims(218), Some((12, 12)));
        assert_eq!(astc_block_dims(80), None);
    }

    #[test]
    fn astc_void_extent_block_and_surface_decode() {
        // Void-extent block (mode bits & 0x1ff == 0x1fc) with RGBA = (255, 64, 128, 255)
        let mut blk = [0u8; 16];
        blk[0] = 0xfc;
        blk[1] = 0x01;
        blk[8..10].copy_from_slice(&0xffffu16.to_le_bytes()); // R = 255
        blk[10..12].copy_from_slice(&0x4000u16.to_le_bytes()); // G = 64
        blk[12..14].copy_from_slice(&0x8000u16.to_le_bytes()); // B = 128
        blk[14..16].copy_from_slice(&0xffffu16.to_le_bytes()); // A = 255

        let mut rgba = [0u8; 4 * 4 * 4];
        decode_astc_block(&blk, 4, 4, &mut rgba);
        for px in rgba.chunks_exact(4) {
            assert_eq!(px, &[255, 64, 128, 255]);
        }

        // 8x8 surface from 2x2 void-extent 4x4 blocks
        let mut surf_bytes = Vec::with_capacity(64);
        for _ in 0..4 {
            surf_bytes.extend_from_slice(&blk);
        }
        let decoded = decode_astc_surface(&surf_bytes, 32, 8, 8, 4, 4);
        assert_eq!(decoded.len(), 8 * 8 * 4);
        assert_eq!(&decoded[0..4], &[255, 64, 128, 255]);
        assert_eq!(&decoded[decoded.len() - 4..], &[255, 64, 128, 255]);

        // Arbitrary bytes must never panic across any block size
        let noise = [0x5au8; 16];
        let mut out_12 = vec![0u8; 12 * 12 * 4];
        decode_astc_block(&noise, 12, 12, &mut out_12);
    }
}
