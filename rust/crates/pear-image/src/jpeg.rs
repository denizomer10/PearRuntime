//! JPEG (ITU T.81): baseline and progressive DCT with Huffman coding, any
//! sampling factors, 1 (grayscale), 3 (YCbCr, or RGB per Adobe) or 4 (Adobe
//! CMYK/YCCK) components. Arithmetic coding, lossless and hierarchical modes
//! are reported as unsupported.

use crate::{Error, Image, Source};

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21,
    28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61,
    54, 47, 55, 62, 63,
];

#[derive(Clone)]
struct Huffman {
    /// (code length, symbol) for the next 9 bits; length 0 = slow path.
    fast: Vec<(u8, u8)>,
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    values: Vec<u8>,
}

const FAST: u32 = 9;

impl Huffman {
    fn new(counts: &[u8; 16], values: &[u8]) -> Result<Self, Error> {
        let mut fast = vec![(0u8, 0u8); 1 << FAST];
        let mut maxcode = [-1i32; 18];
        let mut valptr = [0i32; 17];
        let mut mincode = [0i32; 17];
        let mut code = 0i32;
        let mut k = 0usize;
        for len in 1..=16 {
            let n = counts[len - 1] as usize;
            valptr[len] = k as i32;
            mincode[len] = code;
            for _ in 0..n {
                if k >= values.len() {
                    return Err(Error::Corrupt("Huffman table"));
                }
                if len as u32 <= FAST {
                    let shift = FAST - len as u32;
                    let base = (code as usize) << shift;
                    for i in 0..(1usize << shift) {
                        fast[base + i] = (len as u8, values[k]);
                    }
                }
                code += 1;
                k += 1;
            }
            maxcode[len] = if n > 0 { code - 1 } else { -1 };
            if code > (1 << len) {
                return Err(Error::Corrupt("Huffman table"));
            }
            code <<= 1;
        }
        maxcode[17] = i32::MAX;
        Ok(Huffman { fast, maxcode, valptr, mincode, values: values.to_vec() })
    }
}

/// Entropy-coded data reader: MSB first, 0xFF00 stuffing, stops at markers.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    count: u32,
    /// A marker was reached; zeros are fed from here on.
    marker: Option<u8>,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Reader { data, pos, buf: 0, count: 0, marker: None }
    }

    fn fill(&mut self) {
        while self.count <= 24 {
            let mut byte = 0u8;
            if self.marker.is_none() && self.pos < self.data.len() {
                byte = self.data[self.pos];
                if byte == 0xff {
                    let next = self.data.get(self.pos + 1).copied().unwrap_or(0xd9);
                    if next == 0 {
                        self.pos += 2;
                    } else {
                        self.marker = Some(next);
                        byte = 0;
                    }
                } else {
                    self.pos += 1;
                }
            }
            self.buf |= (byte as u32) << (24 - self.count);
            self.count += 8;
        }
    }

    #[inline]
    fn bits(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        if self.count < n {
            self.fill();
        }
        let v = self.buf >> (32 - n);
        self.buf <<= n;
        self.count -= n;
        v
    }

    fn bit(&mut self) -> u32 {
        self.bits(1)
    }

    /// A signed value of `n` bits (JPEG's EXTEND).
    fn receive_extend(&mut self, n: u32) -> i32 {
        if n == 0 {
            return 0;
        }
        let v = self.bits(n) as i32;
        if v < (1 << (n - 1)) {
            v - (1 << n) + 1
        } else {
            v
        }
    }

    fn decode(&mut self, h: &Huffman) -> Result<u8, Error> {
        if self.count < 16 {
            self.fill();
        }
        let (len, sym) = h.fast[(self.buf >> (32 - FAST)) as usize];
        if len != 0 {
            self.buf <<= len;
            self.count -= len as u32;
            return Ok(sym);
        }
        let mut code = 0i32;
        for len in 1..=16 {
            code = (code << 1) | self.bit() as i32;
            if code <= h.maxcode[len] {
                let idx = h.valptr[len] + code - h.mincode[len];
                return h.values.get(idx as usize).copied().ok_or(Error::Corrupt("Huffman code"));
            }
        }
        Err(Error::Corrupt("Huffman code"))
    }

    /// Restart: drop buffered bits and step over the RSTn marker.
    fn restart(&mut self) {
        self.buf = 0;
        self.count = 0;
        if let Some(m) = self.marker {
            if (0xd0..=0xd7).contains(&m) {
                self.pos += 2;
                self.marker = None;
            }
        } else {
            // Not yet reached: find it (skip padding).
            while self.pos + 1 < self.data.len() {
                if self.data[self.pos] == 0xff && (0xd0..=0xd7).contains(&self.data[self.pos + 1]) {
                    self.pos += 2;
                    break;
                }
                self.pos += 1;
            }
        }
    }

    /// Position after the scan: at the marker that ended it.
    fn end(&self) -> usize {
        let mut p = self.pos;
        while p + 1 < self.data.len() && !(self.data[p] == 0xff && self.data[p + 1] != 0 && self.data[p + 1] != 0xff) {
            p += 1;
        }
        p
    }
}

