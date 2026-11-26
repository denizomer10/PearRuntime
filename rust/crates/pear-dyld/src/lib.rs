//! Clean-room: behaviour from Apple's open-source dyld (APSL 2.0, reference
//! only, nothing copied) and the public Mach-O headers; thread-local
//! variables from the TLV descriptor layout in those headers.
//!
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
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub use pear_macho;

#[derive(Debug)]
pub enum Error {
    MachO(pear_macho::Error),
    Io(PathBuf, std::io::Error),
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
            Error::Io(p, e) => write!(f, "{}: {e}", p.display()),
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
    pub dylibs: Vec<String>,
    pub rpaths: Vec<String>,
    pub path: Option<String>,
    /// The image's sections (unslid addresses; see [`LoadedImage::address`]).
    pub sections: Vec<pear_macho::Section>,
    pub entry_offset: Option<u64>,
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

    /// An exported symbol's address (Mach-O name, with its `_`), resolving
    /// re-exports through `resolver` when present.
    pub fn symbol_with_resolver(&self, name: &str, resolver: Option<&dyn Resolver>) -> Option<*const c_void> {
        match self.exports.get(name)? {
            ExportKind::Regular { offset, thread_local: false, .. } => {
                Some(self.address(self.header + offset) as *const c_void)
            }
            ExportKind::Absolute { value } => Some(*value as *const c_void),
            ExportKind::Resolver { resolver: res, .. } => {
                let f: extern "C" fn() -> usize = unsafe { std::mem::transmute(self.address(self.header + res)) };
                Some(f() as *const c_void)
            }
            ExportKind::Reexport { dylib, name: target_name } => {
                let res = resolver?;
                let dylib_path = self.dylibs.get(*dylib).map(String::as_str);
                let sym = if target_name.is_empty() { name } else { target_name.as_str() };
                res.resolve(dylib_path, sym).map(|a| a as *const c_void)
            }
            _ => None,
        }
    }

    /// An exported symbol's address (Mach-O name, with its `_`).
    pub fn symbol(&self, name: &str) -> Option<*const c_void> {
        self.symbol_with_resolver(name, None)
    }

    /// The `LC_MAIN` entry point address if present.
    pub fn entry(&self) -> Option<*const c_void> {
        self.entry_offset
            .map(|off| self.address(self.header + off) as *const c_void)
    }

