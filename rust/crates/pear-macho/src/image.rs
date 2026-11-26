//! Clean-room: from Apple's open-source dyld and xnu headers (mach-o/loader.h,
//! mach-o/fixup-chains.h; APSL 2.0, used as reference only, nothing copied)
//! and the analysed apps' own binaries; checked against `dyld_info`.
//!
//! A 64-bit arm64 Mach-O image as a loader sees it: segments, sections,
//! linked dylibs, the entry point, fixups (rebases and binds, from either
//! dyld-info opcodes or chained fixups), exports and initializers.

use std::fmt;

const MH_MAGIC_64: u32 = 0xfeed_facf;
const FAT_MAGIC: u32 = 0xcafe_babe;
const FAT_MAGIC_64: u32 = 0xcafe_babf;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;

const LC_SEGMENT_64: u32 = 0x19;
const LC_LOAD_DYLIB: u32 = 0x0c;
const LC_ID_DYLIB: u32 = 0x0d;
const LC_LOAD_WEAK_DYLIB: u32 = 0x8000_0018;
const LC_REEXPORT_DYLIB: u32 = 0x8000_001f;
const LC_RPATH: u32 = 0x8000_001c;
const LC_LAZY_LOAD_DYLIB: u32 = 0x20;
const LC_LOAD_UPWARD_DYLIB: u32 = 0x8000_0023;
const LC_DYLD_INFO: u32 = 0x22;
const LC_DYLD_INFO_ONLY: u32 = 0x8000_0022;
const LC_MAIN: u32 = 0x8000_0028;
const LC_DYLD_EXPORTS_TRIE: u32 = 0x8000_0033;
const LC_DYLD_CHAINED_FIXUPS: u32 = 0x8000_0034;

pub const MH_EXECUTE: u32 = 2;
pub const MH_DYLIB: u32 = 6;
pub const MH_BUNDLE: u32 = 8;

