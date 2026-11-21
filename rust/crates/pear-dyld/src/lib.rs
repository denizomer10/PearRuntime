//! PearRuntime's own loader for 64-bit (arm64) Mach-O images: what dyld does
//! for an iOS image, without dyld, on any arm64 host.
//!
//! [`load`] reserves the image's address range, copies its segments, applies
//! every rebase and bind (dyld-info opcodes or chained fixups, both through
//! pear-macho; lazy bindings are bound up front), sets segment protections,
//! makes the code executable and returns a [`LoadedImage`] whose
//! [`LoadedImage::run_initializers`] and [`LoadedImage::symbol`] complete the
//! job. Imports are resolved by a [`Resolver`] the caller supplies: the host
//! libraries on macOS, PearRuntime's own frameworks elsewhere.
//!
//! Thread-local variables (`__thread_vars`) get PearRuntime's own
//! `tlv_get_addr` equivalent (see `tlv`). Not yet: re-exported symbols,
//! thread-local initializer sections, arm64e pointer authentication.

mod memory;
#[cfg(target_arch = "aarch64")]
mod tlv;

use pear_macho::{ExportKind, FixupKind, Import, Initializer, MachO, Ordinal};
use std::collections::HashMap;
use std::ffi::c_void;
use std::fmt;

pub use pear_macho;

#[derive(Debug)]
pub enum Error {
    MachO(pear_macho::Error),
    /// No memory for the image.
    Map,
    /// A segment protection could not be applied.
    Protect(String),
    /// Imports no resolver supplied (and that are not weak).
    MissingSymbols(Vec<String>),
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MachO(e) => write!(f, "{e}"),
            Error::Map => write!(f, "could not reserve memory for the image"),
            Error::Protect(seg) => write!(f, "could not protect segment {seg}"),
            Error::MissingSymbols(names) => write!(f, "missing symbols: {}", names.join(", ")),
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<pear_macho::Error> for Error {
    fn from(e: pear_macho::Error) -> Self {
        Error::MachO(e)
    }
}

/// Supplies the address of an imported symbol.
pub trait Resolver {
    /// `library` is the install name of the dylib the import names (None for
    /// flat-namespace and weak-definition lookups); `symbol` is the Mach-O
    /// name (C names start with `_`). None: not found.
    fn resolve(&self, library: Option<&str>, symbol: &str) -> Option<usize>;
}

impl<F: Fn(Option<&str>, &str) -> Option<usize>> Resolver for F {
    fn resolve(&self, library: Option<&str>, symbol: &str) -> Option<usize> {
        self(library, symbol)
    }
}

#[derive(Default, Clone, Copy)]
pub struct Options {
    /// Bind imports nothing supplies to [`missing_symbol_called`] instead of
    /// failing the load (an app may never call them).
    pub allow_missing: bool,
}

/// Where unresolved imports point under [`Options::allow_missing`].
pub extern "C" fn missing_symbol_called() -> ! {
    eprintln!("[PearRuntime][dyld] an unresolved import was called");
    std::process::abort()
}

/// dyld_stub_binder: never reached, since lazy pointers are bound at load.
extern "C" fn stub_binder_called() -> ! {
    eprintln!("[PearRuntime][dyld] dyld_stub_binder called (a lazy pointer was not bound)");
    std::process::abort()
}

/// A mapped, fixed-up image.
pub struct LoadedImage {
    region: memory::Region,
    /// Unslid vmaddr of the region's start.
    low: u64,
    /// Unslid vmaddr of the mach header.
    header: u64,
    exports: HashMap<String, ExportKind>,
    initializers: Vec<Initializer>,
    /// Imports bound to [`missing_symbol_called`].
    pub missing: Vec<String>,
    pub install_name: Option<String>,
    /// The image's sections (unslid addresses; see [`LoadedImage::address`]).
    pub sections: Vec<pear_macho::Section>,
}

impl LoadedImage {
    /// The live address of an unslid vmaddr of the image.
    pub fn address(&self, vmaddr: u64) -> *mut u8 {
        unsafe { self.region.start.add((vmaddr - self.low) as usize) }
    }

    /// A section by segment and section name.
    pub fn section(&self, segment: &str, name: &str) -> Option<&pear_macho::Section> {
        self.sections.iter().find(|s| s.segment == segment && s.name == name)
    }