struct Component {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    /// Blocks per line and column, padded to whole MCUs.
    bw: usize,
    bh: usize,
    coeffs: Vec<i16>,
    dc_table: usize,
    ac_table: usize,
    dc_pred: i32,
}

struct Frame {
    width: usize,
    height: usize,
    progressive: bool,
    hmax: usize,
    vmax: usize,
    mcux: usize,
    mcuy: usize,
    comps: Vec<Component>,
}

pub fn decode(data: &[u8]) -> Result<Image, Error> {
    if !data.starts_with(&[0xff, 0xd8]) {
        return Err(Error::Corrupt("JPEG SOI"));
    }
    let mut qt = [[0u16; 64]; 4];
    let mut dc: [Option<Huffman>; 4] = Default::default();
    let mut ac: [Option<Huffman>; 4] = Default::default();
    let mut frame: Option<Frame> = None;
    let mut restart_interval = 0usize;
    let mut adobe: Option<u8> = None;
    let mut icc = false;
    let mut eobrun = 0u32;
    let mut pos = 2;
    loop {
        // Next marker (skip fill bytes).
        while pos < data.len() && data[pos] != 0xff {
            pos += 1;
        }
        while pos < data.len() && data[pos] == 0xff {
            pos += 1;
        }
        if pos >= data.len() {
            break;
        }
        let marker = data[pos];
        pos += 1;
        if marker == 0xd9 {
            break;
        }
        if (0xd0..=0xd7).contains(&marker) || marker == 0x01 {
            continue;
        }
        if pos + 2 > data.len() {
            return Err(Error::Truncated);
        }
        let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        let seg = data.get(pos + 2..pos + len).ok_or(Error::Truncated)?;
        let next = pos + len;
        match marker {
            0xdb => {
                let mut s = seg;
                while !s.is_empty() {
                    let (precision, id) = ((s[0] >> 4) as usize, (s[0] & 15) as usize);
                    let n = if precision == 0 { 64 } else { 128 };
                    if s.len() < 1 + n || id > 3 {
                        return Err(Error::Corrupt("DQT"));
                    }
                    for i in 0..64 {
                        qt[id][ZIGZAG[i]] = if precision == 0 {
                            s[1 + i] as u16
                        } else {
                            u16::from_be_bytes([s[1 + 2 * i], s[2 + 2 * i]])
                        };
                    }
                    s = &s[1 + n..];
                }
            }
            0xc4 => {
                let mut s = seg;
                while s.len() >= 17 {
                    let (class, id) = (s[0] >> 4, (s[0] & 15) as usize);
                    let counts: [u8; 16] = s[1..17].try_into().unwrap();
                    let total: usize = counts.iter().map(|&c| c as usize).sum();
                    if s.len() < 17 + total || id > 3 {
                        return Err(Error::Corrupt("DHT"));
                    }
                    let table = Huffman::new(&counts, &s[17..17 + total])?;
                    if class == 0 {
                        dc[id] = Some(table);
                    } else {
                        ac[id] = Some(table);
                    }
                    s = &s[17 + total..];
                }
            }
            0xdd => {
                if seg.len() >= 2 {
                    restart_interval = u16::from_be_bytes([seg[0], seg[1]]) as usize;
                }
            }
            0xe2 => icc |= seg.starts_with(b"ICC_PROFILE\0"),
            0xee => {
                if seg.len() >= 12 && &seg[..5] == b"Adobe" {
                    adobe = Some(seg[11]);
                }
            }
            0xc0 | 0xc1 | 0xc2 => frame = Some(start_frame(seg, marker == 0xc2)?),
            0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => {
                return Err(Error::Unsupported("JPEG arithmetic, lossless or hierarchical coding"))
            }
            0xda => {
                let f = frame.as_mut().ok_or(Error::Corrupt("SOS before SOF"))?;
                let end = scan(data, next, seg, f, &dc, &ac, restart_interval, &mut eobrun)?;
                pos = end;
                continue;
            }
            _ => {}
        }
        pos = next;
    }
    let f = frame.ok_or(Error::Corrupt("no JPEG frame"))?;
    let mut image = output(&f, &qt, adobe);
    image.source = match f.comps.len() {
        1 => Source::Gray,
        4 => Source::Cmyk,
        _ => Source::Rgb,
    };
    image.notes.icc_profile = icc;
    Ok(image)
}