/// Section types (`flags & SECTION_TYPE`) a loader acts on.
pub const S_ZEROFILL: u32 = 0x1;
pub const S_MOD_INIT_FUNC_POINTERS: u32 = 0x9;
pub const S_THREAD_LOCAL_VARIABLES: u32 = 0x13;
pub const S_INIT_FUNC_OFFSETS: u32 = 0x16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated(&'static str),
    Malformed(&'static str),
    Unsupported(&'static str),
    NoArm64Slice,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated(what) => write!(f, "truncated Mach-O: {what}"),
            Error::Malformed(what) => write!(f, "malformed Mach-O: {what}"),
            Error::Unsupported(what) => write!(f, "unsupported Mach-O feature: {what}"),
            Error::NoArm64Slice => write!(f, "no arm64 slice"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub name: String,
    pub vmaddr: u64,
    pub vmsize: u64,
    pub fileoff: u64,
    pub filesize: u64,
    pub maxprot: u32,
    pub initprot: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub segment: String,
    pub name: String,
    pub addr: u64,
    pub size: u64,
    pub offset: u32,
    pub flags: u32,
}

impl Section {
    pub fn kind(&self) -> u32 {
        self.flags & 0xff
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DylibKind {
    Load,
    Weak,
    Reexport,
    Lazy,
    Upward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dylib {
    pub path: String,
    pub kind: DylibKind,
}

/// Where an import is looked up (dyld's "library ordinal").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ordinal {
    /// `dylibs[n]` (ordinal n + 1 in the file).
    Dylib(usize),
    /// The image itself.
    SelfImage,
    /// The main executable.
    MainExecutable,
    /// Flat namespace: every loaded image in order.
    Flat,
    /// Weak definition coalescing: the first image that defines it.
    WeakLookup,
}

impl Ordinal {
    fn from_file(value: i64, dylib_count: usize) -> Result<Self, Error> {
        Ok(match value {
            0 => Ordinal::SelfImage,
            -1 => Ordinal::MainExecutable,
            -2 => Ordinal::Flat,
            -3 => Ordinal::WeakLookup,
            n if n > 0 && (n as usize) <= dylib_count => Ordinal::Dylib(n as usize - 1),
            _ => return Err(Error::Malformed("library ordinal")),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The symbol name as in the file (C names start with `_`).
    pub name: String,
    pub ordinal: Ordinal,
    /// Missing is allowed: the pointer becomes 0 (plus addend).
    pub weak: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixupKind {
    /// Pointer to this image's own `target` (unslid vmaddr); `high8` goes
    /// into the pointer's top byte.
    Rebase { target: u64, high8: u8 },
    /// Pointer to `imports[import]` plus `addend`.
    Bind { import: usize, addend: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixup {
    /// Unslid vmaddr of the 64-bit pointer to write.
    pub address: u64,
    pub kind: FixupKind,
    /// A lazy binding (dyld-info lazy stream); bound eagerly by a loader.
    pub lazy: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fixups {
    pub imports: Vec<Import>,
    pub fixups: Vec<Fixup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportKind {
    /// At `offset` from the image's base (mach header).
    Regular { offset: u64, weak: bool, thread_local: bool },
    /// Absolute value.
    Absolute { value: u64 },
    /// Re-exported from `dylibs[ordinal]` as `name` (empty: same name).
    Reexport { dylib: usize, name: String },
    /// A resolver function at `resolver` decides; `stub` is the fallback.
    Resolver { stub: u64, resolver: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
}

/// An initializer: a fixed-up function pointer stored at an (unslid)
/// address, or a function at an (unslid) address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Initializer {
    PointerAt(u64),
    Function(u64),
}

#[derive(Debug, Clone)]
pub struct MachO<'a> {
    data: &'a [u8],
    pub filetype: u32,
    pub flags: u32,
    pub segments: Vec<Segment>,
    pub sections: Vec<Section>,
    pub dylibs: Vec<Dylib>,
    pub rpaths: Vec<String>,
    pub install_name: Option<String>,
    /// LC_MAIN entry, as an offset from the mach header.
    pub entry_offset: Option<u64>,
    dyld_info: Option<[u32; 10]>,
    chained_fixups: Option<(u32, u32)>,
    exports_trie: Option<(u32, u32)>,
}

pub(crate) fn u16_at(d: &[u8], off: usize) -> Result<u16, Error> {
    d.get(off..off + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(Error::Truncated("u16"))
}
pub(crate) fn u32_at(d: &[u8], off: usize) -> Result<u32, Error> {
    d.get(off..off + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(Error::Truncated("u32"))
}
pub(crate) fn u64_at(d: &[u8], off: usize) -> Result<u64, Error> {
    d.get(off..off + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).ok_or(Error::Truncated("u64"))
}
fn fixed_str(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}
pub(crate) fn c_str(d: &[u8], off: usize) -> Result<String, Error> {
    let rest = d.get(off..).ok_or(Error::Truncated("string"))?;
    let end = rest.iter().position(|&c| c == 0).ok_or(Error::Truncated("string"))?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

/// The arm64 slice of a thin or fat file.
pub fn arm64_slice(data: &[u8]) -> Result<&[u8], Error> {
    let magic_be = data.get(..4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or(Error::Truncated("magic"))?;
    if magic_be == FAT_MAGIC || magic_be == FAT_MAGIC_64 {
        let be32 = |o: usize| -> Result<u32, Error> {
            data.get(o..o + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or(Error::Truncated("fat"))
        };
        let be64 = |o: usize| -> Result<u64, Error> {
            data.get(o..o + 8).map(|b| u64::from_be_bytes(b.try_into().unwrap())).ok_or(Error::Truncated("fat"))
        };
        let n = be32(4)? as usize;
        let entry = if magic_be == FAT_MAGIC { 20 } else { 32 };
        for i in 0..n {
            let at = 8 + i * entry;
            if be32(at)? == CPU_TYPE_ARM64 && be32(at + 4)? & 0x00ff_ffff == 0 {
                let (off, size) = if magic_be == FAT_MAGIC {
                    (be32(at + 8)? as usize, be32(at + 12)? as usize)
                } else {
                    (be64(at + 8)? as usize, be64(at + 16)? as usize)
                };
                return data.get(off..off + size).ok_or(Error::Truncated("fat slice"));
            }
        }
        return Err(Error::NoArm64Slice);
    }
    Ok(data)
}

impl<'a> MachO<'a> {
    /// Parses the arm64 slice of `data` (thin or fat).
    pub fn parse(data: &'a [u8]) -> Result<Self, Error> {
        let data = arm64_slice(data)?;
        if u32_at(data, 0)? != MH_MAGIC_64 || u32_at(data, 4)? != CPU_TYPE_ARM64 {
            return Err(Error::NoArm64Slice);
        }
        let filetype = u32_at(data, 12)?;
        let ncmds = u32_at(data, 16)? as usize;
        let flags = u32_at(data, 24)?;
        let mut m = MachO {
            data,
            filetype,
            flags,
            segments: Vec::new(),
            sections: Vec::new(),
            dylibs: Vec::new(),
            rpaths: Vec::new(),
            install_name: None,
            entry_offset: None,
            dyld_info: None,
            chained_fixups: None,
            exports_trie: None,
        };
        let mut off = 32;
        for _ in 0..ncmds {
            let cmd = u32_at(data, off)?;
            let size = u32_at(data, off + 4)? as usize;
            if size < 8 || off + size > data.len() {
                return Err(Error::Malformed("load command size"));
            }
            let lc = &data[off..off + size];
            match cmd {
                LC_SEGMENT_64 => {
                    let seg = Segment {
                        name: fixed_str(&lc[8..24]),
                        vmaddr: u64_at(lc, 24)?,
                        vmsize: u64_at(lc, 32)?,
                        fileoff: u64_at(lc, 40)?,
                        filesize: u64_at(lc, 48)?,
                        maxprot: u32_at(lc, 56)?,
                        initprot: u32_at(lc, 60)?,
                    };
                    let nsects = u32_at(lc, 64)? as usize;
                    for i in 0..nsects {
                        let s = lc.get(72 + i * 80..72 + (i + 1) * 80).ok_or(Error::Truncated("section"))?;
                        m.sections.push(Section {
                            name: fixed_str(&s[0..16]),
                            segment: fixed_str(&s[16..32]),
                            addr: u64_at(s, 32)?,
                            size: u64_at(s, 40)?,
                            offset: u32_at(s, 48)?,
                            flags: u32_at(s, 64)?,
                        });
                    }
                    m.segments.push(seg);
                }
                LC_LOAD_DYLIB | LC_LOAD_WEAK_DYLIB | LC_REEXPORT_DYLIB | LC_LAZY_LOAD_DYLIB | LC_LOAD_UPWARD_DYLIB => {
                    let kind = match cmd {
                        LC_LOAD_WEAK_DYLIB => DylibKind::Weak,
                        LC_REEXPORT_DYLIB => DylibKind::Reexport,
                        LC_LAZY_LOAD_DYLIB => DylibKind::Lazy,
                        LC_LOAD_UPWARD_DYLIB => DylibKind::Upward,
                        _ => DylibKind::Load,
                    };
                    let name_off = u32_at(lc, 8)? as usize;
                    m.dylibs.push(Dylib { path: c_str(lc, name_off)?, kind });
                }
                LC_ID_DYLIB => m.install_name = Some(c_str(lc, u32_at(lc, 8)? as usize)?),
                LC_RPATH => m.rpaths.push(c_str(lc, u32_at(lc, 8)? as usize)?),
                LC_MAIN => m.entry_offset = Some(u64_at(lc, 8)?),
                LC_DYLD_INFO | LC_DYLD_INFO_ONLY => {
                    let mut v = [0u32; 10];
                    for (i, slot) in v.iter_mut().enumerate() {
                        *slot = u32_at(lc, 8 + i * 4)?;
                    }
                    m.dyld_info = Some(v);
                }
                LC_DYLD_CHAINED_FIXUPS => m.chained_fixups = Some((u32_at(lc, 8)?, u32_at(lc, 12)?)),
                LC_DYLD_EXPORTS_TRIE => m.exports_trie = Some((u32_at(lc, 8)?, u32_at(lc, 12)?)),
                _ => {}
            }
            off += size;
        }
        if m.segments.is_empty() {
            return Err(Error::Malformed("no segments"));
        }
        Ok(m)
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The vmaddr of the mach header (the `__TEXT` segment at file offset 0).
    pub fn base_address(&self) -> u64 {
        self.segments.iter().find(|s| s.fileoff == 0 && s.filesize > 0).map_or(0, |s| s.vmaddr)
    }

    /// The span of all segments except `__PAGEZERO`: (lowest vmaddr, size).
    pub fn vm_range(&self) -> (u64, u64) {
        let mapped: Vec<&Segment> = self.segments.iter().filter(|s| s.name != "__PAGEZERO").collect();
        let lo = mapped.iter().map(|s| s.vmaddr).min().unwrap_or(0);
        let hi = mapped.iter().map(|s| s.vmaddr + s.vmsize).max().unwrap_or(0);
        (lo, hi - lo)
    }

    pub(crate) fn file_bytes(&self, off: u32, size: u32) -> Result<&'a [u8], Error> {
        self.data.get(off as usize..off as usize + size as usize).ok_or(Error::Truncated("linkedit blob"))
    }

    /// Reads the file's 64-bit value stored at `vmaddr`.
    pub fn read_u64_at_address(&self, vmaddr: u64) -> Result<u64, Error> {
        let seg = self
            .segments
            .iter()
            .find(|s| vmaddr >= s.vmaddr && vmaddr + 8 <= s.vmaddr + s.filesize)
            .ok_or(Error::Malformed("address outside file-backed segments"))?;
        u64_at(self.data, (seg.fileoff + (vmaddr - seg.vmaddr)) as usize)
    }

    /// All rebases and binds, from chained fixups or dyld-info opcodes.
    pub fn fixups(&self) -> Result<Fixups, Error> {
        if let Some((off, size)) = self.chained_fixups {
            return crate::fixups::chained(self, self.file_bytes(off, size)?);
        }
        if let Some(info) = self.dyld_info {
            return crate::fixups::opcodes(self, info);
        }
        Ok(Fixups::default())
    }

    /// Every exported symbol.
    pub fn exports(&self) -> Result<Vec<Export>, Error> {
        let trie = match (self.exports_trie, self.dyld_info) {
            (Some((off, size)), _) => self.file_bytes(off, size)?,
            (None, Some(info)) if info[9] > 0 => self.file_bytes(info[8], info[9])?,
            _ => return Ok(Vec::new()),
        };
        crate::exports::walk(trie)
    }

    /// Initializers in the order dyld runs them: function pointers in
    /// `S_MOD_INIT_FUNC_POINTERS` sections (read after fixups) and offsets
    /// in `S_INIT_FUNC_OFFSETS` sections.
    pub fn initializers(&self) -> Result<Vec<Initializer>, Error> {
        let mut out = Vec::new();
        let base = self.base_address();
        for s in &self.sections {
            match s.kind() {
                S_MOD_INIT_FUNC_POINTERS => {
                    for i in 0..s.size / 8 {
                        out.push(Initializer::PointerAt(s.addr + i * 8));
                    }
                }
                S_INIT_FUNC_OFFSETS => {
                    for i in 0..s.size / 4 {
                        let offset = u32_at(self.data, s.offset as usize + i as usize * 4)? as u64;
                        out.push(Initializer::Function(base + offset));
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// Sections of a type.
    pub fn sections_of_kind(&self, kind: u32) -> impl Iterator<Item = &Section> {
        self.sections.iter().filter(move |s| s.kind() == kind)
    }

    pub(crate) fn dylib_count(&self) -> usize {
        self.dylibs.len()
    }

    pub(crate) fn ordinal(&self, value: i64) -> Result<Ordinal, Error> {
        Ordinal::from_file(value, self.dylib_count())
    }
}
