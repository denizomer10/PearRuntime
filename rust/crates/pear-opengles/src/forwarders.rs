//! Every GLES prototype as a register-preserving trampoline that resolves its
//! target once through `PearANGLEGameSymbol` and tail-jumps to it, so no C
//! signature is needed and no system OpenGL ES is ever used. Shared by
//! libPearOpenGLES.dylib (Mac Catalyst, feature `catalyst`) and PearHost's
//! OpenGLES.framework (pear-gles includes this file); the including crate's
//! build script writes the list (gles_prototypes.rs).

// The trampolines are AArch64 (Apple silicon, and arm64 Linux/Windows);
// an x86-64 host needs its own pair (System V and Win64 argument registers).
#[cfg(not(target_arch = "aarch64"))]
compile_error!("the GLES forwarders are AArch64 only so far");

use std::ffi::{c_char, c_void};

extern "C" {
    #[link_name = "_Z19PearANGLEGameSymbolPKc"]
    pub(crate) fn PearANGLEGameSymbol(name: *const c_char) -> *mut c_void;
    #[cfg(feature = "catalyst")]
    #[link_name = "_Z14PearANGLETracePKcz"]
    fn PearANGLETrace(format: *const c_char, ...);
    #[cfg(not(feature = "catalyst"))]
    fn PearHostLogMessage(area: *const c_char, message: *const c_char);
    fn abort() -> !;
}

unsafe fn report_unavailable(name: *const c_char) {
    #[cfg(feature = "catalyst")]
    PearANGLETrace(c"[PearRuntime][angle] unavailable GLES entry: %s\n".as_ptr(), name);
    #[cfg(not(feature = "catalyst"))]
    {
        let text = format!("unavailable GLES entry: {}", std::ffi::CStr::from_ptr(name).to_string_lossy());
        let message = std::ffi::CString::new(text).unwrap_or_default();
        PearHostLogMessage(c"angle".as_ptr(), message.as_ptr());
    }
}

/// Slow path of every forwarder: resolve, cache, or abort (no silent no-op).
unsafe extern "C" fn resolve(name: *const c_char, slot: *mut usize) -> *mut c_void {
    let function = PearANGLEGameSymbol(name);
    if function.is_null() {
        report_unavailable(name);
        abort();
    }
    std::sync::atomic::AtomicUsize::from_ptr(slot).store(function as usize, std::sync::atomic::Ordering::Release);
    function
}

/// Saves the argument registers (x0–x8, q0–q7), resolves the forwarder named
/// in x17 into the slot in x16, restores the registers and tail-jumps.
#[unsafe(naked)]
unsafe extern "C" fn resolve_and_jump() {
    core::arch::naked_asm!(
        "stp x29, x30, [sp, #-16]!",
        "mov x29, sp",
        "sub sp, sp, #208",
        "stp x0, x1, [sp, #0]",
        "stp x2, x3, [sp, #16]",
        "stp x4, x5, [sp, #32]",
        "stp x6, x7, [sp, #48]",
        "str x8, [sp, #64]",
        "stp q0, q1, [sp, #80]",
        "stp q2, q3, [sp, #112]",
        "stp q4, q5, [sp, #144]",
        "stp q6, q7, [sp, #176]",
        "mov x0, x17",
        "mov x1, x16",
        "bl {resolve}",
        "mov x16, x0",
        "ldp x0, x1, [sp, #0]",
        "ldp x2, x3, [sp, #16]",
        "ldp x4, x5, [sp, #32]",
        "ldp x6, x7, [sp, #48]",
        "ldr x8, [sp, #64]",
        "ldp q0, q1, [sp, #80]",
        "ldp q2, q3, [sp, #112]",
        "ldp q4, q5, [sp, #144]",
        "ldp q6, q7, [sp, #176]",
        "add sp, sp, #208",
        "ldp x29, x30, [sp], #16",
        "br x16",
        resolve = sym resolve,
    )
}

/// The resolved target of a forwarder (written once by `resolve`).
#[repr(transparent)]
pub struct Slot(std::cell::UnsafeCell<usize>);
unsafe impl Sync for Slot {}

/// The page-relative address of a symbol, in the object format's syntax
/// (Mach-O `sym@PAGE`/`sym@PAGEOFF`, ELF `sym`/`:lo12:sym`); `sym` operands
/// already carry the platform's symbol prefix.
#[cfg(target_vendor = "apple")]
macro_rules! forwarder_asm {
    ($slot:path, $name:path) => {
        core::arch::naked_asm!(
            "adrp x16, {slot}@PAGE",
            "add x16, x16, {slot}@PAGEOFF",
            "ldr x17, [x16]",
            "cbz x17, 1f",
            "br x17",
            "1:",
            "adrp x17, {name}@PAGE",
            "ldr x17, [x17, {name}@PAGEOFF]",
            "b {slow}",
            slot = sym $slot,
            name = sym $name,
            slow = sym resolve_and_jump,
        )
    };
}
#[cfg(not(target_vendor = "apple"))]
macro_rules! forwarder_asm {
    ($slot:path, $name:path) => {
        core::arch::naked_asm!(
            "adrp x16, {slot}",
            "add x16, x16, :lo12:{slot}",
            "ldr x17, [x16]",
            "cbz x17, 1f",
            "br x17",
            "1:",
            "adrp x17, {name}",
            "ldr x17, [x17, :lo12:{name}]",
            "b {slow}",
            slot = sym $slot,
            name = sym $name,
            slow = sym resolve_and_jump,
        )
    };
}

macro_rules! forward {
    ($($name:ident),* $(,)?) => {$(
        mod $name {
            pub static SLOT: super::Slot = super::Slot(std::cell::UnsafeCell::new(0));
            pub static NAME: &[u8] = concat!(stringify!($name), "\0").as_bytes();
        }
        #[no_mangle]
        #[unsafe(naked)]
        pub unsafe extern "C" fn $name() {
            forwarder_asm!($name::SLOT, $name::NAME)
        }
    )*};
}

include!(concat!(env!("OUT_DIR"), "/forwarders.rs"));