fn start_frame(seg: &[u8], progressive: bool) -> Result<Frame, Error> {
    if seg.len() < 6 || seg[0] != 8 {
        return Err(Error::Unsupported("JPEG sample precision other than 8"));
    }
    let height = u16::from_be_bytes([seg[1], seg[2]]) as usize;
    let width = u16::from_be_bytes([seg[3], seg[4]]) as usize;
    let n = seg[5] as usize;
    if width == 0 || height == 0 {
        return Err(Error::Unsupported("JPEG without a height in its frame header"));
    }
    if width as u64 * height as u64 > crate::MAX_PIXELS {
        return Err(Error::Unsupported("image too large"));
    }
    if !matches!(n, 1 | 3 | 4) || seg.len() < 6 + 3 * n {
        return Err(Error::Unsupported("JPEG component count"));
    }
    let mut comps = Vec::new();
    for i in 0..n {
        let c = &seg[6 + 3 * i..];
        let (h, v) = ((c[1] >> 4) as usize, (c[1] & 15) as usize);
        if !(1..=4).contains(&h) || !(1..=4).contains(&v) || c[2] > 3 {
            return Err(Error::Corrupt("SOF component"));
        }
        comps.push(Component {
            id: c[0], h, v, tq: c[2] as usize, bw: 0, bh: 0, coeffs: Vec::new(), dc_table: 0, ac_table: 0, dc_pred: 0,
        });
    }
    let hmax = comps.iter().map(|c| c.h).max().unwrap();
    let vmax = comps.iter().map(|c| c.v).max().unwrap();
    let mcux = (width + 8 * hmax - 1) / (8 * hmax);
    let mcuy = (height + 8 * vmax - 1) / (8 * vmax);
    for c in &mut comps {
        c.bw = mcux * c.h;
        c.bh = mcuy * c.v;
        c.coeffs = vec![0; c.bw * c.bh * 64];
    }
    Ok(Frame { width, height, progressive, hmax, vmax, mcux, mcuy, comps })
}

