//! Clean-room parser for Apple `MTLB` precompiled Metal shader libraries
//! and LLVM bitcode / AIR payload extraction.

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex = 0,
    Fragment = 1,
    Kernel = 2,
    Unknown(u8),
}

impl ShaderStage {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => ShaderStage::Vertex,
            1 => ShaderStage::Fragment,
            2 => ShaderStage::Kernel,
            other => ShaderStage::Unknown(other),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MtlbFunction {
    pub name: String,
    pub stage: ShaderStage,
    pub hash: [u8; 32],
    pub bitcode_offset: usize,
    pub bitcode_size: usize,
    pub bitcode: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct MtlbLibrary {
    pub version_major: u16,
    pub version_minor: u16,
    pub is_macos: bool,
    pub os_id: u8,
    pub functions: Vec<MtlbFunction>,
}

pub fn is_llvm_bitcode(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && (bytes[..4] == [0x42, 0x43, 0xC0, 0xDE] || bytes[..4] == [0xDE, 0xC0, 0x17, 0x0B])
}

fn u16_le(d: &[u8], off: usize) -> Result<u16, String> {
    let b = d.get(off..off + 2).ok_or_else(|| "truncated u16".to_string())?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn u32_le(d: &[u8], off: usize) -> Result<u32, String> {
    let b = d.get(off..off + 4).ok_or_else(|| "truncated u32".to_string())?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u64_le(d: &[u8], off: usize) -> Result<u64, String> {
    let b = d.get(off..off + 8).ok_or_else(|| "truncated u64".to_string())?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

fn looks_like_tag(bytes: &[u8]) -> bool {
    bytes.len() == 4
        && bytes
            .iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Parses an Apple `MTLB` container buffer into its function table and AIR bitcode slices.
pub fn parse_mtlb(data: &[u8]) -> Result<MtlbLibrary, String> {
    if data.len() < 88 {
        return Err("MTLB buffer shorter than 88-byte header".to_string());
    }
    if &data[0..4] != b"MTLB" {
        return Err("invalid MTLB magic".to_string());
    }

    let version_major = u16_le(data, 4)? & 0x7FFF;
    let is_macos = (data[5] & 0x80) != 0;
    let version_minor = u16_le(data, 6)?;
    let os_id = data[11];

    let func_list_off = u64_le(data, 24)? as usize;
    let func_list_size = u64_le(data, 32)? as usize;
    let bitcode_base_off = u64_le(data, 72)? as usize;

    let fl_end = func_list_off.saturating_add(func_list_size).min(data.len());
    if func_list_off > fl_end {
        return Err("MTLB function list out of bounds".to_string());
    }

    let fl = &data[func_list_off..fl_end];
    let mut functions = Vec::new();

    if !fl.is_empty() {
        let mut cursor = 0usize;
        // Newer/standard MTLB function lists start with a 4-byte function count
        // when the first 4 bytes are not already a 4-char ASCII tag.
        if fl.len() >= 4 && !looks_like_tag(&fl[0..4]) {
            let _count = u32_le(fl, 0)?;
            cursor = 4;
        }

        while cursor + 4 <= fl.len() {
            // Each function record may start with a 4-byte byte-length prefix.
            if !looks_like_tag(&fl[cursor..cursor + 4]) {
                cursor += 4;
                if cursor + 4 > fl.len() {
                    break;
                }
            }

            let mut name = String::new();
            let mut stage = ShaderStage::Unknown(0xFF);
            let mut hash = [0u8; 32];
            let mut bitcode_size = 0usize;
            let mut bitcode_rel = 0usize;
            let mut saw_end = false;

            while cursor + 4 <= fl.len() {
                let tag = &fl[cursor..cursor + 4];
                cursor += 4;
                if tag == b"ENDT" {
                    saw_end = true;
                    break;
                }
                if cursor + 2 > fl.len() {
                    break;
                }
                let payload_len = u16_le(fl, cursor)? as usize;
                cursor += 2;
                let payload_end = cursor.saturating_add(payload_len);
                if payload_end > fl.len() {
                    break;
                }
                let payload = &fl[cursor..payload_end];
                cursor = payload_end;

                match tag {
                    b"NAME" => {
                        let nul = payload.iter().position(|&b| b == 0).unwrap_or(payload.len());
                        name = String::from_utf8_lossy(&payload[..nul]).into_owned();
                    }
                    b"TYPE" if !payload.is_empty() => {
                        stage = ShaderStage::from_byte(payload[0]);
                    }
                    b"HASH" if payload.len() >= 32 => {
                        hash.copy_from_slice(&payload[..32]);
                    }
                    b"MDSZ" if payload.len() >= 8 => {
                        bitcode_size = u64_le(payload, 0)? as usize;
                    }
                    b"OFFT" if payload.len() >= 24 => {
                        bitcode_rel = u64_le(payload, 16)? as usize;
                    }
                    _ => {}
                }
            }

            if !name.is_empty() || saw_end {
                let abs_off = bitcode_base_off.saturating_add(bitcode_rel);
                let bitcode = if bitcode_size > 0 && abs_off.saturating_add(bitcode_size) <= data.len() {
                    data[abs_off..abs_off + bitcode_size].to_vec()
                } else {
                    Vec::new()
                };
                functions.push(MtlbFunction {
                    name,
                    stage,
                    hash,
                    bitcode_offset: abs_off,
                    bitcode_size,
                    bitcode,
                });
            }
            if !saw_end {
                break;
            }
        }
    }

    Ok(MtlbLibrary {
        version_major,
        version_minor,
        is_macos,
        os_id,
        functions,
    })
}
