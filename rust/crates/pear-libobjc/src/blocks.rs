//! Clean-room: ABI layouts and semantics from Apple's open-source libclosure
//! (APSL 2.0, reference only, nothing copied) and the public Block_private.h /
//! objc/runtime.h documentation.
//!
//! Objective-C blocks (`_Block_copy`, `_Block_release`, `_Block_object_assign`,
//! `_Block_object_dispose`, `__block` byref forwarding, `NSBlock` class
//! hierarchy, and `imp_implementationWithBlock` RX trampolines).

use crate::abi::*;
use crate::object::{EMPTY_CACHE, NSOBJECT_CLASS, NSOBJECT_META, StaticClass};
use crate::runtime;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, Once, OnceLock};

const BLOCK_NEEDS_FREE: i32 = 1 << 24;
const BLOCK_HAS_COPY_DISPOSE: i32 = 1 << 25;
const BLOCK_IS_GLOBAL: i32 = 1 << 28;
const BLOCK_REFCOUNT_MASK: i32 = 0xFFFE;
const BLOCK_REFCOUNT_STEP: i32 = 2;

const BLOCK_BYREF_NEEDS_FREE: i32 = 1 << 24;
const BLOCK_BYREF_HAS_COPY_DISPOSE: i32 = 1 << 25;
const BLOCK_BYREF_LAYOUT_MASK: i32 = 0xF << 28;

const BLOCK_FIELD_IS_OBJECT: i32 = 3;
const BLOCK_FIELD_IS_BLOCK: i32 = 7;
const BLOCK_FIELD_IS_BYREF: i32 = 8;
const BLOCK_FIELD_IS_WEAK: i32 = 16;
const BLOCK_BYREF_CALLER: i32 = 128;

#[repr(C)]
pub struct BlockLayout {
    pub isa: *const ClassT,
    pub flags: AtomicI32,
    pub reserved: i32,
    pub invoke: *const c_void,
    pub descriptor: *const BlockDescriptor1,
}

#[repr(C)]
pub struct BlockDescriptor1 {
    pub reserved: usize,
    pub size: usize,
}

#[repr(C)]
pub struct BlockDescriptor2 {
    pub copy: extern "C" fn(*mut c_void, *const c_void),
    pub dispose: extern "C" fn(*const c_void),
}

#[repr(C)]
pub struct BlockByref {
    pub isa: *mut c_void,
    pub forwarding: *mut BlockByref,
    pub flags: AtomicI32,
    pub size: u32,
}

#[repr(C)]
pub struct BlockByref2 {
    pub byref_keep: extern "C" fn(*mut c_void, *mut c_void),
    pub byref_destroy: extern "C" fn(*mut c_void),
}

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
    fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
    fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> i32;
    fn mprotect(addr: *mut c_void, len: usize, prot: i32) -> i32;
}

const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const PROT_EXEC: i32 = 4;
const MAP_PRIVATE: i32 = 0x0002;
#[cfg(target_os = "macos")]
const MAP_ANON: i32 = 0x1000;
#[cfg(not(target_os = "macos"))]
const MAP_ANON: i32 = 0x0020;

// ---- NSBlock class hierarchy -----------------------------------------------

pub static NSBLOCK_META: StaticClass = StaticClass(UnsafeCell::new(ClassT {
    isa: &raw const NSOBJECT_META as *const ClassT,
    superclass: &raw const NSOBJECT_META as *const ClassT,
    cache: &raw const EMPTY_CACHE as *const c_void,
    vtable: std::ptr::null(),
    bits: 0,
}));

pub static NSBLOCK_CLASS: StaticClass = StaticClass(UnsafeCell::new(ClassT {
    isa: &raw const NSBLOCK_META as *const ClassT,
    superclass: &raw const NSOBJECT_CLASS as *const ClassT,
    cache: &raw const EMPTY_CACHE as *const c_void,
    vtable: std::ptr::null(),
    bits: 0,
}));

macro_rules! block_subclass {
    ($cls:ident, $meta:ident) => {
        pub static $meta: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const NSOBJECT_META as *const ClassT,
            superclass: &raw const NSBLOCK_META as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
        pub static $cls: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const $meta as *const ClassT,
            superclass: &raw const NSBLOCK_CLASS as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
    };
}