#[allow(clippy::too_many_arguments)]
fn scan(
    data: &[u8],
    start: usize,
    seg: &[u8],
    f: &mut Frame,
    dc: &[Option<Huffman>; 4],
    ac: &[Option<Huffman>; 4],
    restart_interval: usize,
    eobrun: &mut u32,
) -> Result<usize, Error> {
    let ns = *seg.first().ok_or(Error::Corrupt("SOS"))? as usize;
    if seg.len() < 1 + 2 * ns + 3 {
        return Err(Error::Corrupt("SOS"));
    }
    let mut members = Vec::new();
    for i in 0..ns {
        let id = seg[1 + 2 * i];
        let t = seg[2 + 2 * i];
        let ci = f.comps.iter().position(|c| c.id == id).ok_or(Error::Corrupt("SOS component"))?;
        f.comps[ci].dc_table = (t >> 4) as usize & 3;
        f.comps[ci].ac_table = (t & 15) as usize & 3;
        members.push(ci);
    }
    let ss = seg[1 + 2 * ns] as usize;
    let se = seg[2 + 2 * ns] as usize;
    let ah = (seg[3 + 2 * ns] >> 4) as u32;
    let al = (seg[3 + 2 * ns] & 15) as u32;
    if !f.progressive && (ss != 0 || se != 63 || ah != 0 || al != 0) {
        // Baseline encoders write 0, 63, 0, 0; tolerate others by ignoring them.
    }
    let (ss, se) = if f.progressive { (ss, se.min(63)) } else { (0, 63) };
    if ss > se {
        return Err(Error::Corrupt("spectral selection"));
    }
    let mut r = Reader::new(data, start);
    for &ci in &members {
        f.comps[ci].dc_pred = 0;
    }
    *eobrun = 0;

    // A single-component scan is not interleaved: it walks that component's
    // own blocks (only those inside the image), not MCUs.
    let single = members.len() == 1;
    let units = if single {
        let c = &f.comps[members[0]];
        let w = (f.width * c.h + 8 * f.hmax - 1) / (8 * f.hmax);
        let h = (f.height * c.v + 8 * f.vmax - 1) / (8 * f.vmax);
        (w, h)
    } else {
        (f.mcux, f.mcuy)
    };
    let total = units.0 * units.1;
    for unit in 0..total {
        if restart_interval > 0 && unit > 0 && unit % restart_interval == 0 {
            r.restart();
            for &ci in &members {
                f.comps[ci].dc_pred = 0;
            }
            *eobrun = 0;
        }
        let (ux, uy) = (unit % units.0, unit / units.0);
        for &ci in &members {
            let blocks: Vec<(usize, usize)> = if single {
                vec![(ux, uy)]
            } else {
                let c = &f.comps[ci];
                (0..c.v).flat_map(|by| (0..c.h).map(move |bx| (ux * c.h + bx, uy * c.v + by))).collect()
            };
            for (bx, by) in blocks {
                let c = &mut f.comps[ci];
                let at = (by * c.bw + bx) * 64;
                let (dct, act) = (c.dc_table, c.ac_table);
                let block = &mut c.coeffs[at..at + 64];
                let pred = &mut c.dc_pred;
                if !f.progressive {
                    decode_baseline(&mut r, block, pred, table(dc, dct)?, table(ac, act)?)?;
                } else if ss == 0 {
                    if ah == 0 {
                        let t = table(dc, dct)?;
                        let s = r.decode(t)? as u32;
                        *pred += r.receive_extend(s);
                        block[0] = (*pred << al) as i16;
                    } else if r.bit() == 1 {
                        block[0] |= (1 << al) as i16;
                    }
                } else if ah == 0 {
                    ac_first(&mut r, block, table(ac, act)?, ss, se, al, eobrun)?;
                } else {
                    ac_refine(&mut r, block, table(ac, act)?, ss, se, al, eobrun)?;
                }
            }
        }
    }
    Ok(r.end())
}

fn table(tables: &[Option<Huffman>; 4], i: usize) -> Result<&Huffman, Error> {
    tables[i].as_ref().ok_or(Error::Corrupt("missing Huffman table"))
}

fn decode_baseline(r: &mut Reader, block: &mut [i16], pred: &mut i32, dc: &Huffman, ac: &Huffman) -> Result<(), Error> {
    let s = r.decode(dc)? as u32;
    *pred += r.receive_extend(s);
    block[0] = *pred as i16;
    let mut k = 1;
    while k < 64 {
        let rs = r.decode(ac)?;
        let (run, size) = ((rs >> 4) as usize, (rs & 15) as u32);
        if size == 0 {
            if run == 15 {
                k += 16;
                continue;
            }
            break;
        }
        k += run;
        if k > 63 {
            break;
        }
        block[ZIGZAG[k]] = r.receive_extend(size) as i16;
        k += 1;
    }
    Ok(())
}

fn ac_first(r: &mut Reader, block: &mut [i16], ac: &Huffman, ss: usize, se: usize, al: u32, eobrun: &mut u32) -> Result<(), Error> {
    if *eobrun > 0 {
        *eobrun -= 1;
        return Ok(());
    }
    let mut k = ss;
    while k <= se {
        let rs = r.decode(ac)?;
        let (run, size) = ((rs >> 4) as u32, (rs & 15) as u32);
        if size == 0 {
            if run < 15 {
                *eobrun = (1 << run) - 1 + r.bits(run);
                break;
            }
            k += 16;
            continue;
        }
        k += run as usize;
        if k > 63 {
            break;
        }
        block[ZIGZAG[k]] = (r.receive_extend(size) * (1 << al)) as i16;
        k += 1;
    }
    Ok(())
}

