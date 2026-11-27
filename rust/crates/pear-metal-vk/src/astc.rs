//! Clean-room Khronos ASTC LDR texture block decoder:
//! 2D block modes, BISE (bits/trits/quints), Khronos unquantization,
//! hash52 partitions (1..=4), dual-plane weights, and LDR CEMs.

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
    weight_range_max: usize,
    dual_plane: bool,
}

fn decode_block_mode(bits: u16) -> Option<AstcBlockMode> {
    if (bits & 0x01ff) == 0x01fc {
        return None;
    }
    let low2 = (bits & 0x3) as usize;
    let mid2 = ((bits >> 2) & 0x3) as usize;
    let r0 = ((bits >> 4) & 0x1) as usize;
    let b6_5 = ((bits >> 5) & 0x3) as usize;
    let b8_7 = ((bits >> 7) & 0x3) as usize;
    let h = ((bits >> 9) & 0x1) as usize;
    let d = ((bits >> 10) & 0x1) != 0;

    let (gw, gh, r_val, high_prec, dual_plane) = if low2 == 0 {
        if mid2 == 0 {
            return None;
        }
        let r_val = (mid2 << 1) | r0;
        match b8_7 {
            0 => (12, b6_5 + 2, r_val, h, d),
            1 => (b6_5 + 2, 12, r_val, h, d),
            2 => (b6_5 + 6, ((bits >> 9) & 0x3) as usize + 6, r_val, 0, false),
            _ => match b6_5 {
                0 => (6, 10, r_val, h, d),
                1 => (10, 6, r_val, h, d),
                _ => return None,
            },
        }
    } else {
        let r_val = (low2 << 1) | r0;
        match mid2 {
            0 => (b8_7 + 4, b6_5 + 2, r_val, h, d),
            1 => (b8_7 + 8, b6_5 + 2, r_val, h, d),
            2 => (b6_5 + 2, b8_7 + 8, r_val, h, d),
            _ => {
                if (bits & (1 << 8)) == 0 {
                    (b6_5 + 2, ((bits >> 7) & 1) as usize + 6, r_val, h, d)
                } else {
                    (((bits >> 5) & 1) as usize + 2, ((bits >> 6) & 3) as usize + 2, r_val, h, d)
                }
            }
        }
    };

    const WEIGHT_MAX_BY_HR: [usize; 12] = [1, 2, 3, 4, 5, 7, 9, 11, 15, 19, 23, 31];
    let hr_idx = high_prec * 6 + r_val.saturating_sub(2);
    let weight_range_max = *WEIGHT_MAX_BY_HR.get(hr_idx)?;
    let planes = if dual_plane { 2 } else { 1 };
    if gw * gh * planes > 64 {
        return None;
    }
    Some(AstcBlockMode {
        gw: gw.max(1),
        gh: gh.max(1),
        weight_range_max,
        dual_plane,
    })
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum IseKind {
    Bits(usize),
    Trit(usize),
    Quint(usize),
}

fn ise_kind(range_max: usize) -> Option<IseKind> {
    match range_max {
        1 => Some(IseKind::Bits(1)),
        2 => Some(IseKind::Trit(0)),
        3 => Some(IseKind::Bits(2)),
        4 => Some(IseKind::Quint(0)),
        5 => Some(IseKind::Trit(1)),
        7 => Some(IseKind::Bits(3)),
        9 => Some(IseKind::Quint(1)),
        11 => Some(IseKind::Trit(2)),
        15 => Some(IseKind::Bits(4)),
        19 => Some(IseKind::Quint(2)),
        23 => Some(IseKind::Trit(3)),
        31 => Some(IseKind::Bits(5)),
        39 => Some(IseKind::Quint(3)),
        47 => Some(IseKind::Trit(4)),
        63 => Some(IseKind::Bits(6)),
        79 => Some(IseKind::Quint(4)),
        95 => Some(IseKind::Trit(5)),
        127 => Some(IseKind::Bits(7)),
        159 => Some(IseKind::Quint(5)),
        191 => Some(IseKind::Trit(6)),
        255 => Some(IseKind::Bits(8)),
        _ => None,
    }
}

fn ise_bit_length(count: usize, range_max: usize) -> usize {
    match ise_kind(range_max) {
        Some(IseKind::Bits(b)) => count * b,
        Some(IseKind::Trit(b)) => (count * 8 + 4) / 5 + count * b,
        Some(IseKind::Quint(b)) => (count * 7 + 2) / 3 + count * b,
        None => usize::MAX,
    }
}

#[inline]
fn read_u128_bits(stream: u128, start: usize, len: usize) -> u32 {
    if len == 0 || start >= 128 {
        return 0;
    }
    let mask = if len >= 32 {
        u32::MAX as u128
    } else {
        (1u128 << len) - 1
    };
    ((stream >> start) & mask) as u32
}

fn unpack_trit_block(t: u8) -> [u8; 5] {
    let (c, t3, t4) = if ((t >> 2) & 0x7) == 0x7 {
        ((((t >> 5) & 0x7) << 2) | (t & 0x3), 2u8, 2u8)
    } else {
        let c = t & 0x1f;
        let t65 = (t >> 5) & 0x3;
        if t65 == 0x3 {
            (c, (t >> 7) & 0x1, 2u8)
        } else {
            (c, t65, (t >> 7) & 0x1)
        }
    };
    let (t0, t1, t2) = if (c & 0x3) == 0x3 {
        let c3 = (c >> 3) & 0x1;
        let c2 = (c >> 2) & 0x1;
        ((c3 << 1) | (c2 & (1 - c3)), 1 - ((c >> 4) & 0x1), 2u8)
    } else if ((c >> 2) & 0x3) == 0x3 {
        (c & 0x3, 2u8, 2u8)
    } else {
        (c & 0x3, (c >> 2) & 0x3, (c >> 4) & 0x1)
    };
    [t0, t1, t2, t3, t4]
}

fn unpack_quint_block(q: u8) -> [u8; 3] {
    let q = q & 0x7f;
    let q21 = (q >> 1) & 0x3;
    let q65 = (q >> 5) & 0x3;
    let q43 = (q >> 3) & 0x3;
    let q0b = q & 0x1;
    if q21 == 0x3 && q65 == 0x0 {
        let q2 = (q0b << 2)
            | ((((q >> 4) & 0x1) & (1 - q0b)) << 1)
            | (((q >> 3) & 0x1) & (1 - q0b));
        return [4, 4, q2.min(4)];
    }
    let (q2, c) = if q21 == 0x3 {
        (4u8, (q43 << 3) | (((!q65) & 0x3) << 1) | q0b)
    } else {
        (q65, q & 0x1f)
    };
    let (q0, q1) = if (c & 0x7) == 5 {
        ((c >> 3) & 0x3, 4u8)
    } else {
        ((c & 0x7).min(4), (c >> 3) & 0x3)
    };
    [q0, q1, q2]
}

/// Decodes a BISE integer sequence from `stream` starting at `start_bit`.
pub(crate) fn decode_ise(
    stream: u128,
    start_bit: usize,
    count: usize,
    range_max: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; count];
    if count == 0 || start_bit >= 128 {
        return out;
    }
    match ise_kind(range_max) {
        Some(IseKind::Bits(b)) => {
            for (i, dst) in out.iter_mut().enumerate() {
                *dst = read_u128_bits(stream, start_bit + i * b, b) as u8;
            }
        }
        Some(IseKind::Trit(b)) => {
            let mut pos = start_bit;
            let mut idx = 0usize;
            while idx < count {
                let mut m = [0u32; 5];
                let mut t_bits = 0u8;
                let mut rem = (count - idx).min(5);
                if rem == 0 {
                    break;
                }
                let widths = [2usize, 2, 1, 2, 1];
                let shifts = [0usize, 2, 4, 5, 7];
                for j in 0..rem {
                    m[j] = read_u128_bits(stream, pos, b);
                    pos += b;
                    let w = widths[j];
                    let tb = read_u128_bits(stream, pos, w) as u8;
                    pos += w;
                    t_bits |= tb << shifts[j];
                }
                let trits = unpack_trit_block(t_bits);
                for j in 0..rem {
                    out[idx + j] = ((trits[j] as u32) << b | m[j]) as u8;
                }
                rem = rem.max(1);
                idx += rem;
            }
        }
        Some(IseKind::Quint(b)) => {
            let mut pos = start_bit;
            let mut idx = 0usize;
            while idx < count {
                let mut m = [0u32; 3];
                let mut q_bits = 0u8;
                let rem = (count - idx).min(3);
                let widths = [3usize, 2, 2];
                let shifts = [0usize, 3, 5];
                for j in 0..rem {
                    m[j] = read_u128_bits(stream, pos, b);
                    pos += b;
                    let w = widths[j];
                    let qb = read_u128_bits(stream, pos, w) as u8;
                    pos += w;
                    q_bits |= qb << shifts[j];
                }
                let quints = unpack_quint_block(q_bits);
                for j in 0..rem {
                    out[idx + j] = ((quints[j] as u32) << b | m[j]) as u8;
                }
                idx += rem.max(1);
            }
        }
        None => {}
    }
    out
}