block_subclass!(NSSTACKBLOCK_CLASS, NSSTACKBLOCK_META);
block_subclass!(NSMALLOCBLOCK_CLASS, NSMALLOCBLOCK_META);
block_subclass!(NSGLOBALBLOCK_CLASS, NSGLOBALBLOCK_META);

extern "C" fn blk_self(this: Id, _: Sel) -> Id {
    this
}
extern "C" fn blk_void(_: Id, _: Sel) {}
extern "C" fn blk_copy_heap(this: Id, _: Sel) -> Id {
    unsafe { block_copy(this as *const c_void) as Id }
}
extern "C" fn blk_copy_zone(this: Id, _: Sel, _: *mut c_void) -> Id {
    unsafe { block_copy(this as *const c_void) as Id }
}
extern "C" fn blk_release_heap(this: Id, _: Sel) {
    unsafe { block_release(this as *const c_void) }
}
extern "C" fn blk_global_copy(this: Id, _: Sel) -> Id {
    this
}
extern "C" fn blk_global_copy_zone(this: Id, _: Sel, _: *mut c_void) -> Id {
    this
}

pub fn init_blocks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        crate::object::init_runtime();
        let i = |f: *const ()| f as Imp;
        runtime::register_builtin(
            NSBLOCK_CLASS.ptr(),
            NSBLOCK_META.ptr(),
            "NSBlock",
            32,
            &[
                ("copy", i(blk_copy_heap as *const ())),
                ("copyWithZone:", i(blk_copy_zone as *const ())),
            ],
            &[],
        );
        runtime::register_builtin(
            NSSTACKBLOCK_CLASS.ptr(),
            NSSTACKBLOCK_META.ptr(),
            "__NSStackBlock__",
            32,
            &[
                ("retain", i(blk_self as *const ())),
                ("release", i(blk_void as *const ())),
                ("autorelease", i(blk_self as *const ())),
                ("copy", i(blk_copy_heap as *const ())),
                ("copyWithZone:", i(blk_copy_zone as *const ())),
            ],
            &[],
        );
        runtime::register_builtin(
            NSMALLOCBLOCK_CLASS.ptr(),
            NSMALLOCBLOCK_META.ptr(),
            "__NSMallocBlock__",
            32,
            &[
                ("retain", i(blk_copy_heap as *const ())),
                ("release", i(blk_release_heap as *const ())),
                ("copy", i(blk_copy_heap as *const ())),
                ("copyWithZone:", i(blk_copy_zone as *const ())),
            ],
            &[],
        );
        runtime::register_builtin(
            NSGLOBALBLOCK_CLASS.ptr(),
            NSGLOBALBLOCK_META.ptr(),
            "__NSGlobalBlock__",
            32,
            &[
                ("retain", i(blk_self as *const ())),
                ("release", i(blk_void as *const ())),
                ("autorelease", i(blk_self as *const ())),
                ("copy", i(blk_global_copy as *const ())),
                ("copyWithZone:", i(blk_global_copy_zone as *const ())),
            ],
            &[],
        );
    });
}

// ---- _Block_copy / _Block_release ------------------------------------------

unsafe fn block_descriptor2(blk: *const BlockLayout) -> Option<&'static BlockDescriptor2> {
    if ((*blk).flags.load(Ordering::Relaxed) & BLOCK_HAS_COPY_DISPOSE) == 0 {
        return None;
    }
    let desc1 = (*blk).descriptor;
    if desc1.is_null() {
        return None;
    }
    Some(&*((desc1 as *const u8).add(std::mem::size_of::<BlockDescriptor1>()) as *const BlockDescriptor2))
}

pub unsafe extern "C" fn block_copy(arg: *const c_void) -> *mut c_void {
    init_blocks();
    if arg.is_null() {
        return std::ptr::null_mut();
    }
    let blk = arg as *mut BlockLayout;
    let flags = (*blk).flags.load(Ordering::Acquire);
    if (flags & BLOCK_IS_GLOBAL) != 0 {
        return blk as *mut c_void;
    }
    if (flags & BLOCK_NEEDS_FREE) != 0 {
        // Already a heap block: increment refcount.
        let mut cur = flags;
        loop {
            let next = cur.wrapping_add(BLOCK_REFCOUNT_STEP);
            match (*blk).flags.compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return blk as *mut c_void,
                Err(v) => cur = v,
            }
        }
    }
    // Stack block: copy to malloc heap.
    let size = (*(*blk).descriptor).size;
    let copy = malloc(size) as *mut BlockLayout;
    if copy.is_null() {
        return std::ptr::null_mut();
    }
    memcpy(copy as *mut c_void, blk as *const c_void, size);
    let new_flags = (flags & !BLOCK_REFCOUNT_MASK) | BLOCK_NEEDS_FREE | BLOCK_REFCOUNT_STEP;
    (*copy).flags.store(new_flags, Ordering::Release);
    (*copy).isa = NSMALLOCBLOCK_CLASS.ptr();
    if let Some(d2) = block_descriptor2(blk) {
        (d2.copy)(copy as *mut c_void, blk as *const c_void);
    }
    copy as *mut c_void
}

