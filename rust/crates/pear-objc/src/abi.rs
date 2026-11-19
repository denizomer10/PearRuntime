//! Objective-C ABI data that compiled iOS binaries bind to, emitted from
//! Rust the way clang emits it from Objective-C source:
//!
//! - static classes (`OBJC_CLASS_$_…` / `OBJC_METACLASS_$_…`, class_ro data,
//!   method, ivar and property lists, `__objc_classlist` entries): apps bind
//!   to and subclass them before any initializer runs; the runtime realizes
//!   them from the class list and uniques the selector strings;
//! - constant NSStrings (`__cfstring` objects behind exported pointers);
//! - `@try`/`@catch`: Rust cannot catch a foreign (Objective-C) exception, so
//!   the catching frames are assembly with the Objective-C personality and an
//!   exception table, exactly as clang generates them.
//!
//! An image that defines static classes must also carry `image_info!()`.

use crate::{Id, Sel};
use std::cell::UnsafeCell;
use std::ffi::{c_char, c_int, c_void};

/// Data the Objective-C runtime writes to (realization, selector fixups, ivar
/// sliding) lives in `__DATA` behind an `UnsafeCell`.
#[repr(transparent)]
pub struct Writable<T>(pub UnsafeCell<T>);
unsafe impl<T> Sync for Writable<T> {}
impl<T> Writable<T> {
    pub const fn new(v: T) -> Self {
        Writable(UnsafeCell::new(v))
    }
    pub const fn ptr(&self) -> *const c_void {
        self.0.get() as *const c_void
    }
}

/// A read-only pointer that may live in a `static`.
#[repr(transparent)]
pub struct Ptr(pub *const c_void);
unsafe impl Sync for Ptr {}

#[repr(C)]
pub struct ClassT {
    pub isa: *const c_void,
    pub superclass: *const c_void,
    pub cache: *const c_void,
    pub vtable: *const c_void,
    pub data: *const c_void,
}

#[repr(C)]
pub struct ClassRo {
    pub flags: u32,
    pub instance_start: u32,
    pub instance_size: u32,
    pub reserved: u32,
    pub ivar_layout: *const u8,
    pub name: *const c_char,
    pub methods: *const c_void,
    pub protocols: *const c_void,
    pub ivars: *const c_void,
    pub weak_ivar_layout: *const u8,
    pub properties: *const c_void,
}

#[repr(C)]
pub struct MethodEntry {
    pub name: *const c_char,
    pub types: *const c_char,
    pub imp: *const c_void,
}

#[repr(C)]
pub struct MethodList<const N: usize> {
    pub entsize_flags: u32,
    pub count: u32,
    pub methods: [MethodEntry; N],
}

#[repr(C)]
pub struct IvarEntry {
    pub offset: *mut i32,
    pub name: *const c_char,
    pub kind: *const c_char,
    pub alignment_raw: u32,
    pub size: u32,
}

#[repr(C)]
pub struct IvarList<const N: usize> {
    pub entsize: u32,
    pub count: u32,
    pub ivars: [IvarEntry; N],
}

#[repr(C)]
pub struct PropertyEntry {
    pub name: *const c_char,
    pub attributes: *const c_char,
}

#[repr(C)]
pub struct PropertyList<const N: usize> {
    pub entsize: u32,
    pub count: u32,
    pub properties: [PropertyEntry; N],
}

pub const RO_META: u32 = 0x1;
pub const RO_HAS_CXX_STRUCTORS: u32 = 0x4;
pub const RO_IS_ARC: u32 = 0x80;
pub const RO_HAS_CXX_DTOR_ONLY: u32 = 0x100;
/// `sizeof(objc_class)`, the metaclass instance size clang emits.
pub const META_SIZE: u32 = 40;

extern "C" {
    #[link_name = "_objc_empty_cache"]
    pub static EMPTY_CACHE: c_void;
    #[link_name = "OBJC_CLASS_$_NSObject"]
    pub static NSOBJECT_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_NSObject"]
    pub static NSOBJECT_META: c_void;
    #[link_name = "__CFConstantStringClassReference"]
    pub static CF_CONSTANT_STRING_CLASS: c_void;
}

