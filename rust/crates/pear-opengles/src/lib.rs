//! libPearOpenGLES.dylib: the GLES/EAGL ABI a converted iOS game links, on
//! ANGLE. Every exported `gl*` symbol is a register-preserving trampoline that
//! resolves its target once through the runtime's routing table
//! (`PearANGLEGameSymbol`) and tail-jumps to it, so no C signature is needed
//! and no system OpenGL ES is ever used.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_void};

extern "C" {
    #[link_name = "_Z19PearANGLEGameSymbolPKc"]
    fn PearANGLEGameSymbol(name: *const c_char) -> *mut c_void;
    #[link_name = "_Z14PearANGLETracePKcz"]
    fn PearANGLETrace(format: *const c_char, ...);
    fn abort() -> !;
}

/// Slow path of every forwarder: resolve, cache, or abort (no silent no-op).
unsafe extern "C" fn resolve(name: *const c_char, slot: *mut usize) -> *mut c_void {
    let function = PearANGLEGameSymbol(name);
    if function.is_null() {
        PearANGLETrace(c"[PearRuntime][angle] unavailable GLES entry: %s\n".as_ptr(), name);
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

macro_rules! forward {
    ($($name:ident),* $(,)?) => {$(
        mod $name {
            pub static SLOT: super::Slot = super::Slot(std::cell::UnsafeCell::new(0));
            pub static NAME: &[u8] = concat!(stringify!($name), "\0").as_bytes();
        }
        #[no_mangle]
        #[unsafe(naked)]
        pub unsafe extern "C" fn $name() {
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
                slot = sym $name::SLOT,
                name = sym $name::NAME,
                slow = sym resolve_and_jump,
            )
        }
    )*};
}

include!(concat!(env!("OUT_DIR"), "/forwarders.rs"));

// ---------------------------------------------------------------------------
// EAGL C ABI: string constants and entry points.

#[repr(C)]
pub struct ConstantString {
    isa: *const c_void,
    flags: u32,
    bytes: *const u8,
    length: usize,
}
unsafe impl Sync for ConstantString {}

#[repr(transparent)]
pub struct StringRef(*const ConstantString);
unsafe impl Sync for StringRef {}

extern "C" {
    static __CFConstantStringClassReference: c_void;
}

macro_rules! constant_string {
    ($export:ident, $value:literal) => {
        const _: () = {
            #[link_section = "__DATA,__cfstring"]
            static STRING: ConstantString = ConstantString {
                isa: &raw const __CFConstantStringClassReference,
                flags: 0x7c8,
                bytes: concat!($value, "\0").as_ptr(),
                length: $value.len(),
            };
            #[no_mangle]
            pub static $export: StringRef = StringRef(&STRING);
        };
    };
}

constant_string!(kEAGLDrawablePropertyColorFormat, "EAGLDrawablePropertyColorFormat");
constant_string!(kEAGLDrawablePropertyRetainedBacking, "EAGLDrawablePropertyRetained");
constant_string!(kEAGLColorFormatRGBA8, "EAGLColorFormat8888");
constant_string!(kEAGLColorFormatRGB565, "EAGLColorFormat565");
constant_string!(kEAGLColorFormatSRGBA8, "EAGLColorFormatS8888");

#[no_mangle]
pub unsafe extern "C" fn EAGLGetVersion(major: *mut u32, minor: *mut u32) {
    if !major.is_null() {
        *major = 1;
    }
    if !minor.is_null() {
        *minor = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn EAGLGetProcAddress(name: *const c_char) -> *mut c_void {
    PearANGLEGameSymbol(name)
}