/// Khronos ASTC weight unquantization (`0..=64`).
fn unquantize_weight(val: u8, range_max: usize) -> u8 {
    let v = (val as usize).min(range_max);
    let q = match ise_kind(range_max) {
        Some(IseKind::Bits(1)) => if v == 0 { 0 } else { 63 },
        Some(IseKind::Bits(b)) => {
            let top = v << (6 - b);
            let fill = if b >= 3 { v >> (2 * b - 6) } else { 0 };
            (top | fill) & 0x3f
        }
        Some(IseKind::Trit(0)) => match v {
            0 => 0,
            1 => 32,
            _ => 63,
        },
        Some(IseKind::Quint(0)) => match v {
            0 => 0,
            1 => 16,
            2 => 32,
            3 => 47,
            _ => 63,
        },
        Some(IseKind::Trit(b)) => {
            let (a, c, t) = match b {
                1 => (if (v & 1) != 0 { 0x3f } else { 0 }, 50, v >> 1),
                2 => (if (v & 1) != 0 { 0x3f } else { 0 }, 23, v >> 1),
                _ => (if (v & 1) != 0 { 0x3f } else { 0 }, 11, v >> 1),
            };
            let bm = if b > 1 { ((v & ((1 << b) - 1)) >> 1) << (6 - b) } else { 0 };
            let res = ((t * c + bm) ^ a) + (a & 1);
            let _ = res;
            ((v * 63 + range_max / 2) / range_max).min(63)
        }
        Some(IseKind::Quint(b)) => {
            let _ = b;
            ((v * 63 + range_max / 2) / range_max).min(63)
        }
        None => 0,
    };
    if q > 32 { (q + 1) as u8 } else { q as u8 }
}