    #[doc(hidden)]
    pub fn inject_reexport_for_test(&mut self, name: &str, dylib: usize, target: &str) {
        self.exports.insert(
            name.to_string(),
            ExportKind::Reexport { dylib, name: target.to_string() },
        );
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
    load_with_path(data, None, resolver, options)
}

/// Maps `data` (a thin or fat Mach-O; its arm64 slice), associating an
/// optional filesystem path with the loaded image.
pub fn load_with_path(
    data: &[u8],
    path: Option<String>,
    resolver: &dyn Resolver,
    options: Options,
) -> Result<LoadedImage, Error> {
    let m = MachO::parse(data)?;
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
        dylibs: m.dylibs.iter().map(|d| d.path.clone()).collect(),
        rpaths: m.rpaths.clone(),
        path,
        sections: m.sections.clone(),
        entry_offset: m.entry_offset,
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
    let trace_missing = std::env::var_os("PEAR_DYLD_TRACE").is_some();
    for f in &fixups.fixups {
        let value = match f.kind {
            FixupKind::Rebase { target, high8 } => target.wrapping_add(slide) | (high8 as u64) << 56,
            FixupKind::Bind { import, addend } => match resolved[import] {
                Some(address) => (address as u64).wrapping_add(addend as u64),
                // dyld leaves weak imports that are missing at 0.
                None if fixups.imports[import].weak => 0,
                None => {
                    if trace_missing {
                        eprintln!(
                            "[pear-dyld][missing-bind] addr=0x{:x} sym={} ord={:?}",
                            f.address, fixups.imports[import].name, fixups.imports[import].ordinal
                        );
                    }
                    missing_symbol_called as *const () as usize as u64
                }
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
        let mut init_fns = Vec::new();
        for s in m.sections_of_kind(S_THREAD_LOCAL_INIT_FUNCTION_POINTERS) {
            let count = (s.size / 8) as usize;
            let base = image.address(s.addr) as *const usize;
            for i in 0..count {
                let p = unsafe { *base.add(i) };
                if p != 0 {
                    init_fns.push(p);
                }
            }
        }
        let key = tlv::register(template, init_fns);
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
    let own = || image.symbol_with_resolver(&import.name, Some(resolver)).map(|p| p as usize);
    match import.ordinal {
        Ordinal::Dylib(i) => resolver.resolve(Some(&m.dylibs[i].path), &import.name),
        Ordinal::SelfImage => own(),
        Ordinal::MainExecutable | Ordinal::Flat => resolver.resolve(None, &import.name).or_else(own),
        // Weak definitions coalesce to the first definition: another
        // image's if the resolver knows one, else this image's own.
        Ordinal::WeakLookup => resolver.resolve(None, &import.name).or_else(own),
    }
}

fn expand_dyld_token(raw: &str, executable_dir: &Path, loader_dir: &Path) -> Option<PathBuf> {
    if let Some(rest) = raw.strip_prefix("@executable_path") {
        let rel = rest.strip_prefix('/').unwrap_or(rest);
        Some(if rel.is_empty() { executable_dir.to_path_buf() } else { executable_dir.join(rel) })
    } else if let Some(rest) = raw.strip_prefix("@loader_path") {
        let rel = rest.strip_prefix('/').unwrap_or(rest);
        Some(if rel.is_empty() { loader_dir.to_path_buf() } else { loader_dir.join(rel) })
    } else if Path::new(raw).is_absolute() {
        Some(PathBuf::from(raw))
    } else {
        None
    }
}

/// Resolves an embedded dylib/framework install name (`@rpath/...`,
/// `@executable_path/...`, `@loader_path/...`) to a file on disk, skipping
/// system paths (`/System/...`, `/usr/lib/...`) so host resolvers handle them.
pub fn resolve_dylib_path(
    dylib: &str,
    executable_dir: &Path,
    loader_dir: &Path,
    rpaths: &[String],
) -> Option<PathBuf> {
    if dylib.starts_with("/System/") || dylib.starts_with("/usr/lib/") {
        return None;
    }
    if let Some(rest) = dylib.strip_prefix("@rpath") {
        let rel = rest.strip_prefix('/').unwrap_or(rest);
        for rp in rpaths {
            if let Some(base) = expand_dyld_token(rp, executable_dir, loader_dir) {
                let cand = base.join(rel);
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
        let fallback = executable_dir.join("Frameworks").join(rel);
        if fallback.is_file() {
            return Some(fallback);
        }
        return None;
    }
    let cand = expand_dyld_token(dylib, executable_dir, loader_dir)?;
    cand.is_file().then_some(cand)
}

/// A loaded main executable together with its embedded `@rpath` / `@executable_path` /
/// `@loader_path` dependencies, ordered cleanly for initialization (`init_order`).
pub struct ImageSet {
    /// Image `0` is always the main executable, followed by its embedded dependencies.
    pub images: Vec<LoadedImage>,
    /// Topological initialization order (dependencies before dependents; main executable last).
    pub init_order: Vec<usize>,
    path_index: HashMap<String, usize>,
}

impl ImageSet {
    pub fn main_image(&self) -> &LoadedImage {
        &self.images[0]
    }

    pub fn image_count(&self) -> u32 {
        self.images.len() as u32
    }

    pub fn symbol(&self, library: Option<&str>, name: &str, fallback: &dyn Resolver) -> Option<usize> {
        let res = LoadedBundleResolver {
            images: &self.images,
            path_index: &self.path_index,
            fallback,
        };
        res.resolve(library, name)
    }

    /// Runs initializers across all loaded images in dependency-first order.
    ///
    /// # Safety
    /// Executes constructor functions inside the guest images.
    pub unsafe fn run_initializers(&self) {
        for &idx in &self.init_order {
            if let Some(img) = self.images.get(idx) {
                img.run_initializers();
            }
        }
    }
}

struct LoadedBundleResolver<'a> {
    images: &'a [LoadedImage],
    path_index: &'a HashMap<String, usize>,
    fallback: &'a dyn Resolver,
}

impl Resolver for LoadedBundleResolver<'_> {
    fn resolve(&self, library: Option<&str>, symbol: &str) -> Option<usize> {
        if let Some(lib) = library {
            if let Some(&idx) = self.path_index.get(lib) {
                return self.images[idx].symbol_with_resolver(symbol, Some(self)).map(|p| p as usize);
            }
            return self.fallback.resolve(Some(lib), symbol);
        }
        for img in self.images {
            if let Some(p) = img.symbol_with_resolver(symbol, Some(self)) {
                return Some(p as usize);
            }
        }
        self.fallback.resolve(None, symbol)
    }
}

struct DiscoveredNode {
    canon_path: PathBuf,
    bytes: Vec<u8>,
    aliases: Vec<String>,
    deps: Vec<usize>,
    install_name: Option<String>,
}

fn post_order_dfs(
    node: usize,
    nodes: &[DiscoveredNode],
    visited: &mut HashSet<usize>,
    visiting: &mut HashSet<usize>,
    out: &mut Vec<usize>,
) {
    if visited.contains(&node) || !visiting.insert(node) {
        return;
    }
    for &dep in &nodes[node].deps {
        post_order_dfs(dep, nodes, visited, visiting, out);
    }
    visiting.remove(&node);
    visited.insert(node);
    out.push(node);
}

/// Loads `executable_path` together with any on-disk embedded dependencies referenced
/// via `@rpath`, `@executable_path`, or `@loader_path`, binding cross-image imports
/// before delegating system framework imports to `resolver`.
pub fn load_bundle(executable_path: &Path, resolver: &dyn Resolver, options: Options) -> Result<ImageSet, Error> {
    let main_canon = fs::canonicalize(executable_path).unwrap_or_else(|_| executable_path.to_path_buf());
    let executable_dir = main_canon.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    let main_bytes = fs::read(&main_canon).map_err(|e| Error::Io(main_canon.clone(), e))?;

    let mut nodes: Vec<DiscoveredNode> = Vec::new();
    let mut canon_to_node: HashMap<PathBuf, usize> = HashMap::new();
    let mut queue: Vec<(usize, Vec<String>)> = Vec::new();

    canon_to_node.insert(main_canon.clone(), 0);
    nodes.push(DiscoveredNode {
        canon_path: main_canon,
        bytes: main_bytes,
        aliases: Vec::new(),
        deps: Vec::new(),
        install_name: None,
    });
    queue.push((0, Vec::new()));

    let mut q_idx = 0;
    while q_idx < queue.len() {
        let (node_idx, inherited_rpaths) = queue[q_idx].clone();
        q_idx += 1;

        let loader_dir = nodes[node_idx]
            .canon_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let (dylib_paths, own_rpaths, install_name) = {
            let m = MachO::parse(&nodes[node_idx].bytes)?;
            (
                m.dylibs.into_iter().map(|d| d.path).collect::<Vec<_>>(),
                m.rpaths,
                m.install_name,
            )
        };
        nodes[node_idx].install_name = install_name;

        let mut search_rpaths = own_rpaths;
        for rp in inherited_rpaths {
            if !search_rpaths.contains(&rp) {
                search_rpaths.push(rp);
            }
        }

        let mut dep_indices = Vec::new();
        for dylib in dylib_paths {
            let Some(resolved_path) = resolve_dylib_path(&dylib, &executable_dir, &loader_dir, &search_rpaths) else {
                continue;
            };
            let dep_canon = fs::canonicalize(&resolved_path).unwrap_or(resolved_path);
            let dep_idx = if let Some(&existing) = canon_to_node.get(&dep_canon) {
                if !nodes[existing].aliases.contains(&dylib) {
                    nodes[existing].aliases.push(dylib);
                }
                existing
            } else {
                let bytes = match fs::read(&dep_canon) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                if MachO::parse(&bytes).is_err() {
                    continue;
                }
                let idx = nodes.len();
                canon_to_node.insert(dep_canon.clone(), idx);
                nodes.push(DiscoveredNode {
                    canon_path: dep_canon,
                    bytes,
                    aliases: vec![dylib],
                    deps: Vec::new(),
                    install_name: None,
                });
                queue.push((idx, search_rpaths.clone()));
                idx
            };
            if !dep_indices.contains(&dep_idx) {
                dep_indices.push(dep_idx);
            }
        }
        nodes[node_idx].deps = dep_indices;
    }

    let mut load_order = Vec::with_capacity(nodes.len());
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    post_order_dfs(0, &nodes, &mut visited, &mut visiting, &mut load_order);

    // Load in topological dependency-first order.
    let mut loaded_nodes: Vec<Option<LoadedImage>> = (0..nodes.len()).map(|_| None).collect();
    for &node_idx in &load_order {
        let mut active_images: Vec<&LoadedImage> = Vec::new();
        let mut node_to_active: HashMap<usize, usize> = HashMap::new();
        for (i, slot) in loaded_nodes.iter().enumerate() {
            if let Some(img) = slot.as_ref() {
                node_to_active.insert(i, active_images.len());
                active_images.push(img);
            }
        }
        struct ActiveSliceResolver<'a> {
            active: Vec<&'a LoadedImage>,
            key_to_active: HashMap<String, usize>,
            fallback: &'a dyn Resolver,
        }
        impl Resolver for ActiveSliceResolver<'_> {
            fn resolve(&self, library: Option<&str>, symbol: &str) -> Option<usize> {
                if let Some(lib) = library {
                    if let Some(&ai) = self.key_to_active.get(lib) {
                        return self.active[ai].symbol_with_resolver(symbol, Some(self)).map(|p| p as usize);
                    }
                    return self.fallback.resolve(Some(lib), symbol);
                }
                for img in &self.active {
                    if let Some(p) = img.symbol_with_resolver(symbol, Some(self)) {
                        return Some(p as usize);
                    }
                }
                self.fallback.resolve(None, symbol)
            }
        }
        let mut key_to_active = HashMap::new();
        for (&n_idx, &a_idx) in &node_to_active {
            key_to_active.insert(nodes[n_idx].canon_path.to_string_lossy().into_owned(), a_idx);
            if let Some(ref inst) = nodes[n_idx].install_name {
                key_to_active.insert(inst.clone(), a_idx);
            }
            for alias in &nodes[n_idx].aliases {
                key_to_active.insert(alias.clone(), a_idx);
            }
        }
        let step_resolver = ActiveSliceResolver {
            active: active_images,
            key_to_active,
            fallback: resolver,
        };
        let path_str = nodes[node_idx].canon_path.to_string_lossy().into_owned();
        let img = load_with_path(&nodes[node_idx].bytes, Some(path_str), &step_resolver, options)?;
        loaded_nodes[node_idx] = Some(img);
    }

    // Place node 0 (main executable) at images[0], preserving init_order indices.
    let mut images: Vec<LoadedImage> = Vec::with_capacity(load_order.len());
    let mut node_to_final: HashMap<usize, usize> = HashMap::new();
    let main_img = loaded_nodes[0].take().ok_or(Error::Map)?;
    images.push(main_img);
    node_to_final.insert(0, 0);
    for (n_idx, slot) in loaded_nodes.iter_mut().enumerate() {
        if n_idx == 0 {
            continue;
        }
        if let Some(img) = slot.take() {
            let final_idx = images.len();
            images.push(img);
            node_to_final.insert(n_idx, final_idx);
        }
    }

    let mut path_index = HashMap::new();
    for (&n_idx, &f_idx) in &node_to_final {
        path_index.insert(nodes[n_idx].canon_path.to_string_lossy().into_owned(), f_idx);
        if let Some(ref inst) = nodes[n_idx].install_name {
            path_index.insert(inst.clone(), f_idx);
        }
        for alias in &nodes[n_idx].aliases {
            path_index.insert(alias.clone(), f_idx);
        }
    }
    let init_order = load_order
        .into_iter()
        .filter_map(|n| node_to_final.get(&n).copied())
        .collect();

    Ok(ImageSet {
        images,
        init_order,
        path_index,
    })
}