    /// Sections named `name` in any segment (Objective-C metadata moves
    /// between __DATA and __DATA_CONST across toolchains).
    pub fn sections_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a pear_macho::Section> + 'a {
        self.sections.iter().filter(move |s| s.name == name)
    }

    /// The mach header in memory.
    pub fn header(&self) -> *mut u8 {
        self.address(self.header)
    }

    /// How far the image moved from its link-time addresses.
    pub fn slide(&self) -> i64 {
        (self.region.start as u64).wrapping_sub(self.low) as i64
    }

    /// An exported symbol's address (Mach-O name, with its `_`).
    pub fn symbol(&self, name: &str) -> Option<*const c_void> {
        match self.exports.get(name)? {
            ExportKind::Regular { offset, thread_local: false, .. } => {
                Some(self.address(self.header + offset) as *const c_void)
            }
            ExportKind::Absolute { value } => Some(*value as *const c_void),
            ExportKind::Resolver { resolver, .. } => {
                let f: extern "C" fn() -> usize = unsafe { std::mem::transmute(self.address(self.header + resolver)) };
                Some(f() as *const c_void)
            }
            _ => None,
        }
    }

    /// Runs the image's initializers in order, with dyld's arguments
    /// (argc, argv, envp, apple, program vars), as empty lists.
    ///
    /// # Safety
    /// Runs the image's code.
    pub unsafe fn run_initializers(&self) {
        static EMPTY: [usize; 1] = [0];
        let list = EMPTY.as_ptr() as *const *const i8;
        for init in &self.initializers {
            let address = match *init {
                Initializer::PointerAt(slot) => *(self.address(slot) as *const usize),
                Initializer::Function(f) => self.address(f) as usize,
            };
            let f: extern "C" fn(i32, *const *const i8, *const *const i8, *const *const i8, *const c_void) =
                std::mem::transmute(address);
            f(0, list, list, list, std::ptr::null());
        }
    }
}

/// Maps `data` (a thin or fat Mach-O; its arm64 slice) and applies its
/// fixups. Initializers are not run yet (see [`LoadedImage::run_initializers`]).
pub fn load(data: &[u8], resolver: &dyn Resolver, options: Options) -> Result<LoadedImage, Error> {
    let m = MachO::parse(data)?;
    if m.sections_of_kind(S_THREAD_LOCAL_INIT_FUNCTION_POINTERS).next().is_some() {
        return Err(Error::Unsupported("thread-local initializer functions"));
    }
    let (low, size) = m.vm_range();
    let region = memory::Region::reserve(size as usize).ok_or(Error::Map)?;
    let slice = m.data();
    for seg in m.segments.iter().filter(|s| s.name != "__PAGEZERO" && s.filesize > 0) {
        let from = slice
            .get(seg.fileoff as usize..(seg.fileoff + seg.filesize) as usize)
            .ok_or(pear_macho::Error::Truncated("segment"))?;
        let count = from.len().min(seg.vmsize as usize);
        unsafe {
            std::ptr::copy_nonoverlapping(from.as_ptr(), region.start.add((seg.vmaddr - low) as usize), count);
        }
    }

    let header = m.base_address();
    let exports: HashMap<String, ExportKind> = m.exports()?.into_iter().map(|e| (e.name, e.kind)).collect();
    let mut image = LoadedImage {
        region,
        low,
        header,
        exports,
        initializers: m.initializers()?,
        missing: Vec::new(),
        install_name: m.install_name.clone(),
        sections: m.sections.clone(),
    };

    let fixups = m.fixups()?;
    let mut resolved: Vec<Option<usize>> = Vec::with_capacity(fixups.imports.len());
    let mut missing = Vec::new();
    for import in &fixups.imports {
        let address = resolve(&m, &image, import, resolver);
        if address.is_none() && !import.weak {
            missing.push(import.name.clone());
        }
        resolved.push(address);
    }
    if !missing.is_empty() && !options.allow_missing {
        missing.sort();
        missing.dedup();
        return Err(Error::MissingSymbols(missing));
    }
    let slide = image.slide() as u64;
    for f in &fixups.fixups {
        let value = match f.kind {
            FixupKind::Rebase { target, high8 } => target.wrapping_add(slide) | (high8 as u64) << 56,
            FixupKind::Bind { import, addend } => match resolved[import] {
                Some(address) => (address as u64).wrapping_add(addend as u64),
                // dyld leaves weak imports that are missing at 0.
                None if fixups.imports[import].weak => 0,
                None => missing_symbol_called as *const () as usize as u64,
            },
        };
        unsafe { (image.address(f.address) as *mut u64).write_unaligned(value) };
    }
    missing.sort();
    missing.dedup();
    image.missing = missing;

    setup_thread_locals(&m, &image)?;

    // Segment protections; code becomes visible to instruction fetch.
    for seg in m.segments.iter().filter(|s| s.name != "__PAGEZERO" && s.vmsize > 0) {
        let prot = (if seg.initprot & 1 != 0 { memory::PROT_READ } else { 0 })
            | (if seg.initprot & 2 != 0 { memory::PROT_WRITE } else { 0 })
            | (if seg.initprot & 4 != 0 { memory::PROT_EXEC } else { 0 });
        let offset = (seg.vmaddr - low) as usize;
        if seg.initprot & 4 != 0 {
            memory::flush_icache(unsafe { image.region.start.add(offset) }, seg.vmsize as usize);
        }
        if !image.region.protect(offset, seg.vmsize as usize, prot) {
            return Err(Error::Protect(seg.name.clone()));
        }
    }
    Ok(image)
}

