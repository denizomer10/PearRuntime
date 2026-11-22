//! Fixups in a loader-friendly form, from dyld-info opcodes (rebase, bind,
//! lazy-bind and weak-bind streams) or from chained fixups.

use crate::exports::{sleb, uleb};
use crate::image::{c_str, u16_at, u32_at, u64_at, Error, Fixup, FixupKind, Fixups, Import, MachO, Ordinal};
use std::collections::HashMap;

/// Collects imports, sharing one entry per (name, ordinal, weak).
#[derive(Default)]
struct Imports {
    list: Vec<Import>,
    index: HashMap<(String, Ordinal, bool), usize>,
}

impl Imports {
    fn add(&mut self, name: &str, ordinal: Ordinal, weak: bool) -> usize {
        let key = (name.to_string(), ordinal, weak);
        if let Some(&i) = self.index.get(&key) {
            return i;
        }
        self.list.push(Import { name: name.to_string(), ordinal, weak });
        self.index.insert(key, self.list.len() - 1);
        self.list.len() - 1
    }
}

const REBASE_TYPE_POINTER: u8 = 1;
const BIND_TYPE_POINTER: u8 = 1;
const BIND_SYMBOL_FLAGS_WEAK_IMPORT: u8 = 0x1;
const BIND_SYMBOL_FLAGS_NON_WEAK_DEFINITION: u8 = 0x8;

/// dyld-info: `info` is the LC_DYLD_INFO payload (rebase, bind, weak bind,
/// lazy bind, export: offset/size pairs).
pub(crate) fn opcodes(m: &MachO, info: [u32; 10]) -> Result<Fixups, Error> {
    let mut fixups = Vec::new();
    let mut imports = Imports::default();
    rebases(m, m.file_bytes(info[0], info[1])?, &mut fixups)?;
    binds(m, m.file_bytes(info[2], info[3])?, Stream::Regular, &mut imports, &mut fixups)?;
    binds(m, m.file_bytes(info[6], info[7])?, Stream::Lazy, &mut imports, &mut fixups)?;
    binds(m, m.file_bytes(info[4], info[5])?, Stream::Weak, &mut imports, &mut fixups)?;
    Ok(Fixups { imports: imports.list, fixups })
}

fn segment_address(m: &MachO, index: usize, offset: u64) -> Result<u64, Error> {
    let seg = m.segments.get(index).ok_or(Error::Malformed("segment index"))?;
    Ok(seg.vmaddr.wrapping_add(offset))
}