/// Marks the image as containing Objective-C metadata (dyld hands only such
/// images to the runtime); flags 64: class properties, as clang emits.
#[macro_export]
macro_rules! image_info {
    () => {
        #[used]
        #[link_section = "__DATA,__objc_imageinfo,regular,no_dead_strip"]
        static OBJC_IMAGE_INFO: [u32; 2] = [0, 64];
    };
}

/// A class with no ivars, methods or properties (behaviour is added at
/// run time, or every message reaches the missing-call reporter), as an empty
/// `@implementation` emits it: `empty_class!(Name : super_class, super_meta)`.
/// The runtime sizes it after its superclass.
#[macro_export]
macro_rules! empty_class {
    ($name:ident : $sup_class:expr, $sup_meta:expr) => {
        #[allow(non_snake_case, non_upper_case_globals)]
        pub mod $name {
            use $crate::abi::*;
            const NAME: *const std::ffi::c_char = concat!(stringify!($name), "\0").as_ptr() as _;
            #[link_section = "__DATA,__objc_const"]
            static RO: Writable<ClassRo> = Writable::new(ClassRo {
                flags: RO_IS_ARC,
                instance_start: 8,
                instance_size: 8,
                reserved: 0,
                ivar_layout: std::ptr::null(),
                name: NAME,
                methods: std::ptr::null(),
                protocols: std::ptr::null(),
                ivars: std::ptr::null(),
                weak_ivar_layout: std::ptr::null(),
                properties: std::ptr::null(),
            });
            #[link_section = "__DATA,__objc_const"]
            static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
                flags: RO_IS_ARC | RO_META,
                instance_start: META_SIZE,
                instance_size: META_SIZE,
                reserved: 0,
                ivar_layout: std::ptr::null(),
                name: NAME,
                methods: std::ptr::null(),
                protocols: std::ptr::null(),
                ivars: std::ptr::null(),
                weak_ivar_layout: std::ptr::null(),
                properties: std::ptr::null(),
            });
            #[export_name = concat!("OBJC_METACLASS_$_", stringify!($name))]
            #[link_section = "__DATA,__objc_data"]
            pub static META: Writable<ClassT> = Writable::new(ClassT {
                isa: &raw const NSOBJECT_META,
                superclass: $sup_meta,
                cache: &raw const EMPTY_CACHE,
                vtable: std::ptr::null(),
                data: META_RO.ptr(),
            });
            #[export_name = concat!("OBJC_CLASS_$_", stringify!($name))]
            #[link_section = "__DATA,__objc_data"]
            pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
                isa: META.ptr(),
                superclass: $sup_class,
                cache: &raw const EMPTY_CACHE,
                vtable: std::ptr::null(),
                data: RO.ptr(),
            });
            #[used]
            #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
            static CLASS_LIST: Writable<*const std::ffi::c_void> = Writable::new(CLASS.ptr());
        }
    };
}

// ---------------------------------------------------------------------------
// Constant strings

/// A compile-time NSString (`@"…"`): CoreFoundation's constant-string layout.
#[repr(C)]
pub struct CFConstString {
    pub isa: *const c_void,
    pub flags: u64,
    pub bytes: *const u8,
    pub length: usize,
}
unsafe impl Sync for CFConstString {}

/// Flags clang emits for an ASCII constant string.
pub const CF_CONSTANT_ASCII: u64 = 0x7c8;

