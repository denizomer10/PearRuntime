//! libPearOpenGLES.dylib: the GLES/EAGL ABI a converted iOS game links, on
//! ANGLE. Every exported `gl*` symbol is a forwarder (forwarders.rs) resolved
//! through the runtime's routing table (`PearANGLEGameSymbol`).

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_void};

mod forwarders;
pub use forwarders::*;
use forwarders::PearANGLEGameSymbol;

// ---------------------------------------------------------------------------
// EAGL C ABI: string constants and entry points (Mac Catalyst; PearHost's are
// in pear-gles).
#[cfg(feature = "catalyst")]
pub use eagl_abi::*;
#[cfg(feature = "catalyst")]
mod eagl_abi {
use super::*;

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
}
