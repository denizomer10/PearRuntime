//! Objective-C classes emitted as static data, the way clang emits them, so
//! their `OBJC_CLASS_$_…` symbols exist at load time: apps bind to them (and
//! subclass them) before any initializer runs. The runtime realizes them from
//! `__objc_classlist` and uniques the selector strings in the method lists.

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void};

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
pub struct IvarList {
    pub entsize: u32,
    pub count: u32,
    pub ivars: [IvarEntry; 1],
}

pub const RO_META: u32 = 1;
/// `sizeof(objc_class)`, the metaclass instance size clang emits.
pub const META_SIZE: u32 = 40;

extern "C" {
    #[link_name = "_objc_empty_cache"]
    pub static EMPTY_CACHE: c_void;
    #[link_name = "OBJC_CLASS_$_NSObject"]
    pub static NSOBJECT_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_NSObject"]
    pub static NSOBJECT_META: c_void;
    #[link_name = "OBJC_CLASS_$_UIView"]
    pub static UIVIEW_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_UIView"]
    pub static UIVIEW_META: c_void;
    #[link_name = "OBJC_CLASS_$_UIViewController"]
    pub static UIVIEWCONTROLLER_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_UIViewController"]
    pub static UIVIEWCONTROLLER_META: c_void;
}

/// Marks this image as containing Objective-C metadata (dyld hands only such
/// images to the runtime). Flags 64: class properties supported, as clang.
#[used]
#[link_section = "__DATA,__objc_imageinfo,regular,no_dead_strip"]
static IMAGE_INFO: [u32; 2] = [0, 64];

/// The per-instance Rust state pointer of a static class (one pointer ivar,
/// `pear_state`, whose offset the runtime slides past the superclass).
pub unsafe fn state_slot(obj: *mut c_void, offset: *const i32) -> *mut *mut c_void {
    (obj as *mut u8).offset(std::ptr::read_volatile(offset) as isize) as *mut *mut c_void
}

/// Defines a static Objective-C class in a module:
/// `objc_class! { mod name = "ClassName" : (SUPER_CLASS, SUPER_META), state, methods {..}, class_methods {..} }`.
/// With `state`, instances carry one pointer ivar (`pear_state`) read by
/// `state_slot(obj, &raw const name::STATE_OFFSET)`.
#[macro_export]
macro_rules! objc_class {
    (
        mod $m:ident = $name:literal : ($sup:ident, $supmeta:ident),
        state: $state:tt,
        methods { $($sel:literal $ty:literal => $imp:expr),* $(,)? }
        class_methods { $($csel:literal $cty:literal => $cimp:expr),* $(,)? }
    ) => {
        #[allow(non_upper_case_globals, dead_code)]
        pub mod $m {
            use $crate::graphics_gl::objc_class::*;
            use std::ffi::c_void;
            #[allow(unused_imports)]
            use super::*;

            const NAME: &std::ffi::CStr = match std::ffi::CStr::from_bytes_with_nul(concat!($name, "\0").as_bytes()) {
                Ok(c) => c,
                Err(_) => panic!(),
            };
            const N: usize = <[&str]>::len(&[$($sel),*]);
            const CN: usize = <[&str]>::len(&[$($csel),*]);

            #[link_section = "__DATA,__objc_const"]
            static METHODS: Writable<MethodList<N>> = Writable::new(MethodList {
                entsize_flags: 24,
                count: N as u32,
                methods: [$(MethodEntry { name: concat!($sel, "\0").as_ptr() as _, types: concat!($ty, "\0").as_ptr() as _, imp: $imp as *const c_void }),*],
            });
            #[link_section = "__DATA,__objc_const"]
            static CLASS_METHODS: Writable<MethodList<CN>> = Writable::new(MethodList {
                entsize_flags: 24,
                count: CN as u32,
                methods: [$(MethodEntry { name: concat!($csel, "\0").as_ptr() as _, types: concat!($cty, "\0").as_ptr() as _, imp: $cimp as *const c_void }),*],
            });

            /// Offset of the `pear_state` ivar (slid by the runtime).
            #[link_section = "__DATA,__objc_ivar"]
            pub static mut STATE_OFFSET: i32 = 8;
            const HAS_STATE: bool = $crate::objc_class!(@bool $state);
            #[link_section = "__DATA,__objc_const"]
            static IVARS: Writable<IvarList> = Writable::new(IvarList {
                entsize: 32,
                count: 1,
                ivars: [IvarEntry { offset: &raw mut STATE_OFFSET, name: c"pear_state".as_ptr(), kind: c"^v".as_ptr(), alignment_raw: 3, size: 8 }],
            });

            #[link_section = "__DATA,__objc_const"]
            static RO: Writable<ClassRo> = Writable::new(ClassRo {
                flags: 0,
                instance_start: 8,
                instance_size: if HAS_STATE { 16 } else { 8 },
                reserved: 0,
                ivar_layout: std::ptr::null(),
                name: NAME.as_ptr(),
                methods: if N > 0 { METHODS.ptr() } else { std::ptr::null() },
                protocols: std::ptr::null(),
                ivars: if HAS_STATE { IVARS.ptr() } else { std::ptr::null() },
                weak_ivar_layout: std::ptr::null(),
                properties: std::ptr::null(),
            });
            #[link_section = "__DATA,__objc_const"]
            static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
                flags: RO_META,
                instance_start: META_SIZE,
                instance_size: META_SIZE,
                reserved: 0,
                ivar_layout: std::ptr::null(),
                name: NAME.as_ptr(),
                methods: if CN > 0 { CLASS_METHODS.ptr() } else { std::ptr::null() },
                protocols: std::ptr::null(),
                ivars: std::ptr::null(),
                weak_ivar_layout: std::ptr::null(),
                properties: std::ptr::null(),
            });

            #[export_name = concat!("OBJC_METACLASS_$_", $name)]
            #[link_section = "__DATA,__objc_data"]
            pub static META: Writable<ClassT> = Writable::new(ClassT {
                isa: &raw const NSOBJECT_META,
                superclass: &raw const $supmeta,
                cache: &raw const EMPTY_CACHE,
                vtable: std::ptr::null(),
                data: META_RO.ptr(),
            });
            #[export_name = concat!("OBJC_CLASS_$_", $name)]
            #[link_section = "__DATA,__objc_data"]
            pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
                isa: META.ptr(),
                superclass: &raw const $sup,
                cache: &raw const EMPTY_CACHE,
                vtable: std::ptr::null(),
                data: RO.ptr(),
            });

            #[used]
            #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
            static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());

            /// The class object (realized on first message).
            pub fn class() -> pear_objc::Class {
                CLASS.ptr() as pear_objc::Class
            }

            /// The instance's `pear_state` slot.
            pub unsafe fn slot(obj: pear_objc::Id) -> *mut *mut c_void {
                state_slot(obj, &raw const STATE_OFFSET)
            }
        }
    };
    (@bool true) => { true };
    (@bool false) => { false };
}