/// Exports `NAME` as an `NSString *const` holding the ASCII `value`, as
/// `NSString *const NAME = @"value";` does.
#[macro_export]
macro_rules! nsstring_constant {
    ($(#[$meta:meta])* $name:ident = $value:literal) => {
        $(#[$meta])*
        #[no_mangle]
        #[allow(non_upper_case_globals)]
        pub static $name: $crate::abi::Ptr = {
            #[link_section = "__DATA,__cfstring"]
            static STRING: $crate::abi::CFConstString = $crate::abi::CFConstString {
                isa: &raw const $crate::abi::CF_CONSTANT_STRING_CLASS,
                flags: $crate::abi::CF_CONSTANT_ASCII,
                bytes: concat!($value, "\0").as_ptr(),
                length: $value.len(),
            };
            $crate::abi::Ptr(&raw const STRING as *const std::ffi::c_void)
        };
    };
}

// ---------------------------------------------------------------------------
// Objective-C exceptions

#[cfg(all(target_vendor = "apple", target_arch = "aarch64"))]
extern "C-unwind" {
    fn pear_objc_try_call(ctx: *mut c_void, f: unsafe extern "C-unwind" fn(*mut c_void), exception: *mut Id) -> c_int;
    fn pear_objc_try_send(receiver: Id, sel: Sel, a2: usize, a3: usize, exception: *mut Id) -> c_int;
}

#[cfg(all(target_vendor = "apple", target_arch = "aarch64"))]
/// Runs `f(ctx)`. An Objective-C exception escaping it is caught and
/// returned retained (+1). Rust frames inside `f` must be `C-unwind` all the
/// way down for the exception to reach this frame.
pub unsafe fn try_call(ctx: *mut c_void, f: unsafe extern "C-unwind" fn(*mut c_void)) -> Result<(), Id> {
    let mut exception: Id = crate::NIL;
    if pear_objc_try_call(ctx, f, &mut exception) != 0 { Ok(()) } else { Err(exception) }
}

#[cfg(all(target_vendor = "apple", target_arch = "aarch64"))]
/// `objc_msgSend(receiver, sel, a2, a3)` with integer/pointer arguments and
/// the result ignored; an Objective-C exception it raises is caught and
/// returned retained (+1). No Rust frame sits between the try and the send.
pub unsafe fn try_send(receiver: Id, sel: Sel, a2: usize, a3: usize) -> Result<(), Id> {
    let mut exception: Id = crate::NIL;
    if pear_objc_try_send(receiver, sel, a2, a3, &mut exception) != 0 { Ok(()) } else { Err(exception) }
}

// clang's code for (with -fobjc-exceptions, no ARC):
//
//   int pear_objc_try_call(void *ctx, void (*fn)(void *), id *exception) {
//       @try { fn(ctx); return 1; }
//       @catch (id e) { if (exception) *exception = objc_retain(e); return 0; }
//   }
//
// and the same around objc_msgSend(x0, x1, x2, x3) with the out-parameter
// in x4. The exception table catches `id` (OBJC_EHTYPE_id) for the call, and
// lets objc_retain's own failure unwind after ending the catch.
#[cfg(all(target_vendor = "apple", target_arch = "aarch64"))]
macro_rules! try_frame {
    ($name:literal, $save_out:literal, $call:literal) => {
        concat!(
            ".section __TEXT,__text,regular,pure_instructions\n",
            ".private_extern _", $name, "\n",
            ".globl _", $name, "\n",
            ".p2align 2\n",
            "_", $name, ":\n",
            "L", $name, "_begin:\n",
            ".cfi_startproc\n",
            ".cfi_personality 155, ___objc_personality_v0\n",
            ".cfi_lsda 16, L", $name, "_lsda\n",
            "stp x20, x19, [sp, #-32]!\n",
            "stp x29, x30, [sp, #16]\n",
            "add x29, sp, #16\n",
            ".cfi_def_cfa w29, 16\n",
            ".cfi_offset w30, -8\n",
            ".cfi_offset w29, -16\n",
            ".cfi_offset w19, -24\n",
            ".cfi_offset w20, -32\n",
            $save_out, "\n",
            "L", $name, "_t0:\n",
            $call, "\n",
            "L", $name, "_t1:\n",
            "mov w0, #1\n",
            "L", $name, "_ret:\n",
            "ldp x29, x30, [sp, #16]\n",
            "ldp x20, x19, [sp], #32\n",
            "ret\n",
            "L", $name, "_t2:\n",
            "mov x20, x0\n",
            "cmp w1, #1\n",
            "b.ne L", $name, "_resume\n",
            "mov x0, x20\n",
            "bl _objc_begin_catch\n",
            "cbz x19, L", $name, "_end\n",
            "L", $name, "_t3:\n",
            "bl _objc_retain\n",
            "L", $name, "_t4:\n",
            "str x0, [x19]\n",
            "L", $name, "_end:\n",
            "bl _objc_end_catch\n",
            "mov w0, #0\n",
            "b L", $name, "_ret\n",
            "L", $name, "_t5:\n",
            "mov x20, x0\n",
            "bl _objc_end_catch\n",
            "L", $name, "_resume:\n",
            "mov x0, x20\n",
            "bl __Unwind_Resume\n",
            "L", $name, "_end_fn:\n",
            ".cfi_endproc\n",
            ".section __TEXT,__gcc_except_tab\n",
            ".p2align 2, 0x0\n",
            "L", $name, "_lsda:\n",
            ".byte 255\n",
            ".byte 155\n",
            ".uleb128 L", $name, "_ttbase-L", $name, "_ttref\n",
            "L", $name, "_ttref:\n",
            ".byte 1\n",
            ".uleb128 L", $name, "_cs_end-L", $name, "_cs_begin\n",
            "L", $name, "_cs_begin:\n",
            ".uleb128 L", $name, "_t0-L", $name, "_begin\n",
            ".uleb128 L", $name, "_t1-L", $name, "_t0\n",
            ".uleb128 L", $name, "_t2-L", $name, "_begin\n",
            ".byte 1\n",
            ".uleb128 L", $name, "_t1-L", $name, "_begin\n",
            ".uleb128 L", $name, "_t3-L", $name, "_t1\n",
            ".byte 0\n",
            ".byte 0\n",
            ".uleb128 L", $name, "_t3-L", $name, "_begin\n",
            ".uleb128 L", $name, "_t4-L", $name, "_t3\n",
            ".uleb128 L", $name, "_t5-L", $name, "_begin\n",
            ".byte 0\n",
            ".uleb128 L", $name, "_t4-L", $name, "_begin\n",
            ".uleb128 L", $name, "_end_fn-L", $name, "_t4\n",
            ".byte 0\n",
            ".byte 0\n",
            "L", $name, "_cs_end:\n",
            ".byte 1\n",
            ".byte 0\n",
            ".p2align 2, 0x0\n",
            "L", $name, "_tinfo:\n",
            ".long _OBJC_EHTYPE_id@GOT-L", $name, "_tinfo\n",
            "L", $name, "_ttbase:\n",
            ".p2align 2, 0x0\n",
            ".section __TEXT,__text,regular,pure_instructions\n",
        )
    };
}

#[cfg(all(target_vendor = "apple", target_arch = "aarch64"))]
std::arch::global_asm!(
    try_frame!("pear_objc_try_call", "mov x19, x2", "blr x1"),
    try_frame!("pear_objc_try_send", "mov x19, x4", "bl _objc_msgSend"),
);

#[cfg(all(test, target_vendor = "apple", target_arch = "aarch64"))]
mod tests {
    use super::*;
    use crate::{cls, msg0, msg3, nsstring, sel, NIL};

    #[link(name = "Foundation", kind = "framework")]
    extern "C" {}

    crate::nsstring_constant!(PearAbiTestConstant = "PearAbiTestConstant");

    unsafe fn exception(reason: &str) -> Id {
        msg3(cls("NSException"), "exceptionWithName:reason:userInfo:", nsstring("PearAbiTest"), nsstring(reason), NIL)
    }

    #[test]
    fn send_catches_objective_c_exceptions() {
        unsafe {
            let e = exception("send");
            assert_eq!(try_send(e, sel("raise"), 0, 0), Err(e));
            crate::objc_release(e);
            let s = nsstring("text");
            assert_eq!(try_send(s, sel("length"), 0, 0), Ok(()));
            // KVC on an object without the key raises NSUnknownKeyException.
            let object: Id = msg0(cls("NSObject"), "new");
            let caught = try_send(object, sel("setValue:forKey:"), s as usize, nsstring("noSuchKey") as usize);
            assert!(caught.is_err());
        }
    }

    unsafe extern "C-unwind" fn raise(ctx: *mut c_void) {
        let _: () = msg0(ctx as Id, "raise");
    }
    unsafe extern "C-unwind" fn nothing(_: *mut c_void) {}

    #[test]
    fn call_catches_objective_c_exceptions() {
        unsafe {
            let e = exception("call");
            assert_eq!(try_call(e as *mut c_void, raise), Err(e));
            assert_eq!(try_call(std::ptr::null_mut(), nothing), Ok(()));
        }
    }

    #[test]
    fn constant_strings_are_nsstrings() {
        unsafe {
            let s = PearAbiTestConstant.0 as Id;
            assert!(crate::is_kind_of(s, "NSString"));
            assert_eq!(crate::nsstring_to_string(s).as_deref(), Some("PearAbiTestConstant"));
        }
    }
}
