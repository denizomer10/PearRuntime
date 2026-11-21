//! Clean-room: unified PearRuntime facade tying together `pear-macho`,
//! `pear-dyld`, `pear-libobjc`, `pear-foundation`, `pear-host` (+ OS backend),
//! and `pear-image` with zero external crates.io dependencies.

pub use pear_dyld;
pub use pear_foundation;
pub use pear_graphics;
pub use pear_host;
pub use pear_image;
pub use pear_libobjc;
pub use pear_macho;

#[cfg(target_os = "linux")]
pub use pear_host_linux;

use std::ffi::{c_char, c_void};

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn realloc(p: *mut c_void, size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
    fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
    fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void;
    fn strlen(s: *const c_char) -> usize;
    fn strcmp(a: *const c_char, b: *const c_char) -> i32;
    fn getenv(name: *const c_char) -> *const c_char;
    fn abort() -> !;
}

extern "C" fn ca_current_media_time() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

extern "C" fn unwind_resume_trap() -> ! {
    eprintln!("[pear-runtime] _Unwind_Resume reached");
    std::process::abort()
}

/// Initializes the portable PearRuntime subsystems (ObjC runtime, Foundation,
/// and the platform's host backend when running on Linux).
pub fn init() {
    pear_libobjc::object::init_runtime();
    pear_libobjc::blocks::init_blocks();
    pear_foundation::init_foundation();
    #[cfg(target_os = "linux")]
    {
        pear_host_linux::LinuxHost::install().initialize();
    }
}

/// Unified symbol resolver for `pear-dyld`: resolves `libobjc`, `libSystem`
/// (libclosure + libc basics), `CoreFoundation`, and `Foundation` imports.
pub fn resolve_symbol(library: Option<&str>, symbol: &str) -> Option<usize> {
    init();
    match library {
        Some("/usr/lib/libobjc.A.dylib") => pear_libobjc::symbol(symbol),
        Some("/usr/lib/libSystem.B.dylib") => match symbol {
            "__Unwind_Resume" => Some(unwind_resume_trap as *const () as usize),
            "_malloc" => Some(malloc as *const () as usize),
            "_calloc" => Some(calloc as *const () as usize),
            "_realloc" => Some(realloc as *const () as usize),
            "_free" => Some(free as *const () as usize),
            "_memcpy" => Some(memcpy as *const () as usize),
            "_memset" => Some(memset as *const () as usize),
            "_strlen" => Some(strlen as *const () as usize),
            "_strcmp" => Some(strcmp as *const () as usize),
            "_getenv" => Some(getenv as *const () as usize),
            "_abort" => Some(abort as *const () as usize),
            _ => pear_libobjc::symbol(symbol),
        },
        Some(path) if path.contains("CoreFoundation") || path.contains("Foundation") => {
            pear_foundation::symbol(symbol)
        }
        Some(path) if path.contains("CoreGraphics") => pear_graphics::symbol(symbol),
        Some(path) if path.contains("QuartzCore") => match symbol {
            "_CACurrentMediaTime" => Some(ca_current_media_time as *const () as usize),
            _ => None,
        },
        None => pear_libobjc::symbol(symbol)
            .or_else(|| pear_foundation::symbol(symbol))
            .or_else(|| pear_graphics::symbol(symbol)),
        _ => None,
    }
}

/// Maps, fixes up, registers ObjC/Foundation metadata, calls `+load`, and runs
/// C/C++ initializers for an arm64 Mach-O image.
pub fn load_and_boot(
    data: &[u8],
    options: pear_dyld::Options,
) -> Result<(pear_dyld::LoadedImage, pear_libobjc::ImageReport), pear_dyld::Error> {
    init();
    let image = pear_dyld::load(data, &resolve_symbol, options)?;
    let report = unsafe { pear_libobjc::register_image(&image) };
    unsafe {
        pear_libobjc::call_load_methods(&image);
        image.run_initializers();
    }
    Ok((image, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_runtime_boots_both_objc_fixtures_and_foundation() {
        init();
        let f1 = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../pear-libobjc/fixtures/fixture-objc.dylib")).unwrap();
        let (img1, _) = load_and_boot(&f1, pear_dyld::Options::default()).unwrap();
        let run1: extern "C" fn() -> i32 = unsafe { std::mem::transmute(img1.symbol("_fixture_objc").unwrap()) };
        assert_eq!(run1(), 255);

        let f2 = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../pear-libobjc/fixtures/fixture2-objc.dylib")).unwrap();
        let (img2, _) = load_and_boot(&f2, pear_dyld::Options::default()).unwrap();
        let run2: extern "C" fn() -> i32 = unsafe { std::mem::transmute(img2.symbol("_fixture2_objc").unwrap()) };
        assert_eq!(run2(), 511);

        // Verify Foundation, CoreGraphics, QuartzCore & CFConstantString resolution
        assert!(resolve_symbol(Some("/System/Library/Frameworks/Foundation.framework/Foundation"), "_OBJC_CLASS_$_NSString").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"), "_CFStringCreateWithCString").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"), "___CFConstantStringClassReference").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics"), "_CGRectInset").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/QuartzCore.framework/QuartzCore"), "_CACurrentMediaTime").is_some());
    }
}