fn ac_refine(r: &mut Reader, block: &mut [i16], ac: &Huffman, ss: usize, se: usize, al: u32, eobrun: &mut u32) -> Result<(), Error> {
    let p1 = 1i16 << al;
    let m1 = -1i16 << al;
    let mut k = ss;
    if *eobrun == 0 {
        while k <= se {
            let rs = r.decode(ac)?;
            let (mut run, size) = ((rs >> 4) as i32, (rs & 15) as u32);
            let mut value = 0i16;
            if size == 0 {
                if run < 15 {
                    *eobrun = (1 << run) + r.bits(run as u32);
                    break;
                }
            } else {
                value = if r.bit() == 1 { p1 } else { m1 };
            }
            while k <= se {
                let z = ZIGZAG[k];
                if block[z] != 0 {
                    if r.bit() == 1 && (block[z] & p1) == 0 {
                        block[z] += if block[z] >= 0 { p1 } else { m1 };
                    }
                } else {
                    if run == 0 {
                        if value != 0 {
                            block[z] = value;
                        }
                        k += 1;
                        break;
                    }
                    run -= 1;
                }
                k += 1;
            }
        }
    }
    if *eobrun > 0 {
        while k <= se {
            let z = ZIGZAG[k];
            if block[z] != 0 && r.bit() == 1 && (block[z] & p1) == 0 {
                block[z] += if block[z] >= 0 { p1 } else { m1 };
            }
            k += 1;
        }
        *eobrun -= 1;
    }
    Ok(())
}