pub unsafe extern "C" fn block_release(arg: *const c_void) {
    if arg.is_null() {
        return;
    }
    let blk = arg as *mut BlockLayout;
    let mut cur = (*blk).flags.load(Ordering::Acquire);
    if (cur & BLOCK_IS_GLOBAL) != 0 || (cur & BLOCK_NEEDS_FREE) == 0 {
        return;
    }
    loop {
        let rc = cur & BLOCK_REFCOUNT_MASK;
        if rc <= BLOCK_REFCOUNT_STEP {
            if let Some(d2) = block_descriptor2(blk) {
                (d2.dispose)(blk as *const c_void);
            }
            free(blk as *mut c_void);
            return;
        }
        let next = cur - BLOCK_REFCOUNT_STEP;
        match (*blk).flags.compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(v) => cur = v,
        }
    }
}

pub unsafe extern "C" fn objc_retain_block(blk: Id) -> Id {
    block_copy(blk as *const c_void) as Id
}

// ---- byref & _Block_object_assign / _Block_object_dispose ------------------

unsafe fn byref_copy(arg: *const c_void) -> *mut c_void {
    let src = arg as *mut BlockByref;
    if ((*src).forwarding as *const BlockByref) != src {
        let fwd = (*src).forwarding;
        if ((*fwd).flags.load(Ordering::Acquire) & BLOCK_BYREF_NEEDS_FREE) != 0 {
            (*fwd).flags.fetch_add(BLOCK_REFCOUNT_STEP, Ordering::AcqRel);
        }
        return fwd as *mut c_void;
    }
    let size = (*src).size as usize;
    let copy = malloc(size) as *mut BlockByref;
    if copy.is_null() {
        return std::ptr::null_mut();
    }
    memcpy(copy as *mut c_void, src as *const c_void, size);
    let flags = (*src).flags.load(Ordering::Acquire);
    (*copy).flags.store((flags & !BLOCK_REFCOUNT_MASK) | BLOCK_BYREF_NEEDS_FREE | (2 * BLOCK_REFCOUNT_STEP), Ordering::Release);
    (*copy).forwarding = copy;
    (*src).forwarding = copy;
    if (flags & BLOCK_BYREF_HAS_COPY_DISPOSE) != 0 {
        let b2 = &*((copy as *const u8).add(std::mem::size_of::<BlockByref>()) as *const BlockByref2);
        (b2.byref_keep)(copy as *mut c_void, src as *mut c_void);
    }
    copy as *mut c_void
}

unsafe fn byref_release(arg: *const c_void) {
    if arg.is_null() {
        return;
    }
    let mut byref = arg as *mut BlockByref;
    byref = (*byref).forwarding;
    let mut cur = (*byref).flags.load(Ordering::Acquire);
    if (cur & BLOCK_BYREF_NEEDS_FREE) == 0 {
        return;
    }
    loop {
        let rc = cur & BLOCK_REFCOUNT_MASK;
        if rc <= BLOCK_REFCOUNT_STEP {
            if (cur & BLOCK_BYREF_HAS_COPY_DISPOSE) != 0 {
                let b2 = &*((byref as *const u8).add(std::mem::size_of::<BlockByref>()) as *const BlockByref2);
                (b2.byref_destroy)(byref as *mut c_void);
            }
            free(byref as *mut c_void);
            return;
        }
        let next = cur - BLOCK_REFCOUNT_STEP;
        match (*byref).flags.compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(v) => cur = v,
        }
    }
}