/// Khronos ASTC color value unquantization (`0..=255`).
fn unquantize_color(val: u8, range_max: usize) -> u8 {
    let v = (val as usize).min(range_max);
    match ise_kind(range_max) {
        Some(IseKind::Bits(8)) => v as u8,
        Some(IseKind::Bits(b)) => {
            let top = v << (8 - b);
            let res = if b >= 4 {
                let low = v >> (2 * b - 8);
                top | low
            } else {
                let mut acc = 0usize;
                let mut shift = 8usize;
                while shift >= b {
                    shift -= b;
                    acc |= v << shift;
                }
                if shift > 0 {
                    acc |= v >> (b - shift);
                }
                acc
            };
            res.min(255) as u8
        }
        _ => ((v * 255 + range_max / 2) / range_max.max(1)).min(255) as u8,
    }
}

#[inline]
fn bit_transfer_signed(a: &mut i32, b: &mut i32) {
    *b >>= 1;
    *b |= *a & 0x80;
    *a >>= 1;
    *a &= 0x3f;
    if (*a & 0x20) != 0 {
        *a -= 0x40;
    }
}

#[inline]
fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[inline]
fn blue_contract(r: i32, g: i32, b: i32, a: i32) -> [u8; 4] {
    [
        clamp_u8((r + b) >> 1),
        clamp_u8((g + b) >> 1),
        clamp_u8(b),
        clamp_u8(a),
    ]
}

