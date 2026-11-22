//! DEFLATE (RFC 1951) decompression, raw or in a zlib wrapper (RFC 1950).

use crate::Error;

/// Reads bits least-significant first, as DEFLATE packs them.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u64,
    count: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0, buf: 0, count: 0 }
    }

    #[inline]
    fn refill(&mut self) {
        if self.pos + 8 <= self.data.len() {
            // Fast path: one unaligned 8-byte load fills the buffer to 56+ bits.
            let word = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
            self.buf |= word << self.count;
            let taken = (63 - self.count) / 8;
            self.pos += taken as usize;
            self.count += taken * 8;
            return;
        }
        while self.count <= 56 {
            let byte = match self.data.get(self.pos) {
                Some(&b) => b,
                None => {
                    // Past the end: feed zeros so peeks work near the end; a
                    // real read beyond it is caught by `need`.
                    if self.pos >= self.data.len() + 8 {
                        return;
                    }
                    0
                }
            };
            self.pos += 1;
            self.buf |= (byte as u64) << self.count;
            self.count += 8;
        }
    }

    /// Bits consumed beyond the input (zero padding) mean truncated data.
    fn overrun(&self) -> bool {
        let consumed_bits = self.pos as u64 * 8 - self.count as u64;
        consumed_bits > self.data.len() as u64 * 8
    }

    #[inline]
    fn get(&mut self, n: u32) -> Result<u32, Error> {
        if n == 0 {
            return Ok(0);
        }
        if self.count < n {
            self.refill();
        }
        let v = (self.buf & ((1u64 << n) - 1)) as u32;
        self.buf >>= n;
        self.count -= n;
        // Reading into the zero padding means the input ended early. Cheap
        // test first: padding exists only once `pos` is past the end.
        if self.pos > self.data.len() && self.overrun() {
            return Err(Error::Truncated);
        }
        Ok(v)
    }

    fn align_to_byte(&mut self) {
        let drop = self.count % 8;
        self.buf >>= drop;
        self.count -= drop;
    }

    /// Whole bytes after `align_to_byte`.
    fn byte(&mut self) -> Result<u8, Error> {
        self.get(8).map(|v| v as u8)
    }
}

const FAST_BITS: u32 = 10;

/// A canonical Huffman code: a direct table for codes of up to FAST_BITS
/// bits, and per-length counts/symbols for the rest.
struct Huffman {
    fast: Vec<u16>, // (length << 12) | symbol, 0 = not in table
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Self, Error> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        // Over-subscribed codes are invalid; incomplete ones are allowed
        // (a single distance code, for instance).
        let mut left: i32 = 1;
        for len in 1..16 {
            left <<= 1;
            left -= counts[len] as i32;
            if left < 0 {
                return Err(Error::Corrupt("over-subscribed Huffman code"));
            }
        }
        let mut offsets = [0u16; 16];
        for len in 1..15 {
            offsets[len + 1] = offsets[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offsets[l as usize] as usize] = sym as u16;
                offsets[l as usize] += 1;
            }
        }
        // Fast table: walk codes in canonical order.
        let mut fast = vec![0u16; 1 << FAST_BITS];
        let mut code: u32 = 0;
        let mut index = 0usize;
        for len in 1..=FAST_BITS as usize {
            for _ in 0..counts[len] {
                let sym = symbols[index];
                index += 1;
                // Codes are stored bit-reversed (read LSB first).
                let rev = reverse(code, len as u32);
                let step = 1usize << len;
                let mut i = rev as usize;
                while i < fast.len() {
                    fast[i] = ((len as u16) << 12) | sym;
                    i += step;
                }
                code += 1;
            }
            code <<= 1;
        }
        Ok(Huffman { fast, counts, symbols })
    }

    #[inline]
    fn decode(&self, bits: &mut Bits) -> Result<u16, Error> {
        if bits.count < 16 {
            bits.refill();
        }
        let entry = self.fast[(bits.buf & ((1 << FAST_BITS) - 1)) as usize];
        if entry != 0 {
            let len = (entry >> 12) as u32;
            bits.buf >>= len;
            bits.count -= len;
            if bits.pos > bits.data.len() && bits.overrun() {
                return Err(Error::Truncated);
            }
            return Ok(entry & 0x0fff);
        }
        // Slow path: canonical decode bit by bit.
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= bits.get(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(Error::Corrupt("invalid Huffman code"))
    }
}