/// Separable float IDCT of one dequantized block into 8×8 samples.
fn idct(block: &[i16], q: &[u16; 64], out: &mut [u8], stride: usize) {
    use std::sync::OnceLock;
    static COS: OnceLock<[[f32; 8]; 8]> = OnceLock::new();
    let cos = COS.get_or_init(|| {
        let mut t = [[0f32; 8]; 8];
        for (x, row) in t.iter_mut().enumerate() {
            for (u, v) in row.iter_mut().enumerate() {
                let cu = if u == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
                *v = cu * (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 16.0).cos() / 2.0;
            }
        }
        t
    });
    // A block with only a DC coefficient is flat.
    if block[1..].iter().all(|&c| c == 0) {
        let s = block[0] as f32 * q[0] as f32 / 8.0;
        let value = (s + 128.0).round().clamp(0.0, 255.0) as u8;
        for y in 0..8 {
            out[y * stride..y * stride + 8].fill(value);
        }
        return;
    }
    let mut tmp = [0f32; 64];
    for v in 0..8 {
        // Rows of zero coefficients (most high-frequency rows) stay zero.
        if block[v * 8..v * 8 + 8].iter().all(|&c| c == 0) {
            continue;
        }
        for x in 0..8 {
            let mut s = 0f32;
            for u in 0..8 {
                let c = block[v * 8 + u];
                if c != 0 {
                    s += cos[x][u] * (c as f32 * q[v * 8 + u] as f32);
                }
            }
            tmp[v * 8 + x] = s;
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            let mut s = 0f32;
            for v in 0..8 {
                s += cos[y][v] * tmp[v * 8 + x];
            }
            out[y * stride + x] = (s + 128.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn output(f: &Frame, qt: &[[u16; 64]; 4], adobe: Option<u8>) -> Image {
    // Each component's samples, upsampled to the image's resolution.
    let planes: Vec<Vec<u8>> = f
        .comps
        .iter()
        .map(|c| {
            let stride = c.bw * 8;
            let mut plane = vec![0u8; stride * c.bh * 8];
            for by in 0..c.bh {
                for bx in 0..c.bw {
                    let at = (by * c.bw + bx) * 64;
                    idct(&c.coeffs[at..at + 64], &qt[c.tq], &mut plane[by * 8 * stride + bx * 8..], stride);
                }
            }
            upsample(f, c, &plane, stride)
        })
        .collect();
    let mut image = Image::new(f.width as u32, f.height as u32);
    let n = f.comps.len();
    let transform = match (n, adobe) {
        (3, Some(0)) => 0, // RGB
        (3, _) => 1,       // YCbCr
        (4, Some(2)) => 2, // YCCK
        (4, _) => 3,       // CMYK
        _ => 0,
    };
    for i in 0..f.width * f.height {
        let px = &mut image.pixels[i * 4..i * 4 + 4];
        if n == 1 {
            let g = planes[0][i];
            px.copy_from_slice(&[g, g, g, 255]);
            continue;
        }
        let (a, b, c) = (planes[0][i], planes[1][i], planes[2][i]);
        let (r, g, bl) = if transform == 0 || transform == 3 { (a, b, c) } else { ycc(a, b, c) };
        if n == 4 {
            // Adobe stores CMYK inverted; RGB = (inverted C/M/Y) × (inverted K) / 255.
            let k = planes[3][i] as u32;
            let mul = |v: u8| ((v as u32 * k + 127) / 255) as u8;
            let (r, g, bl) = if transform == 2 { (255 - r, 255 - g, 255 - bl) } else { (r, g, bl) };
            px.copy_from_slice(&[mul(r), mul(g), mul(bl), 255]);
        } else {
            px.copy_from_slice(&[r, g, bl, 255]);
        }
    }
    image
}

/// A component's plane (`stride` wide, its own resolution) at the image's
/// `width × height`. Half-width and half-size chroma use libjpeg's "fancy"
/// triangle filter (what ImageIO and most decoders do); other factors
/// replicate samples.
fn upsample(f: &Frame, c: &Component, plane: &[u8], stride: usize) -> Vec<u8> {
    let (w, h) = (f.width, f.height);
    let (fx, fy) = (f.hmax / c.h, f.vmax / c.v);
    // The component's own size inside the image (edges are clamped here).
    let cw = (w * c.h + f.hmax - 1) / f.hmax;
    let ch = (h * c.v + f.vmax - 1) / f.vmax;
    let at = |x: usize, y: usize| plane[y.min(ch - 1) * stride + x.min(cw - 1)] as i32;
    let mut out = vec![0u8; w * h];
    if f.hmax % c.h != 0 || f.vmax % c.v != 0 || !((fx, fy) == (2, 1) || (fx, fy) == (2, 2)) || cw < 2 {
        for y in 0..h {
            for x in 0..w {
                out[y * w + x] = plane[(y * c.v / f.vmax) * stride + x * c.h / f.hmax];
            }
        }
        return out;
    }
    let mut sums = vec![0i32; cw];
    for y in 0..h {
        // Vertical: the row itself ×3 plus the nearer neighbour row (h2v2),
        // or the row ×4 (h2v1), in units of 1/4.
        let (row, scale) = if fy == 2 {
            let r = y / 2;
            let near = if y % 2 == 0 { r.saturating_sub(1) } else { (r + 1).min(ch - 1) };
            for (x, s) in sums.iter_mut().enumerate() {
                *s = 3 * at(x, r) + at(x, near);
            }
            (r, 4)
        } else {
            for (x, s) in sums.iter_mut().enumerate() {
                *s = at(x, y);
            }
            (y, 1)
        };
        let _ = row;
        let o = &mut out[y * w..y * w + w];
        for x in 0..w {
            let i = x / 2;
            let v = if scale == 4 {
                // h2v2: (3·this + neighbour column) / 16, biased 8 then 7.
                if x == 0 {
                    (sums[0] * 4 + 8) >> 4
                } else if i == cw - 1 && x % 2 == 1 {
                    (sums[i] * 4 + 7) >> 4
                } else if x % 2 == 0 {
                    (3 * sums[i] + sums[i - 1] + 8) >> 4
                } else {
                    (3 * sums[i] + sums[(i + 1).min(cw - 1)] + 7) >> 4
                }
            } else if x == 0 {
                sums[0]
            } else if i == cw - 1 && x % 2 == 1 {
                sums[i]
            } else if x % 2 == 0 {
                (3 * sums[i] + sums[i - 1] + 1) >> 2
            } else {
                (3 * sums[i] + sums[(i + 1).min(cw - 1)] + 2) >> 2
            };
            o[x] = v as u8;
        }
    }
    out
}

/// JFIF YCbCr to RGB.
fn ycc(y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
    let (y, cb, cr) = (y as f32, cb as f32 - 128.0, cr as f32 - 128.0);
    let r = y + 1.402 * cr;
    let g = y - 0.344136 * cb - 0.714136 * cr;
    let b = y + 1.772 * cb;
    let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    (c(r), c(g), c(b))
}