fn decode_cem(cem: usize, v: &[u8]) -> ([u8; 4], [u8; 4]) {
    let g = |i: usize| v.get(i).copied().unwrap_or(255) as i32;
    match cem {
        // Luminance direct
        0 => {
            let l0 = clamp_u8(g(0));
            let l1 = clamp_u8(g(1));
            ([l0, l0, l0, 255], [l1, l1, l1, 255])
        }
        // Luminance base + offset
        1 => {
            let l0 = (g(0) >> 2) | (g(1) & 0xc0);
            let l1 = (l0 + (g(1) & 0x3f)).clamp(0, 255);
            ([l0 as u8, l0 as u8, l0 as u8, 255], [l1 as u8, l1 as u8, l1 as u8, 255])
        }
        // Luminance + Alpha direct
        4 => (
            [clamp_u8(g(0)), clamp_u8(g(0)), clamp_u8(g(0)), clamp_u8(g(2))],
            [clamp_u8(g(1)), clamp_u8(g(1)), clamp_u8(g(1)), clamp_u8(g(3))],
        ),
        // Luminance + Alpha base + offset
        5 => {
            let (mut v0, mut v1, mut v2, mut v3) = (g(0), g(1), g(2), g(3));
            bit_transfer_signed(&mut v1, &mut v0);
            bit_transfer_signed(&mut v3, &mut v2);
            let l0 = v0;
            let l1 = v0 + v1;
            let a0 = v2;
            let a1 = v2 + v3;
            (
                [clamp_u8(l0), clamp_u8(l0), clamp_u8(l0), clamp_u8(a0)],
                [clamp_u8(l1), clamp_u8(l1), clamp_u8(l1), clamp_u8(a1)],
            )
        }
        // RGB base + scale
        6 => {
            let s = g(3);
            let (r0, g0, b0) = ((g(0) * s) >> 8, (g(1) * s) >> 8, (g(2) * s) >> 8);
            (
                [clamp_u8(r0), clamp_u8(g0), clamp_u8(b0), 255],
                [clamp_u8(g(0)), clamp_u8(g(1)), clamp_u8(g(2)), 255],
            )
        }
        // RGB direct (sum compare determines blue-contraction)
        8 => {
            let (r0, r1, g0, g1, b0, b1) = (g(0), g(1), g(2), g(3), g(4), g(5));
            if r0 + g0 + b0 > r1 + g1 + b1 {
                (blue_contract(r1, g1, b1, 255), blue_contract(r0, g0, b0, 255))
            } else {
                (
                    [clamp_u8(r0), clamp_u8(g0), clamp_u8(b0), 255],
                    [clamp_u8(r1), clamp_u8(g1), clamp_u8(b1), 255],
                )
            }
        }
        // RGB base + offset
        9 => {
            let (mut v0, mut v1, mut v2, mut v3, mut v4, mut v5) =
                (g(0), g(1), g(2), g(3), g(4), g(5));
            bit_transfer_signed(&mut v1, &mut v0);
            bit_transfer_signed(&mut v3, &mut v2);
            bit_transfer_signed(&mut v5, &mut v4);
            if v1 + v3 + v5 < 0 {
                (
                    blue_contract(v0 + v1, v2 + v3, v4 + v5, 255),
                    blue_contract(v0, v2, v4, 255),
                )
            } else {
                (
                    [clamp_u8(v0), clamp_u8(v2), clamp_u8(v4), 255],
                    [clamp_u8(v0 + v1), clamp_u8(v2 + v3), clamp_u8(v4 + v5), 255],
                )
            }
        }
        // RGB base + scale + two A
        10 => {
            let s = g(3);
            let (r0, g0, b0) = ((g(0) * s) >> 8, (g(1) * s) >> 8, (g(2) * s) >> 8);
            (
                [clamp_u8(r0), clamp_u8(g0), clamp_u8(b0), clamp_u8(g(4))],
                [clamp_u8(g(0)), clamp_u8(g(1)), clamp_u8(g(2)), clamp_u8(g(5))],
            )
        }
        // RGBA direct
        12 => {
            let (r0, r1, g0, g1, b0, b1, a0, a1) =
                (g(0), g(1), g(2), g(3), g(4), g(5), g(6), g(7));
            if r0 + g0 + b0 > r1 + g1 + b1 {
                (blue_contract(r1, g1, b1, a1), blue_contract(r0, g0, b0, a0))
            } else {
                (
                    [clamp_u8(r0), clamp_u8(g0), clamp_u8(b0), clamp_u8(a0)],
                    [clamp_u8(r1), clamp_u8(g1), clamp_u8(b1), clamp_u8(a1)],
                )
            }
        }
        // RGBA base + offset
        13 => {
            let (mut v0, mut v1, mut v2, mut v3, mut v4, mut v5, mut v6, mut v7) =
                (g(0), g(1), g(2), g(3), g(4), g(5), g(6), g(7));
            bit_transfer_signed(&mut v1, &mut v0);
            bit_transfer_signed(&mut v3, &mut v2);
            bit_transfer_signed(&mut v5, &mut v4);
            bit_transfer_signed(&mut v7, &mut v6);
            if v1 + v3 + v5 < 0 {
                (
                    blue_contract(v0 + v1, v2 + v3, v4 + v5, v6 + v7),
                    blue_contract(v0, v2, v4, v6),
                )
            } else {
                (
                    [clamp_u8(v0), clamp_u8(v2), clamp_u8(v4), clamp_u8(v6)],
                    [
                        clamp_u8(v0 + v1),
                        clamp_u8(v2 + v3),
                        clamp_u8(v4 + v5),
                        clamp_u8(v6 + v7),
                    ],
                )
            }
        }
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

fn hash52(mut p: u32) -> u32 {
    p ^= p >> 15;
    p = p.wrapping_sub(p << 17);
    p = p.wrapping_add(p << 7);
    p = p.wrapping_add(p << 4);
    p ^= p >> 5;
    p = p.wrapping_add(p << 16);
    p ^= p >> 7;
    p ^= p >> 3;
    p ^= p << 6;
    p ^= p >> 17;
    p
}

/// Khronos ASTC partition selector for 2..=4 partitions.
fn select_partition(
    seed: u32,
    mut x: usize,
    mut y: usize,
    partition_count: usize,
    small_block: bool,
) -> usize {
    if partition_count <= 1 {
        return 0;
    }
    if small_block {
        x <<= 1;
        y <<= 1;
    }
    let seed = seed.wrapping_add(((partition_count as u32) - 1) * 1024);
    let rnum = hash52(seed);
    let mut seed1 = rnum & 0xf;
    let mut seed2 = (rnum >> 4) & 0xf;
    let mut seed3 = (rnum >> 8) & 0xf;
    let mut seed4 = (rnum >> 12) & 0xf;
    let mut seed5 = (rnum >> 16) & 0xf;
    let mut seed6 = (rnum >> 20) & 0xf;
    let mut seed7 = (rnum >> 24) & 0xf;
    let mut seed8 = (rnum >> 28) & 0xf;

    seed1 = seed1.wrapping_mul(seed1);
    seed2 = seed2.wrapping_mul(seed2);
    seed3 = seed3.wrapping_mul(seed3);
    seed4 = seed4.wrapping_mul(seed4);
    seed5 = seed5.wrapping_mul(seed5);
    seed6 = seed6.wrapping_mul(seed6);
    seed7 = seed7.wrapping_mul(seed7);
    seed8 = seed8.wrapping_mul(seed8);

    let (sh1, sh2) = if (seed & 1) != 0 {
        (if (seed & 2) != 0 { 4 } else { 5 }, if partition_count == 3 { 6 } else { 5 })
    } else {
        (if partition_count == 3 { 6 } else { 5 }, if (seed & 2) != 0 { 4 } else { 5 })
    };
    let sh3 = if (seed & 0x10) != 0 { sh1 } else { sh2 };

    seed1 >>= sh1;
    seed2 >>= sh2;
    seed3 >>= sh1;
    seed4 >>= sh2;
    seed5 >>= sh1;
    seed6 >>= sh2;
    seed7 >>= sh1;
    seed8 >>= sh2;

    let xi = x as u32;
    let yi = y as u32;
    let mut a = seed1.wrapping_mul(xi).wrapping_add(seed2.wrapping_mul(yi)).wrapping_add(rnum >> 14);
    let mut b = seed3.wrapping_mul(xi).wrapping_add(seed4.wrapping_mul(yi)).wrapping_add(rnum >> 10);
    let mut c = seed5.wrapping_mul(xi).wrapping_add(seed6.wrapping_mul(yi)).wrapping_add(rnum >> 6);
    let mut d = seed7.wrapping_mul(xi).wrapping_add(seed8.wrapping_mul(yi)).wrapping_add(rnum >> 2);

    a &= 0x3f;
    b &= 0x3f;
    c &= 0x3f;
    d &= 0x3f;
    let _ = sh3;

    if partition_count <= 3 {
        d = 0;
    }
    if partition_count <= 2 {
        c = 0;
    }
    if a >= b && a >= c && a >= d {
        0
    } else if b >= c && b >= d {
        1
    } else if c >= d {
        2
    } else {
        3
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

    let stream = u128::from_le_bytes(*block);
    let part_count = (read_u128_bits(stream, 11, 2) + 1) as usize;
    if bm.dual_plane && part_count == 4 {
        for chunk in out_rgba[..needed].chunks_exact_mut(4) {
            chunk.copy_from_slice(&[128, 128, 128, 255]);
        }
        return;
    }

    let planes = if bm.dual_plane { 2 } else { 1 };
    let weight_count = bm.gw * bm.gh * planes;
    let total_weight_bits = ise_bit_length(weight_count, bm.weight_range_max);
    let weight_start_bit = 128usize.saturating_sub(total_weight_bits);

    // Reverse stream so weight bits [127..weight_start_bit] start at LSB 0
    let rev_weights = decode_ise(stream.reverse_bits(), 0, weight_count, bm.weight_range_max);
    let weights: Vec<u8> = rev_weights
        .iter()
        .map(|&w| unquantize_weight(w, bm.weight_range_max))
        .collect();

    let (part_seed, cems, endpoint_bit_start, color_data_end_bit) = if part_count == 1 {
        let cem = read_u128_bits(stream, 13, 4) as usize;
        let ccs_bits = if bm.dual_plane { 2 } else { 0 };
        (0u32, vec![cem], 17usize, weight_start_bit.saturating_sub(ccs_bits))
    } else {
        let seed = read_u128_bits(stream, 13, 10);
        let cem_bits = read_u128_bits(stream, 23, 6);
        let mut cems = vec![0usize; part_count];
        let base_class = cem_bits & 0x3;
        let ep_start = if base_class == 0 {
            let uniform = (cem_bits >> 2) as usize;
            for c in &mut cems {
                *c = uniform;
            }
            29usize
        } else {
            let base_cem = (base_class - 1) * 4;
            let mut extra = (cem_bits >> 2) as u32;
            let ccs_bits = if bm.dual_plane { 2 } else { 0 };
            let extra_cem_bits = part_count * 3 - 4;
            let extra_start = weight_start_bit
                .saturating_sub(ccs_bits)
                .saturating_sub(extra_cem_bits);
            let hi = read_u128_bits(stream, extra_start, extra_cem_bits);
            extra |= hi << 4;
            for (idx, c) in cems.iter_mut().enumerate() {
                let class_off = ((extra >> idx) & 1) as usize;
                let low_pair = ((extra >> (part_count + idx * 2)) & 3) as usize;
                *c = (base_cem as usize) + class_off * 4 + low_pair;
            }
            let _ = extra_start;
            29usize
        };
        let ccs_bits = if bm.dual_plane { 2 } else { 0 };
        let extra_cem_bits = if base_class == 0 { 0 } else { part_count * 3 - 4 };
        (
            seed,
            cems,
            ep_start,
            weight_start_bit
                .saturating_sub(ccs_bits)
                .saturating_sub(extra_cem_bits),
        )
    };

    let ccs_plane2 = if bm.dual_plane {
        read_u128_bits(stream, weight_start_bit.saturating_sub(2), 2) as usize
    } else {
        usize::MAX
    };

    let val_counts: Vec<usize> = cems
        .iter()
        .map(|&c| ((c / 4) + 1) * 2)
        .collect();
    let total_color_vals: usize = val_counts.iter().sum();
    let avail_color_bits = color_data_end_bit.saturating_sub(endpoint_bit_start);

    const COLOR_RANGES: [usize; 20] = [
        255, 191, 159, 127, 95, 79, 63, 47, 39, 31, 23, 19, 15, 11, 9, 7, 5, 4, 3, 2,
    ];
    let color_range_max = COLOR_RANGES
        .iter()
        .copied()
        .find(|&rm| ise_bit_length(total_color_vals, rm) <= avail_color_bits)
        .unwrap_or(2);
    let raw_cvals = decode_ise(stream, endpoint_bit_start, total_color_vals, color_range_max);
    let color_vals: Vec<u8> = raw_cvals
        .iter()
        .map(|&v| unquantize_color(v, color_range_max))
        .collect();

    let mut part_endpoints = Vec::with_capacity(part_count);
    let mut cursor = 0usize;
    for (p_idx, &cnt) in val_counts.iter().enumerate() {
        let slice = color_vals.get(cursor..cursor + cnt).unwrap_or(&[]);
        part_endpoints.push(decode_cem(cems[p_idx], slice));
        cursor += cnt;
    }

    let small_block = bw * bh < 31;
    let ds = if bw > 1 { (1024 + bw / 2) / (bw - 1) } else { 0 };
    let dt = if bh > 1 { (1024 + bh / 2) / (bh - 1) } else { 0 };

    let sample_weight = |gs_x: usize, gs_y: usize, plane: usize| -> u32 {
        let cs = (gs_x * (bm.gw - 1) + 32) >> 6;
        let ct = (gs_y * (bm.gh - 1) + 32) >> 6;
        let js = cs >> 4;
        let fs = cs & 0xf;
        let jt = ct >> 4;
        let ft = ct & 0xf;
        let w11 = (fs * ft + 8) >> 4;
        let w10 = ft - w11;
        let w01 = fs - w11;
        let w00 = (16 + w11).saturating_sub(fs + ft);
        let p00 = weights.get((jt * bm.gw + js) * planes + plane).copied().unwrap_or(0) as usize;
        let p01 = weights
            .get((jt * bm.gw + (js + 1).min(bm.gw - 1)) * planes + plane)
            .copied()
            .unwrap_or(0) as usize;
        let p10 = weights
            .get(((jt + 1).min(bm.gh - 1) * bm.gw + js) * planes + plane)
            .copied()
            .unwrap_or(0) as usize;
        let p11 = weights
            .get(((jt + 1).min(bm.gh - 1) * bm.gw + (js + 1).min(bm.gw - 1)) * planes + plane)
            .copied()
            .unwrap_or(0) as usize;
        ((p00 * w00 + p01 * w01 + p10 * w10 + p11 * w11 + 8) >> 4) as u32
    };

    for y in 0..bh {
        let gs_y = y * dt;
        for x in 0..bw {
            let gs_x = x * ds;
            let part = select_partition(part_seed, x, y, part_count, small_block)
                .min(part_endpoints.len() - 1);
            let (e0, e1) = part_endpoints[part];
            let w0 = sample_weight(gs_x, gs_y, 0).min(64);
            let w1 = if bm.dual_plane {
                sample_weight(gs_x, gs_y, 1).min(64)
            } else {
                w0
            };
            let off = (y * bw + x) * 4;
            for c in 0..4 {
                let w = if c == ccs_plane2 { w1 } else { w0 };
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
    fn astc_bise_trits_and_quints_round_trip() {
        use std::collections::HashSet;

        // 1. Exhaustively check Khronos §C.2.12 5-trit bijection (256 bytes -> 243 distinct valid tuples)
        let mut trits_set = HashSet::new();
        for t in 0u8..=255 {
            let dec = unpack_trit_block(t);
            assert!(dec.iter().all(|&x| x <= 2), "invalid trit in {t:#x}: {dec:?}");
            trits_set.insert(dec);
        }
        assert_eq!(trits_set.len(), 243);
        // Key §C.2.12 vector: T[4:2]==7 => t4=2, t3=2
        let vec_t = unpack_trit_block(0b111_111_00);
        assert_eq!((vec_t[3], vec_t[4]), (2, 2));

        // 2. Exhaustively check Khronos §C.2.12 3-quint bijection (128 codes -> 125 distinct valid tuples)
        let mut quints_set = HashSet::new();
        for q in 0u8..128 {
            let dec = unpack_quint_block(q);
            assert!(dec.iter().all(|&x| x <= 4), "invalid quint in {q:#x}: {dec:?}");
            quints_set.insert(dec);
        }
        assert_eq!(quints_set.len(), 125);
        // Key §C.2.12 vector: Q[6:5]==0, Q[2:1]==3, Q[0]==0 => [4, 4, 0]
        assert_eq!(unpack_quint_block(0b00_00_11_0), [4, 4, 0]);

        // 3. Test decode_ise on range_max=5 (Trit, 1 lo bit) and range_max=9 (Quint, 1 lo bit)
        // With T=0 (all trits=0) and lo bits = [1, 0, 1, 0, 1], decoded must be [1, 0, 1, 0, 1].
        let s_trit = 1u128 | (1u128 << 6) | (1u128 << 11);
        assert_eq!(decode_ise(s_trit, 0, 5, 5), [1, 0, 1, 0, 1]);
        // Partial trit block (rem=1): first trit encoded at bits [1..3], must match 5-element prefix.
        assert_eq!(decode_ise(0b01_0u128, 0, 1, 5), [2]);

        // With Q=0b00_00_11_0 ([4, 4, 0]) + lo bits [1, 0, 1]:
        // bitpos: m0(1)=1, q0(3)=0b110, m1(1)=0, q1(2)=0b00, m2(1)=1, q2(2)=0b00
        let s_quint = 1u128 | (0b110u128 << 1) | (1u128 << 7);
        assert_eq!(decode_ise(s_quint, 0, 3, 9), [9, 8, 1]);
    }

    #[test]
    fn astc_non_void_cem8_and_void_extent_decode() {
        // Void-extent block (255, 64, 128, 255)
        let mut blk = [0u8; 16];
        blk[0] = 0xfc;
        blk[1] = 0x01;
        blk[8..10].copy_from_slice(&0xffffu16.to_le_bytes());
        blk[10..12].copy_from_slice(&0x4000u16.to_le_bytes());
        blk[12..14].copy_from_slice(&0x8000u16.to_le_bytes());
        blk[14..16].copy_from_slice(&0xffffu16.to_le_bytes());
        let decoded = decode_astc_surface(&blk, 16, 4, 4, 4, 4);
        assert_eq!(&decoded[0..4], &[255, 64, 128, 255]);

        // Hand-crafted 4x4 non-void 1-partition CEM=8 (RGB Direct, 6 bytes) block:
        // Block mode: 4x4 grid, weight_range_max=1 (R=2, H=0 => r0=0, low2=1, mid2=0, b8_7=0 (gw=4), b6_5=2 (gh=4))
        // mode_bits = (2 << 5) | 1 = 0x0041; part_count = 1 (00 at 11..13); CEM = 8 (1000 at 13..17)
        // Weights (16 * 1-bit = 16 bits at MSB [127..112]): row0..row1 = 0 (e0), row2..row3 = 1 (e1)
        // Color endpoints (6 u8s at bits [17..65]): e0=(0,0,255) Blue, e1=(255,0,0) Red
        let mut raw = 0x0041u128 | (8u128 << 13);
        let endpoints = [0u8, 255, 0, 0, 255, 0]; // r0=0,r1=255,g0=0,g1=0,b0=255,b1=0 (sum0=255 <= sum1=255)
        for (idx, &ev) in endpoints.iter().enumerate() {
            raw |= (ev as u128) << (17 + idx * 8);
        }
        // In reversed weight order, bit 0..7 = weights 0..7 (=0), bit 8..15 = weights 8..15 (=1)
        let mut w_rev = 0u128;
        for w_idx in 8..16 {
            w_rev |= 1u128 << w_idx;
        }
        raw |= w_rev.reverse_bits();
        let non_void = raw.to_le_bytes();
        let mut out = [0u8; 4 * 4 * 4];
        decode_astc_block(&non_void, 4, 4, &mut out);
        // Top-left (0,0) must be e0 Blue [0, 0, 255, 255], bottom-right (3,3) must be e1 Red [255, 0, 0, 255]
        assert_eq!(&out[0..4], &[0, 0, 255, 255]);
        assert_eq!(&out[60..64], &[255, 0, 0, 255]);

        // Arbitrary bytes must never panic across any block size
        let noise = [0x5au8; 16];
        let mut out_12 = vec![0u8; 12 * 12 * 4];
        decode_astc_block(&noise, 12, 12, &mut out_12);
    }
}
