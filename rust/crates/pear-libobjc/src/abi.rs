//! Apple's 64-bit Objective-C metadata layouts, as clang emits them into
//! `__objc_*` sections (objc4's objc-runtime-new.h).

use std::ffi::{c_char, c_void};

pub type Id = *mut c_void;
pub type Sel = *const c_char;
pub type Imp = *const c_void;

#[repr(C)]
pub struct ClassT {
    pub isa: *const ClassT,
    pub superclass: *const ClassT,
    pub cache: *const c_void,
    pub vtable: *const c_void,
    /// `class_ro_t *` in the low bits mask; Swift flags in bits 0–1.
    pub bits: usize,
}

/// The `class_ro_t *` in `bits`. objc4 masks with 0x0000_7fff_ffff_fff8
/// (Darwin user addresses are below 2^47); Linux arm64 user space spans 48
/// bits, and images only use the low bits for flags, so 48 bits are kept.
pub const FAST_DATA_MASK: usize = 0x0000_ffff_ffff_fff8;
pub const FAST_IS_SWIFT_LEGACY: usize = 1 << 0;
pub const FAST_IS_SWIFT_STABLE: usize = 1 << 1;

#[inline]
pub fn is_tagged_pointer(obj: Id) -> bool {
    let addr = obj as usize;
    addr != 0 && (addr & ((1usize << 63) | 1)) != 0
}

#[repr(C)]
pub struct ClassRo {
    pub flags: u32,
    pub instance_start: u32,
    pub instance_size: u32,
    pub reserved: u32,
    pub ivar_layout: *const u8,
    pub name: *const c_char,
    pub base_methods: *const MethodList,
    pub base_protocols: *const ProtocolList,
    pub ivars: *const IvarList,
    pub weak_ivar_layout: *const u8,
    pub base_properties: *const c_void,
}

pub const RO_META: u32 = 1 << 0;

#[repr(C)]
pub struct MethodList {
    pub entsize_flags: u32,
    pub count: u32,
    // entries follow
}

/// Entries are three 32-bit offsets, each relative to its own field: the
/// selector reference (a `__objc_selrefs` slot), the type string, the IMP.
pub const METHOD_LIST_RELATIVE: u32 = 0x8000_0000;
/// Relative name offsets point at the selector string itself (shared cache).
pub const METHOD_LIST_UNIQUED_SELECTORS: u32 = 0x4000_0000;
pub const METHOD_LIST_ENTSIZE_MASK: u32 = 0x0000_fffc;

#[repr(C)]
pub struct Method {
    pub name: Sel,
    pub types: *const c_char,
    pub imp: Imp,
}

#[repr(C)]
pub struct IvarList {
    pub entsize: u32,
    pub count: u32,
    // Ivar entries follow
}

#[repr(C)]
pub struct Ivar {
    /// The `OBJC_IVAR_$_Class.name` variable compiled code reads.
    pub offset: *mut i32,
    pub name: *const c_char,
    pub kind: *const c_char,
    pub alignment_raw: u32,
    pub size: u32,
}

#[repr(C)]
pub struct ProtocolList {
    pub count: usize,
    // `*const ProtocolT` entries follow
}

#[repr(C)]
pub struct ProtocolT {
    pub isa: *const c_void,
    pub mangled_name: *const c_char,
    pub protocols: *const ProtocolList,
    pub instance_methods: *const MethodList,
    pub class_methods: *const MethodList,
    pub optional_instance_methods: *const MethodList,
    pub optional_class_methods: *const MethodList,
    pub instance_properties: *const c_void,
    pub size: u32,
    pub flags: u32,
}

#[repr(C)]
pub struct CategoryT {
    pub name: *const c_char,
    pub cls: *const ClassT,
    pub instance_methods: *const MethodList,
    pub class_methods: *const MethodList,
    pub protocols: *const ProtocolList,
    pub instance_properties: *const c_void,
}

/// `struct objc_super` for objc_msgSendSuper2: the receiver and the class
/// whose superclass the lookup starts at.
#[repr(C)]
pub struct ObjcSuper {
    pub receiver: Id,
    pub current_class: *const ClassT,
}

/// One method of a method list.
#[derive(Clone, Copy)]
pub struct MethodDesc {
    pub name: Sel,
    pub types: *const c_char,
    pub imp: Imp,
}

/// Reads a method list, with uniqued selectors. Relative lists name their
/// selector through a selref slot, which must already hold the uniqued
/// selector; pointer lists hold the name string itself.
pub unsafe fn methods(list: *const MethodList) -> Vec<MethodDesc> {
    if list.is_null() {
        return Vec::new();
    }
    let flags = (*list).entsize_flags;
    let entsize = (flags & METHOD_LIST_ENTSIZE_MASK) as usize;
    let first = (list as *const u8).add(8);
    let mut out = Vec::with_capacity((*list).count as usize);
    for i in 0..(*list).count as usize {
        let entry = first.add(i * entsize);
        if flags & METHOD_LIST_RELATIVE != 0 {
            let rel = |field: usize| -> *const u8 {
                let at = entry.add(field * 4);
                at.offset(*(at as *const i32) as isize)
            };
            let name = if flags & METHOD_LIST_UNIQUED_SELECTORS != 0 {
                rel(0) as Sel
            } else {
                *(rel(0) as *const Sel)
            };
            out.push(MethodDesc { name, types: rel(1) as *const c_char, imp: rel(2) as Imp });
        } else {
            // Pointer lists (older toolchains) name the method with the
            // image's own string: unique it, so lookups by SEL match.
            let m = &*(entry as *const Method);
            out.push(MethodDesc { name: crate::runtime::intern_c(m.name), types: m.types, imp: m.imp });
        }
    }
    out
}

#[repr(C)]
pub struct PropertyList {
    pub entsize: u32,
    pub count: u32,
}

/// (name, attributes) of each property in a property list.
pub unsafe fn properties(list: *const c_void) -> Vec<(*const c_char, *const c_char)> {
    let list = list as *const PropertyList;
    if list.is_null() {
        return Vec::new();
    }
    let entsize = (*list).entsize as usize;
    let first = (list as *const u8).add(8);
    (0..(*list).count as usize)
        .map(|i| {
            let p = first.add(i * entsize) as *const *const c_char;
            (*p, *p.add(1))
        })
        .collect()
}

pub unsafe fn protocols(list: *const ProtocolList) -> Vec<*const ProtocolT> {
    if list.is_null() {
        return Vec::new();
    }
    let first = (list as *const u8).add(8) as *const *const ProtocolT;
    (0..(*list).count).map(|i| *first.add(i)).collect()
}

pub unsafe fn ivars(list: *const IvarList) -> Vec<*const Ivar> {
    if list.is_null() {
        return Vec::new();
    }
    let entsize = (*list).entsize as usize;
    let first = (list as *const u8).add(8);
    (0..(*list).count as usize).map(|i| first.add(i * entsize) as *const Ivar).collect()
}