fn rebases(m: &MachO, ops: &[u8], out: &mut Vec<Fixup>) -> Result<(), Error> {
    let mut pos = 0;
    let mut address = 0u64;
    let mut kind = REBASE_TYPE_POINTER;
    let mut push = |address: u64, kind: u8| -> Result<(), Error> {
        if kind != REBASE_TYPE_POINTER {
            return Err(Error::Unsupported("rebase type other than pointer"));
        }
        let target = m.read_u64_at_address(address)?;
        out.push(Fixup { address, kind: FixupKind::Rebase { target, high8: 0 }, lazy: false });
        Ok(())
    };
    while pos < ops.len() {
        let byte = ops[pos];
        pos += 1;
        let (op, imm) = (byte & 0xf0, byte & 0x0f);
        match op {
            0x00 => break,
            0x10 => kind = imm,
            0x20 => address = segment_address(m, imm as usize, uleb(ops, &mut pos)?)?,
            0x30 => address = address.wrapping_add(uleb(ops, &mut pos)?),
            0x40 => address = address.wrapping_add(imm as u64 * 8),
            0x50 => {
                for _ in 0..imm {
                    push(address, kind)?;
                    address += 8;
                }
            }
            0x60 => {
                for _ in 0..uleb(ops, &mut pos)? {
                    push(address, kind)?;
                    address += 8;
                }
            }
            0x70 => {
                push(address, kind)?;
                address = address.wrapping_add(8 + uleb(ops, &mut pos)?);
            }
            0x80 => {
                let count = uleb(ops, &mut pos)?;
                let skip = uleb(ops, &mut pos)?;
                for _ in 0..count {
                    push(address, kind)?;
                    address = address.wrapping_add(8 + skip);
                }
            }
            _ => return Err(Error::Malformed("rebase opcode")),
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stream {
    Regular,
    Lazy,
    Weak,
}

fn binds(m: &MachO, ops: &[u8], stream: Stream, imports: &mut Imports, out: &mut Vec<Fixup>) -> Result<(), Error> {
    let mut pos = 0;
    let mut address = 0u64;
    let mut ordinal = if stream == Stream::Weak { Ordinal::WeakLookup } else { Ordinal::SelfImage };
    let mut name = String::new();
    let mut flags = 0u8;
    let mut kind = BIND_TYPE_POINTER;
    let mut addend = 0i64;
    let mut bind = |address: u64, name: &str, ordinal: Ordinal, flags: u8, kind: u8, addend: i64| -> Result<(), Error> {
        if kind != BIND_TYPE_POINTER {
            return Err(Error::Unsupported("bind type other than pointer"));
        }
        let import = imports.add(name, ordinal, flags & BIND_SYMBOL_FLAGS_WEAK_IMPORT != 0);
        out.push(Fixup { address, kind: FixupKind::Bind { import, addend }, lazy: stream == Stream::Lazy });
        Ok(())
    };
    while pos < ops.len() {
        let byte = ops[pos];
        pos += 1;
        let (op, imm) = (byte & 0xf0, byte & 0x0f);
        match op {
            // DONE ends the regular and weak streams; the lazy stream is a
            // sequence of DONE-terminated entries.
            0x00 => {
                if stream != Stream::Lazy {
                    break;
                }
            }
            0x10 => ordinal = m.ordinal(imm as i64)?,
            0x20 => ordinal = m.ordinal(uleb(ops, &mut pos)? as i64)?,
            0x30 => ordinal = m.ordinal(if imm == 0 { 0 } else { (imm | 0xf0) as i8 as i64 })?,
            0x40 => {
                flags = imm;
                name = c_str(ops, pos)?;
                pos += name.len() + 1;
            }
            0x50 => kind = imm,
            0x60 => addend = sleb(ops, &mut pos)?,
            0x70 => address = segment_address(m, imm as usize, uleb(ops, &mut pos)?)?,
            0x80 => address = address.wrapping_add(uleb(ops, &mut pos)?),
            0x90 | 0xa0 | 0xb0 => {
                // In the weak stream, a strong definition is only declared.
                if !(stream == Stream::Weak && flags & BIND_SYMBOL_FLAGS_NON_WEAK_DEFINITION != 0) {
                    bind(address, &name, ordinal, flags, kind, addend)?;
                }
                address = address.wrapping_add(8);
                match op {
                    0xa0 => address = address.wrapping_add(uleb(ops, &mut pos)?),
                    0xb0 => address = address.wrapping_add(imm as u64 * 8),
                    _ => {}
                }
            }
            0xc0 => {
                let count = uleb(ops, &mut pos)?;
                let skip = uleb(ops, &mut pos)?;
                for _ in 0..count {
                    bind(address, &name, ordinal, flags, kind, addend)?;
                    address = address.wrapping_add(8 + skip);
                }
            }
            0xd0 => return Err(Error::Unsupported("threaded binds (arm64e)")),
            _ => return Err(Error::Malformed("bind opcode")),
        }
    }
    Ok(())
}

const DYLD_CHAINED_PTR_ARM64E: u16 = 1;
const DYLD_CHAINED_PTR_64: u16 = 2;
const DYLD_CHAINED_PTR_64_OFFSET: u16 = 6;
const DYLD_CHAINED_PTR_ARM64E_USERLAND: u16 = 7;
const DYLD_CHAINED_PTR_ARM64E_USERLAND24: u16 = 9;
const DYLD_CHAINED_PTR_START_NONE: u16 = 0xffff;

/// Chained fixups: `blob` is the LC_DYLD_CHAINED_FIXUPS payload.
pub(crate) fn chained(m: &MachO, blob: &[u8]) -> Result<Fixups, Error> {
    let version = u32_at(blob, 0)?;
    if version != 0 {
        return Err(Error::Unsupported("chained fixups version"));
    }
    let starts = u32_at(blob, 4)? as usize;
    let imports_at = u32_at(blob, 8)? as usize;
    let symbols_at = u32_at(blob, 12)? as usize;
    let count = u32_at(blob, 16)? as usize;
    let format = u32_at(blob, 20)?;
    if u32_at(blob, 24)? != 0 {
        return Err(Error::Unsupported("compressed chained-fixup symbol names"));
    }
    // Imports: (import index, addend) in file order.
    let mut imports = Vec::with_capacity(count);
    let mut addends = Vec::with_capacity(count);
    for i in 0..count {
        let (ordinal, weak, name_off, addend) = match format {
            1 => {
                let v = u32_at(blob, imports_at + i * 4)?;
                ((v & 0xff) as u8 as i8 as i64, v >> 8 & 1 == 1, (v >> 9) as usize, 0i64)
            }
            2 => {
                let v = u32_at(blob, imports_at + i * 8)?;
                let a = u32_at(blob, imports_at + i * 8 + 4)? as i32 as i64;
                ((v & 0xff) as u8 as i8 as i64, v >> 8 & 1 == 1, (v >> 9) as usize, a)
            }
            3 => {
                let v = u64_at(blob, imports_at + i * 16)?;
                let a = u64_at(blob, imports_at + i * 16 + 8)? as i64;
                ((v & 0xffff) as u16 as i16 as i64, v >> 16 & 1 == 1, (v >> 32) as usize, a)
            }
            _ => return Err(Error::Unsupported("chained import format")),
        };
        // Positive ordinals are unsigned (up to 0xEF / 0xFFEF); special
        // ones are the small negative values.
        let ordinal = if ordinal < -3 {
            match format {
                3 => (ordinal as i16 as u16) as i64,
                _ => (ordinal as i8 as u8) as i64,
            }
        } else {
            ordinal
        };
        imports.push(Import { name: c_str(blob, symbols_at + name_off)?, ordinal: m.ordinal(ordinal)?, weak });
        addends.push(addend);
    }

    let base = m.base_address();
    let mut fixups = Vec::new();
    let seg_count = u32_at(blob, starts)? as usize;
    for seg in 0..seg_count {
        let info_off = u32_at(blob, starts + 4 + seg * 4)? as usize;
        if info_off == 0 {
            continue;
        }
        let at = starts + info_off;
        let page_size = u16_at(blob, at + 4)? as u64;
        let pointer_format = u16_at(blob, at + 6)?;
        let segment_offset = u64_at(blob, at + 8)?;
        let page_count = u16_at(blob, at + 20)? as u64;
        let is_arm64e = matches!(
            pointer_format,
            DYLD_CHAINED_PTR_ARM64E | DYLD_CHAINED_PTR_ARM64E_USERLAND | DYLD_CHAINED_PTR_ARM64E_USERLAND24
        );
        if !is_arm64e && pointer_format != DYLD_CHAINED_PTR_64 && pointer_format != DYLD_CHAINED_PTR_64_OFFSET {
            return Err(Error::Unsupported("chained pointer format (32-bit or unknown)"));
        }
        let stride = if is_arm64e { 8 } else { 4 };
        for page in 0..page_count {
            let start = u16_at(blob, at + 22 + page as usize * 2)?;
            if start == DYLD_CHAINED_PTR_START_NONE {
                continue;
            }
            if start & 0x8000 != 0 {
                return Err(Error::Unsupported("multiple chain starts per page"));
            }
            let mut address = base + segment_offset + page * page_size + start as u64;
            let mut steps = 0;
            loop {
                let raw = m.read_u64_at_address(address)?;
                let (next, kind) = if is_arm64e {
                    let auth = (raw >> 63) & 1 == 1;
                    let bind = (raw >> 62) & 1 == 1;
                    let next = (raw >> 51) & 0x7ff;
                    let kind = match (auth, bind) {
                        (true, true) => {
                            let mask = if pointer_format == DYLD_CHAINED_PTR_ARM64E_USERLAND24 { 0xff_ffff } else { 0xffff };
                            let import = (raw & mask) as usize;
                            let extra = *addends.get(import).ok_or(Error::Malformed("chained bind ordinal"))?;
                            FixupKind::Bind { import, addend: extra }
                        }
                        (false, true) => {
                            let mask = if pointer_format == DYLD_CHAINED_PTR_ARM64E_USERLAND24 { 0xff_ffff } else { 0xffff };
                            let import = (raw & mask) as usize;
                            let inline = ((raw >> 32) & 0x7_ffff) as i64;
                            let signed = if (inline & 0x4_0000) != 0 { inline | !0x7_ffff } else { inline };
                            let extra = *addends.get(import).ok_or(Error::Malformed("chained bind ordinal"))?;
                            FixupKind::Bind { import, addend: signed + extra }
                        }
                        (true, false) => {
                            let target = base + (raw & 0xffff_ffff);
                            FixupKind::Rebase { target, high8: 0 }
                        }
                        (false, false) => {
                            let target = raw & 0x7ff_ffff_ffff;
                            let high8 = ((raw >> 43) & 0xff) as u8;
                            let target = if pointer_format == DYLD_CHAINED_PTR_ARM64E { target } else { base + target };
                            FixupKind::Rebase { target, high8 }
                        }
                    };
                    (next, kind)
                } else {
                    let next = (raw >> 51) & 0xfff;
                    let kind = if raw >> 63 == 1 {
                        let import = (raw & 0xff_ffff) as usize;
                        let inline = ((raw >> 24) & 0xff) as i64;
                        let extra = *addends.get(import).ok_or(Error::Malformed("chained bind ordinal"))?;
                        FixupKind::Bind { import, addend: inline + extra }
                    } else {
                        let target = raw & 0xf_ffff_ffff;
                        let high8 = ((raw >> 36) & 0xff) as u8;
                        let target = if pointer_format == DYLD_CHAINED_PTR_64_OFFSET { base + target } else { target };
                        FixupKind::Rebase { target, high8 }
                    };
                    (next, kind)
                };
                fixups.push(Fixup { address, kind, lazy: false });
                if next == 0 {
                    break;
                }
                address += next * stride;
                steps += 1;
                if steps > page_size {
                    return Err(Error::Malformed("chain does not end in its page"));
                }
            }
        }
    }
    Ok(Fixups { imports, fixups })
}