const S_THREAD_LOCAL_REGULAR: u32 = 0x11;
const S_THREAD_LOCAL_ZEROFILL: u32 = 0x12;
const S_THREAD_LOCAL_INIT_FUNCTION_POINTERS: u32 = 0x15;

/// Registers the image's thread-local template and points every descriptor
/// at the thunk with the image's key (what dyld does with `tlv_get_addr`).
/// Descriptor offsets are relative to the start of the template, the first
/// thread-local data or zero-fill section.
fn setup_thread_locals(m: &MachO, image: &LoadedImage) -> Result<(), Error> {
    let descriptors: Vec<_> = m.sections_of_kind(pear_macho::image::S_THREAD_LOCAL_VARIABLES).collect();
    if descriptors.is_empty() {
        return Ok(());
    }
    #[cfg(not(target_arch = "aarch64"))]
    return Err(Error::Unsupported("thread-local variables on this architecture"));
    #[cfg(target_arch = "aarch64")]
    {
        let parts: Vec<_> = m
            .sections
            .iter()
            .filter(|s| s.kind() == S_THREAD_LOCAL_REGULAR || s.kind() == S_THREAD_LOCAL_ZEROFILL)
            .collect();
        let start = parts.iter().map(|s| s.addr).min().unwrap_or(0);
        let end = parts.iter().map(|s| s.addr + s.size).max().unwrap_or(start);
        let mut template = vec![0u8; (end - start) as usize];
        for s in parts.iter().filter(|s| s.kind() == S_THREAD_LOCAL_REGULAR) {
            // Initial values as fixed up in the image (pointers are slid).
            let at = (s.addr - start) as usize;
            unsafe {
                std::ptr::copy_nonoverlapping(image.address(s.addr), template[at..].as_mut_ptr(), s.size as usize);
            }
        }
        let key = tlv::register(template);
        for s in descriptors {
            for i in 0..s.size / 24 {
                let d = image.address(s.addr + i * 24) as *mut tlv::Descriptor;
                unsafe {
                    (*d).thunk = tlv::thunk as *const () as usize;
                    (*d).key = key;
                }
            }
        }
        Ok(())
    }
}

fn resolve(m: &MachO, image: &LoadedImage, import: &Import, resolver: &dyn Resolver) -> Option<usize> {
    if import.name == "dyld_stub_binder" {
        return Some(stub_binder_called as *const () as usize);
    }
    // Descriptor thunks are set by setup_thread_locals.
    if import.name == "__tlv_bootstrap" {
        return Some(stub_binder_called as *const () as usize);
    }
    let own = || image.symbol(&import.name).map(|p| p as usize);
    match import.ordinal {
        Ordinal::Dylib(i) => resolver.resolve(Some(&m.dylibs[i].path), &import.name),
        Ordinal::SelfImage => own(),
        Ordinal::MainExecutable | Ordinal::Flat => resolver.resolve(None, &import.name).or_else(own),
        // Weak definitions coalesce to the first definition: another
        // image's if the resolver knows one, else this image's own.
        Ordinal::WeakLookup => resolver.resolve(None, &import.name).or_else(own),
    }
}