pub unsafe extern "C" fn block_object_assign(dst: *mut c_void, src: *const c_void, flags: i32) {
    let _ = BLOCK_BYREF_LAYOUT_MASK;
    let caller = (flags & BLOCK_BYREF_CALLER) != 0;
    let kind = flags & 0x1F;
    if kind == BLOCK_FIELD_IS_BYREF || kind == (BLOCK_FIELD_IS_BYREF | BLOCK_FIELD_IS_WEAK) {
        if !caller && !src.is_null() {
            *(dst as *mut *mut c_void) = byref_copy(src);
        } else {
            *(dst as *mut *const c_void) = src;
        }
    } else if kind == BLOCK_FIELD_IS_BLOCK || kind == (BLOCK_FIELD_IS_BLOCK | BLOCK_FIELD_IS_WEAK) {
        if caller {
            *(dst as *mut *const c_void) = src;
        } else {
            *(dst as *mut *mut c_void) = block_copy(src);
        }
    } else if kind == BLOCK_FIELD_IS_OBJECT {
        if caller {
            *(dst as *mut *const c_void) = src;
        } else {
            *(dst as *mut Id) = crate::object::retain(src as Id);
        }
    } else if kind == (BLOCK_FIELD_IS_OBJECT | BLOCK_FIELD_IS_WEAK) {
        *(dst as *mut Id) = std::ptr::null_mut();
        crate::object::store_weak(dst as *mut Id, src as Id);
    }
}

pub unsafe extern "C" fn block_object_dispose(obj: *const c_void, flags: i32) {
    if obj.is_null() || (flags & BLOCK_BYREF_CALLER) != 0 {
        return;
    }
    let kind = flags & 0x1F;
    if kind == BLOCK_FIELD_IS_BYREF || kind == (BLOCK_FIELD_IS_BYREF | BLOCK_FIELD_IS_WEAK) {
        byref_release(obj);
    } else if kind == BLOCK_FIELD_IS_BLOCK || kind == (BLOCK_FIELD_IS_BLOCK | BLOCK_FIELD_IS_WEAK) {
        block_release(obj);
    } else if kind == BLOCK_FIELD_IS_OBJECT {
        crate::object::release(obj as Id);
    } else if kind == (BLOCK_FIELD_IS_OBJECT | BLOCK_FIELD_IS_WEAK) {
        crate::object::store_weak(obj as *mut Id, std::ptr::null_mut());
    }
}

// ---- imp_implementationWithBlock / imp_getBlock / imp_removeBlock ----------

fn block_imps() -> &'static Mutex<HashMap<usize, usize>> {
    static M: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

pub unsafe extern "C" fn imp_implementation_with_block(block: Id) -> Imp {
    if block.is_null() {
        return std::ptr::null();
    }
    let copied = block_copy(block as *const c_void);
    if copied.is_null() {
        return std::ptr::null();
    }
    let page = mmap(std::ptr::null_mut(), 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
    if page == usize::MAX as *mut c_void {
        block_release(copied);
        return std::ptr::null();
    }
    // mov x1, x0; ldr x0, +12; ldr x16, [x0, #16]; br x16; .quad block
    let code = page as *mut u32;
    *code.add(0) = 0xAA00_03E1;
    *code.add(1) = 0x5800_0060;
    *code.add(2) = 0xF940_0810;
    *code.add(3) = 0xD61F_0200;
    *(code.add(4) as *mut usize) = copied as usize;

    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn sys_icache_invalidate(start: *const c_void, len: usize);
        }
        sys_icache_invalidate(page, 24);
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::arch::asm!(
            "dc cvau, {p}",
            "dsb ish",
            "ic ivau, {p}",
            "dsb ish",
            "isb",
            p = in(reg) page,
        );
    }
    mprotect(page, 4096, PROT_READ | PROT_EXEC);
    block_imps().lock().unwrap().insert(page as usize, copied as usize);
    page as Imp
}

pub unsafe extern "C" fn imp_get_block(imp: Imp) -> Id {
    if imp.is_null() {
        return std::ptr::null_mut();
    }
    block_imps().lock().unwrap().get(&(imp as usize)).copied().unwrap_or(0) as Id
}

pub unsafe extern "C" fn imp_remove_block(imp: Imp) -> bool {
    if imp.is_null() {
        return false;
    }
    let Some(blk) = block_imps().lock().unwrap().remove(&(imp as usize)) else {
        return false;
    };
    block_release(blk as *const c_void);
    munmap(imp as *mut c_void, 4096);
    true
}