fn reverse(mut code: u32, len: u32) -> u32 {
    let mut out = 0;
    for _ in 0..len {
        out = (out << 1) | (code & 1);
        code >>= 1;
    }
    out
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// Decompresses raw DEFLATE data. `size_hint` pre-sizes the output.
pub fn inflate(data: &[u8], size_hint: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(size_hint);
    let mut bits = Bits::new(data);
    loop {
        let last = bits.get(1)? == 1;
        match bits.get(2)? {
            0 => {
                bits.align_to_byte();
                let len = bits.byte()? as u16 | (bits.byte()? as u16) << 8;
                let nlen = bits.byte()? as u16 | (bits.byte()? as u16) << 8;
                if len != !nlen {
                    return Err(Error::Corrupt("stored block length"));
                }
                for _ in 0..len {
                    out.push(bits.byte()?);
                }
            }
            1 => {
                let mut lengths = [0u8; 288];
                lengths[..144].fill(8);
                lengths[144..256].fill(9);
                lengths[256..280].fill(7);
                lengths[280..].fill(8);
                let lit = Huffman::new(&lengths)?;
                let dist = Huffman::new(&[5u8; 30])?;
                block(&mut bits, &mut out, &lit, &dist)?;
            }
            2 => {
                let (lit, dist) = dynamic_tables(&mut bits)?;
                block(&mut bits, &mut out, &lit, &dist)?;
            }
            _ => return Err(Error::Corrupt("reserved block type")),
        }
        if last {
            return Ok(out);
        }
    }
}

fn dynamic_tables(bits: &mut Bits) -> Result<(Huffman, Huffman), Error> {
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let hlit = bits.get(5)? as usize + 257;
    let hdist = bits.get(5)? as usize + 1;
    let hclen = bits.get(4)? as usize + 4;
    let mut code_lengths = [0u8; 19];
    for &i in ORDER.iter().take(hclen) {
        code_lengths[i] = bits.get(3)? as u8;
    }
    let code = Huffman::new(&code_lengths)?;
    let mut lengths = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < lengths.len() {
        let sym = code.decode(bits)?;
        let (value, repeat) = match sym {
            0..=15 => (sym as u8, 1),
            16 => {
                if i == 0 {
                    return Err(Error::Corrupt("repeat with no previous length"));
                }
                (lengths[i - 1], 3 + bits.get(2)? as usize)
            }
            17 => (0, 3 + bits.get(3)? as usize),
            18 => (0, 11 + bits.get(7)? as usize),
            _ => return Err(Error::Corrupt("code length symbol")),
        };
        if i + repeat > lengths.len() {
            return Err(Error::Corrupt("code lengths overflow"));
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    Ok((Huffman::new(&lengths[..hlit])?, Huffman::new(&lengths[hlit..])?))
}

fn block(bits: &mut Bits, out: &mut Vec<u8>, lit: &Huffman, dist: &Huffman) -> Result<(), Error> {
    loop {
        let sym = lit.decode(bits)? as usize;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let li = sym - 257;
            if li >= 29 {
                return Err(Error::Corrupt("length symbol"));
            }
            let len = LENGTH_BASE[li] as usize + bits.get(LENGTH_EXTRA[li] as u32)? as usize;
            let di = dist.decode(bits)? as usize;
            if di >= 30 {
                return Err(Error::Corrupt("distance symbol"));
            }
            let distance = DIST_BASE[di] as usize + bits.get(DIST_EXTRA[di] as u32)? as usize;
            if distance > out.len() {
                return Err(Error::Corrupt("distance before start"));
            }
            let start = out.len() - distance;
            if distance >= len {
                out.extend_from_within(start..start + len);
            } else {
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
        }
    }
}

/// Decompresses a zlib stream (2-byte header, DEFLATE data, Adler-32).
pub fn zlib_decompress(data: &[u8], size_hint: usize) -> Result<Vec<u8>, Error> {
    if data.len() < 2 {
        return Err(Error::Truncated);
    }
    let (cmf, flg) = (data[0], data[1]);
    if cmf & 0x0f != 8 || ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return Err(Error::Corrupt("zlib header"));
    }
    if flg & 0x20 != 0 {
        return Err(Error::Unsupported("zlib preset dictionary"));
    }
    // The Adler-32 trailer is not checked: like most decoders (and ImageIO),
    // a PNG whose image data decodes is shown even if the checksum is off.
    inflate(&data[2..], size_hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_fixed_and_dynamic_blocks() {
        // Stored block: "hello".
        let stored = [0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(inflate(&stored, 0).unwrap(), b"hello");
        // zlib.compress(b"hello hello hello hello", 9): fixed Huffman, a back-reference.
        let fixed = hex("78dacb48cdc9c957c8402701680308b1");
        assert_eq!(zlib_decompress(&fixed, 0).unwrap(), b"hello hello hello hello");
        // 40 numbered lines, level 9: a dynamic-Huffman block.
        let dynamic = hex(concat!(
            "78da95955b56c3300c44ff5985966049b663b31b1e010aa18196d2c2ea795893fff9eeb947d1e87abaecf6b3a46bf9789ae5fdb4bb7b",
            "91dbc37adecbc37a91e7d3ebdb51d6cff9f0fff372f3fd25f7eba3a4abe58f528ed2411947e5413947f54165f20bebc00a87591958e5",
            "308f69131948ecd638ac46909dc35a5c4d4945344112d612c3445214cdb1a13a7bf54854595d7a5c505961608c56d6344c24a5b1861d",
            "496ddc906a67edc67b27cdc930c7487372c644b6625aec68a439c5225523cd2953dcd148732acc31d29cba4d24cd99b61d4973a62d55",
            "b672704727cde930c749733a5c75b673129e87b3a593f0229d6d1d45097861ab15f6786549549d93faa8a35dbdd17d8e6c4981b4fcfe",
            "87fc002297002a"
        ));
        let expected: Vec<u8> = (0..40)
            .flat_map(|i| format!("line {i}: the quick brown fox jumps over the lazy dog {}\n", i * i).into_bytes())
            .collect();
        assert_eq!(zlib_decompress(&dynamic, 0).unwrap(), expected);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn truncated_input_is_an_error() {
        let fixed = hex("78dacb48cdc9c957");
        assert!(zlib_decompress(&fixed, 0).is_err());
    }
}
