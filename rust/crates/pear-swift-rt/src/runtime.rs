//! Clean-room Swift 5 ABI object model, ARC / weak / unowned references,
//! `_TtCs12_SwiftObject` ObjC interop root class, boxes, errors, and `_swift_once`.

use pear_libobjc::abi::{ClassT, Id, Imp, Sel};
use pear_libobjc::object::{self, EMPTY_CACHE, StaticClass};
use pear_libobjc::runtime as objc_rt;
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{c_char, c_double, c_float, c_void, CStr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, Once};

extern "C" {
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn strtod(nptr: *const c_char, endptr: *mut *mut c_char) -> c_double;
    fn strtof(nptr: *const c_char, endptr: *mut *mut c_char) -> c_float;
}

/// Swift 5 `HeapObject` header (`isa` + `refcounts`).
#[repr(C)]
pub struct HeapObject {
    pub isa: *const ClassT,
    pub refcounts: AtomicU64,
}

/// Immortal sentinel for static/stack/empty-singleton Swift objects.
const IMMORTAL_REFCOUNT: u64 = u64::MAX;
const STRONG_MASK: u64 = 0x7FFF_FFFF;
const UNOWNED_SHIFT: u32 = 32;
const UNOWNED_MASK: u64 = 0x7FFF_FFFF << 32;
const DEALLOCATING_BIT: u64 = 1u64 << 63;

/// Empty Swift collection singleton storage (`HeapObject` + 3 pointer words).
#[repr(C)]
pub struct EmptySingletonStorage {
    pub isa: *const ClassT,
    pub refcounts: u64,
    pub count: usize,
    pub capacity_and_flags: usize,
    pub reserved: usize,
}
unsafe impl Sync for EmptySingletonStorage {}

/// `_TtCs12_SwiftObject` root class and metaclass (matching NSObject layout with
/// `superclass = null` on root class and `superclass = class` on metaclass).
pub static SWIFT_OBJECT_META: StaticClass = StaticClass(UnsafeCell::new(ClassT {
    isa: &raw const SWIFT_OBJECT_META as *const ClassT,
    superclass: &raw const SWIFT_OBJECT_CLASS as *const ClassT,
    cache: &raw const EMPTY_CACHE as *const c_void,
    vtable: std::ptr::null(),
    bits: 0,
}));

pub static SWIFT_OBJECT_CLASS: StaticClass = StaticClass(UnsafeCell::new(ClassT {
    isa: &raw const SWIFT_OBJECT_META as *const ClassT,
    superclass: std::ptr::null(),
    cache: &raw const EMPTY_CACHE as *const c_void,
    vtable: std::ptr::null(),
    bits: 0,
}));

pub static SWIFT_ISA_MASK: usize = !0usize;

pub static EMPTY_ARRAY_STORAGE: EmptySingletonStorage = EmptySingletonStorage {
    isa: &raw const SWIFT_OBJECT_CLASS as *const ClassT,
    refcounts: IMMORTAL_REFCOUNT,
    count: 0,
    capacity_and_flags: 0,
    reserved: 0,
};

pub static EMPTY_DICTIONARY_SINGLETON: EmptySingletonStorage = EmptySingletonStorage {
    isa: &raw const SWIFT_OBJECT_CLASS as *const ClassT,
    refcounts: IMMORTAL_REFCOUNT,
    count: 0,
    capacity_and_flags: 0,
    reserved: 0,
};

pub static EMPTY_SET_SINGLETON: EmptySingletonStorage = EmptySingletonStorage {
    isa: &raw const SWIFT_OBJECT_CLASS as *const ClassT,
    refcounts: IMMORTAL_REFCOUNT,
    count: 0,
    capacity_and_flags: 0,
    reserved: 0,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SwiftOsVersion {
    pub major: isize,
    pub minor: isize,
    pub patch: isize,
}

static SWIFT_OBJECTS: LazyLock<Mutex<HashSet<usize>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
static WEAK_SLOTS: LazyLock<Mutex<HashMap<usize, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static BOX_ERRORS: LazyLock<Mutex<HashMap<usize, (usize, usize)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn swift_object_class() -> *const ClassT {
    SWIFT_OBJECT_CLASS.ptr()
}

pub fn swift_object_metaclass() -> *const ClassT {
    SWIFT_OBJECT_META.ptr()
}

pub fn register_swift_object_class() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        object::init_runtime();
        let i = |f: *const ()| f as Imp;
        objc_rt::register_builtin(
            SWIFT_OBJECT_CLASS.ptr(),
            SWIFT_OBJECT_META.ptr(),
            "_TtCs12_SwiftObject",
            16,
            &[
                ("init", i(so_self as *const ())),
                ("self", i(so_self as *const ())),
                ("class", i(so_class as *const ())),
                ("superclass", i(so_superclass as *const ())),
                ("retain", i(so_retain as *const ())),
                ("release", i(so_release as *const ())),
                ("autorelease", i(so_self as *const ())),
                ("retainCount", i(so_retain_count as *const ())),
                ("dealloc", i(so_dealloc as *const ())),
                ("isKindOfClass:", i(so_is_kind as *const ())),
                ("isMemberOfClass:", i(so_is_member as *const ())),
                ("respondsToSelector:", i(so_responds as *const ())),
                ("conformsToProtocol:", i(so_conforms as *const ())),
                ("isEqual:", i(so_is_equal as *const ())),
                ("hash", i(so_hash as *const ())),
                ("isProxy", i(so_no as *const ())),
                ("description", i(so_description as *const ())),
            ],
            &[
                ("alloc", i(so_c_alloc as *const ())),
                ("allocWithZone:", i(so_c_alloc_zone as *const ())),
                ("new", i(so_c_new as *const ())),
                ("class", i(so_self as *const ())),
                ("superclass", i(so_c_superclass as *const ())),
                ("initialize", i(so_void as *const ())),
                ("load", i(so_void as *const ())),
                ("isSubclassOfClass:", i(so_c_is_subclass as *const ())),
                ("instancesRespondToSelector:", i(so_c_instances_respond as *const ())),
            ],
        );
    });
}

// ---- _TtCs12_SwiftObject ObjC methods --------------------------------------

extern "C" fn so_self(this: Id, _: Sel) -> Id {
    this
}
extern "C" fn so_void(_: Id, _: Sel) {}
extern "C" fn so_no(_: Id, _: Sel) -> bool {
    false
}
extern "C" fn so_class(this: Id, _: Sel) -> Id {
    unsafe { objc_rt::object_class(this) as Id }
}
extern "C" fn so_superclass(this: Id, _: Sel) -> Id {
    objc_rt::superclass(unsafe { objc_rt::object_class(this) }) as Id
}
extern "C" fn so_c_superclass(this: Id, _: Sel) -> Id {
    objc_rt::superclass(this as *const ClassT) as Id
}
fn is_swift_root_instance(obj: Id) -> bool {
    let mut c = unsafe { objc_rt::object_class(obj) };
    while !c.is_null() {
        if c == SWIFT_OBJECT_CLASS.ptr() {
            return true;
        }
        c = objc_rt::superclass(c);
    }
    false
}
extern "C" fn so_retain(this: Id, _: Sel) -> Id {
    if is_tracked_swift_object(this as usize) {
        swift_retain_n(this, 1)
    } else {
        object::retain_default(this)
    }
}
extern "C" fn so_release(this: Id, _: Sel) {
    if is_tracked_swift_object(this as usize) {
        swift_release_n(this, 1);
    } else {
        object::release_default(this);
    }
}
extern "C" fn so_retain_count(this: Id, _: Sel) -> usize {
    if this.is_null() {
        return 0;
    }
    if is_tracked_swift_object(this as usize) {
        let obj = unsafe { &*(this as *const HeapObject) };
        let rc = obj.refcounts.load(Ordering::Acquire);
        if rc == IMMORTAL_REFCOUNT {
            return usize::MAX;
        }
        return (rc & STRONG_MASK) as usize;
    }
    1
}
extern "C" fn so_dealloc(this: Id, _: Sel) {
    swift_dealloc_object(this, 16, 7);
}
extern "C" fn so_is_kind(this: Id, _: Sel, cls: *const ClassT) -> bool {
    let mut c = unsafe { objc_rt::object_class(this) };
    while !c.is_null() {
        if c == cls {
            return true;
        }
        c = objc_rt::superclass(c);
    }
    false
}
extern "C" fn so_is_member(this: Id, _: Sel, cls: *const ClassT) -> bool {
    unsafe { objc_rt::object_class(this) == cls }
}
extern "C" fn so_responds(this: Id, _: Sel, s: Sel) -> bool {
    unsafe { objc_rt::lookup(objc_rt::object_class(this), s).is_some() }
}
extern "C" fn so_conforms(this: Id, _: Sel, proto: *const pear_libobjc::abi::ProtocolT) -> bool {
    if proto.is_null() {
        return false;
    }
    let name = unsafe { CStr::from_ptr((*proto).mangled_name) };
    unsafe { objc_rt::conforms(objc_rt::object_class(this), name) }
}
extern "C" fn so_is_equal(this: Id, _: Sel, other: Id) -> bool {
    this == other
}
extern "C" fn so_hash(this: Id, _: Sel) -> usize {
    this as usize
}
extern "C" fn so_description(this: Id, _: Sel) -> Id {
    let name = unsafe {
        let cls = objc_rt::object_class(this);
        objc_rt::class(cls).map_or_else(|| "SwiftObject".into(), |c| c.name.clone())
    };
    pear_foundation::nsstring_from_rust(&format!("<{name}: {:#x}>", this as usize))
}
extern "C" fn so_c_alloc(this: Id, _: Sel) -> Id {
    let size = objc_rt::class(this as *const ClassT)
        .map(|c| c.size().max(16))
        .unwrap_or(16);
    swift_alloc_object(this as *const ClassT, size, 7)
}
extern "C" fn so_c_alloc_zone(this: Id, s: Sel, _zone: *mut c_void) -> Id {
    so_c_alloc(this, s)
}
extern "C" fn so_c_new(this: Id, s: Sel) -> Id {
    let obj = so_c_alloc(this, s);
    if obj.is_null() {
        return obj;
    }
    let init_sel = objc_rt::sel("init");
    if let Some(imp) = unsafe { objc_rt::lookup(objc_rt::object_class(obj), init_sel) } {
        let f: extern "C" fn(Id, Sel) -> Id = unsafe { std::mem::transmute(imp) };
        return f(obj, init_sel);
    }
    obj
}
extern "C" fn so_c_is_subclass(this: Id, _: Sel, cls: *const ClassT) -> bool {
    let mut c = this as *const ClassT;
    while !c.is_null() {
        if c == cls {
            return true;
        }
        c = objc_rt::superclass(c);
    }
    false
}
extern "C" fn so_c_instances_respond(this: Id, _: Sel, s: Sel) -> bool {
    objc_rt::lookup(this as *const ClassT, s).is_some()
}

// ---- Swift heap object / refcount helpers -----------------------------------

#[inline]
pub fn is_tagged_or_null(addr: usize) -> bool {
    addr < 0x10000 || (addr & ((1usize << 63) | 1)) != 0
}

#[inline]
pub fn is_tracked_swift_object(addr: usize) -> bool {
    if addr < 0x10000 {
        return false;
    }
    SWIFT_OBJECTS.lock().unwrap().contains(&addr)
}

#[inline]
fn normalize_bridge_ptr(addr: usize) -> usize {
    // Swift arm64 BridgeObject can carry tag bits in 60..63 or 0..2.
    if (addr & 1) != 0 && !SWIFT_OBJECTS.lock().unwrap().contains(&(addr & !0x7)) {
        return 0;
    }
    let masked = addr & 0x0000_FFFF_FFFF_FFF8;
    if masked < 0x10000 {
        0
    } else {
        masked
    }
}

pub extern "C" fn swift_alloc_object(meta: *const ClassT, required_size: usize, _align_mask: usize) -> Id {
    if meta.is_null() {
        return std::ptr::null_mut();
    }
    let size = required_size.max(std::mem::size_of::<HeapObject>());
    unsafe {
        let ptr = calloc(1, size) as *mut HeapObject;
        if ptr.is_null() {
            return std::ptr::null_mut();
        }
        (*ptr).isa = meta;
        (*ptr).refcounts = AtomicU64::new(1 | (1u64 << UNOWNED_SHIFT));
        SWIFT_OBJECTS.lock().unwrap().insert(ptr as usize);
        ptr as Id
    }
}

pub extern "C" fn swift_init_stack_object(meta: *const ClassT, obj: Id) -> Id {
    if !obj.is_null() {
        unsafe {
            let ho = &*(obj as *const HeapObject);
            *(obj as *mut *const ClassT) = meta;
            ho.refcounts.store(IMMORTAL_REFCOUNT, Ordering::Release);
        }
    }
    obj
}

pub extern "C" fn swift_init_static_object(meta: *const ClassT, obj: Id) -> Id {
    swift_init_stack_object(meta, obj)
}

pub extern "C" fn swift_set_deallocating(obj: Id) {
    if is_tracked_swift_object(obj as usize) {
        let ho = unsafe { &*(obj as *const HeapObject) };
        let rc = ho.refcounts.load(Ordering::Acquire);
        if rc != IMMORTAL_REFCOUNT {
            ho.refcounts.fetch_or(DEALLOCATING_BIT, Ordering::AcqRel);
        }
    }
}

fn clear_weak_refs_for(target: usize) {
    let mut weak = WEAK_SLOTS.lock().unwrap();
    for val in weak.values_mut() {
        if *val == target {
            *val = 0;
        }
    }
}

pub extern "C" fn swift_dealloc_object(obj: Id, _allocated_size: usize, _align_mask: usize) {
    let addr = obj as usize;
    if addr < 0x10000 {
        return;
    }
    clear_weak_refs_for(addr);
    let was_tracked = SWIFT_OBJECTS.lock().unwrap().remove(&addr);
    if was_tracked {
        unsafe { free(obj as *mut c_void) };
    }
}

pub extern "C" fn swift_dealloc_class_instance(obj: Id, size: usize, align_mask: usize) {
    let addr = obj as usize;
    if addr < 0x10000 {
        return;
    }
    if is_tracked_swift_object(addr) {
        swift_dealloc_object(obj, size, align_mask);
    } else {
        unsafe { object::dispose(obj) };
    }
}

pub extern "C" fn swift_dealloc_partial_class_instance(
    obj: Id,
    _cls: *const ClassT,
    size: usize,
    align_mask: usize,
) {
    swift_dealloc_class_instance(obj, size, align_mask);
}

pub extern "C" fn swift_dealloc_uninitialized_object(obj: Id, size: usize, align_mask: usize) {
    swift_dealloc_object(obj, size, align_mask);
}

pub extern "C" fn swift_slow_alloc(bytes: usize, _align_mask: usize) -> *mut c_void {
    unsafe { calloc(1, bytes.max(1)) }
}

pub extern "C" fn swift_slow_dealloc(ptr: *mut c_void, _bytes: usize, _align_mask: usize) {
    if !ptr.is_null() {
        unsafe { free(ptr) };
    }
}

#[repr(C)]
pub struct BoxPair {
    pub heap_object: Id,
    pub buffer: *mut c_void,
}

#[repr(C)]
struct GenericBoxHeader {
    isa: *const ClassT,
    refcounts: AtomicU64,
    value_offset: usize,
}

pub extern "C" fn swift_alloc_box(ty: *const c_void) -> BoxPair {
    let payload_size = crate::metadata::type_size(ty).max(16);
    let total = 32 + payload_size;
    let obj = swift_alloc_object(SWIFT_OBJECT_CLASS.ptr(), total, 7);
    if obj.is_null() {
        return BoxPair {
            heap_object: std::ptr::null_mut(),
            buffer: std::ptr::null_mut(),
        };
    }
    let buf = unsafe {
        let bh = &mut *(obj as *mut GenericBoxHeader);
        bh.value_offset = 32;
        (obj as *mut u8).add(32) as *mut c_void
    };
    BoxPair {
        heap_object: obj,
        buffer: buf,
    }
}

pub extern "C" fn swift_project_box(obj: Id) -> *mut c_void {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (obj as *mut u8).add(32) as *mut c_void }
}

pub extern "C" fn swift_make_box_unique(
    buffer: *mut c_void,
    ty: *const c_void,
    _align_mask: usize,
) -> BoxPair {
    let size = crate::metadata::type_size(ty).max(16);
    let current = if buffer.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { *(buffer as *mut Id) }
    };
    if !current.is_null() && swift_is_uniquely_referenced_native(current) {
        return BoxPair {
            heap_object: current,
            buffer: swift_project_box(current),
        };
    }
    let new_box = swift_alloc_box(ty);
    if !current.is_null() && !new_box.buffer.is_null() {
        unsafe {
            std::ptr::copy_nonoverlapping(
                swift_project_box(current) as *const u8,
                new_box.buffer as *mut u8,
                size,
            );
            *(buffer as *mut Id) = new_box.heap_object;
        }
        swift_release(current);
    } else if !buffer.is_null() {
        unsafe { *(buffer as *mut Id) = new_box.heap_object };
    }
    new_box
}

pub extern "C" fn swift_retain_n(obj: Id, n: u32) -> Id {
    let addr = obj as usize;
    if is_tagged_or_null(addr) || n == 0 {
        return obj;
    }
    if is_tracked_swift_object(addr) {
        let ho = unsafe { &*(obj as *const HeapObject) };
        loop {
            let cur = ho.refcounts.load(Ordering::Acquire);
            if cur == IMMORTAL_REFCOUNT || (cur & DEALLOCATING_BIT) != 0 {
                return obj;
            }
            let strong = cur & STRONG_MASK;
            let next = (cur & !STRONG_MASK) | (strong.saturating_add(n as u64) & STRONG_MASK);
            if ho
                .refcounts
                .compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return obj;
            }
        }
    }
    if is_swift_root_instance(obj) {
        for _ in 0..n {
            object::retain_default(obj);
        }
    } else {
        for _ in 0..n {
            object::retain(obj);
        }
    }
    obj
}

pub extern "C" fn swift_retain(obj: Id) -> Id {
    swift_retain_n(obj, 1)
}

pub extern "C" fn swift_release_n(obj: Id, n: u32) {
    let addr = obj as usize;
    if is_tagged_or_null(addr) || n == 0 {
        return;
    }
    if is_tracked_swift_object(addr) {
        let ho = unsafe { &*(obj as *const HeapObject) };
        loop {
            let cur = ho.refcounts.load(Ordering::Acquire);
            if cur == IMMORTAL_REFCOUNT {
                return;
            }
            let strong = cur & STRONG_MASK;
            if strong <= n as u64 {
                if ho
                    .refcounts
                    .compare_exchange_weak(
                        cur,
                        (cur & !STRONG_MASK) | DEALLOCATING_BIT,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    let unowned = (cur & UNOWNED_MASK) >> UNOWNED_SHIFT;
                    clear_weak_refs_for(addr);
                    if unowned <= 1 {
                        swift_dealloc_object(obj, 16, 7);
                    }
                    return;
                }
            } else {
                let next = (cur & !STRONG_MASK) | ((strong - n as u64) & STRONG_MASK);
                if ho
                    .refcounts
                    .compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return;
                }
            }
        }
    }
    if is_swift_root_instance(obj) {
        for _ in 0..n {
            object::release_default(obj);
        }
    } else {
        for _ in 0..n {
            object::release(obj);
        }
    }
}

pub extern "C" fn swift_release(obj: Id) {
    swift_release_n(obj, 1);
}

pub extern "C" fn swift_bridge_object_retain(obj: Id) -> Id {
    swift_bridge_object_retain_n(obj, 1)
}

pub extern "C" fn swift_bridge_object_retain_n(obj: Id, n: u32) -> Id {
    let addr = obj as usize;
    let raw = normalize_bridge_ptr(addr);
    if raw != 0 {
        if is_tracked_swift_object(raw) || ((addr & (1usize << 63)) == 0 && (addr & 1) == 0) {
            swift_retain_n(raw as Id, n);
        }
    }
    obj
}

pub extern "C" fn swift_bridge_object_release(obj: Id) {
    swift_bridge_object_release_n(obj, 1);
}

pub extern "C" fn swift_bridge_object_release_n(obj: Id, n: u32) {
    let addr = obj as usize;
    let raw = normalize_bridge_ptr(addr);
    if raw != 0 {
        if is_tracked_swift_object(raw) || ((addr & (1usize << 63)) == 0 && (addr & 1) == 0) {
            swift_release_n(raw as Id, n);
        }
    }
}

pub extern "C" fn swift_unknown_object_retain(obj: Id) -> Id {
    swift_retain(obj)
}

pub extern "C" fn swift_unknown_object_retain_n(obj: Id, n: u32) -> Id {
    swift_retain_n(obj, n)
}

pub extern "C" fn swift_unknown_object_release(obj: Id) {
    swift_release(obj);
}

pub extern "C" fn swift_unknown_object_release_n(obj: Id, n: u32) {
    swift_release_n(obj, n);
}

pub extern "C" fn swift_unowned_retain(obj: Id) -> Id {
    let addr = obj as usize;
    if is_tracked_swift_object(addr) {
        let ho = unsafe { &*(obj as *const HeapObject) };
        loop {
            let cur = ho.refcounts.load(Ordering::Acquire);
            if cur == IMMORTAL_REFCOUNT {
                return obj;
            }
            let unowned = (cur & UNOWNED_MASK) >> UNOWNED_SHIFT;
            let next = (cur & !UNOWNED_MASK) | ((unowned.saturating_add(1) << UNOWNED_SHIFT) & UNOWNED_MASK);
            if ho
                .refcounts
                .compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return obj;
            }
        }
    }
    swift_retain(obj)
}

pub extern "C" fn swift_unowned_release(obj: Id) {
    let addr = obj as usize;
    if is_tracked_swift_object(addr) {
        let ho = unsafe { &*(obj as *const HeapObject) };
        loop {
            let cur = ho.refcounts.load(Ordering::Acquire);
            if cur == IMMORTAL_REFCOUNT {
                return;
            }
            let unowned = (cur & UNOWNED_MASK) >> UNOWNED_SHIFT;
            let new_unowned = unowned.saturating_sub(1);
            let strong = cur & STRONG_MASK;
            let dealloc = (cur & DEALLOCATING_BIT) != 0;
            let next = (cur & !UNOWNED_MASK) | ((new_unowned << UNOWNED_SHIFT) & UNOWNED_MASK);
            if ho
                .refcounts
                .compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                if dealloc && strong == 0 && new_unowned == 0 {
                    swift_dealloc_object(obj, 16, 7);
                }
                return;
            }
        }
    } else {
        swift_release(obj);
    }
}

pub extern "C" fn swift_unowned_retain_strong(obj: Id) -> Id {
    let addr = obj as usize;
    if is_tracked_swift_object(addr) {
        let ho = unsafe { &*(obj as *const HeapObject) };
        let rc = ho.refcounts.load(Ordering::Acquire);
        if rc != IMMORTAL_REFCOUNT && (rc & DEALLOCATING_BIT) != 0 {
            return std::ptr::null_mut();
        }
    }
    swift_retain(obj)
}

pub extern "C" fn swift_unknown_object_unowned_init(ref_ptr: *mut Id, value: Id) -> *mut Id {
    if !ref_ptr.is_null() {
        unsafe { *ref_ptr = value };
        if !value.is_null() {
            swift_unowned_retain(value);
        }
    }
    ref_ptr
}

pub extern "C" fn swift_unknown_object_unowned_load_strong(ref_ptr: *mut Id) -> Id {
    if ref_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let target = unsafe { *ref_ptr };
    if target.is_null() {
        return std::ptr::null_mut();
    }
    swift_unowned_retain_strong(target)
}

pub extern "C" fn swift_unknown_object_unowned_destroy(ref_ptr: *mut Id) {
    if !ref_ptr.is_null() {
        let target = unsafe { *ref_ptr };
        unsafe { *ref_ptr = std::ptr::null_mut() };
        if !target.is_null() {
            swift_unowned_release(target);
        }
    }
}

pub extern "C" fn swift_weak_init(ref_ptr: *mut Id, value: Id) -> *mut Id {
    if !ref_ptr.is_null() {
        unsafe { *ref_ptr = value };
        if is_tracked_swift_object(value as usize) {
            WEAK_SLOTS.lock().unwrap().insert(ref_ptr as usize, value as usize);
        }
    }
    ref_ptr
}

pub extern "C" fn swift_weak_assign(ref_ptr: *mut Id, value: Id) -> *mut Id {
    swift_weak_init(ref_ptr, value)
}

pub extern "C" fn swift_weak_load_strong(ref_ptr: *mut Id) -> Id {
    if ref_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let val = {
        let slots = WEAK_SLOTS.lock().unwrap();
        if let Some(&tracked) = slots.get(&(ref_ptr as usize)) {
            tracked
        } else {
            unsafe { *ref_ptr as usize }
        }
    };
    if val == 0 {
        unsafe { *ref_ptr = std::ptr::null_mut() };
        return std::ptr::null_mut();
    }
    if is_tracked_swift_object(val) {
        let ho = unsafe { &*(val as *const HeapObject) };
        let rc = ho.refcounts.load(Ordering::Acquire);
        if rc != IMMORTAL_REFCOUNT && (rc & DEALLOCATING_BIT) != 0 {
            return std::ptr::null_mut();
        }
        return swift_retain(val as Id);
    }
    if is_tagged_or_null(val) {
        val as Id
    } else {
        swift_retain(val as Id)
    }
}

pub extern "C" fn swift_weak_destroy(ref_ptr: *mut Id) {
    if !ref_ptr.is_null() {
        WEAK_SLOTS.lock().unwrap().remove(&(ref_ptr as usize));
        unsafe { *ref_ptr = std::ptr::null_mut() };
    }
}

pub extern "C" fn swift_unknown_object_weak_init(ref_ptr: *mut Id, value: Id) -> *mut Id {
    swift_weak_init(ref_ptr, value)
}

pub extern "C" fn swift_unknown_object_weak_assign(ref_ptr: *mut Id, value: Id) -> *mut Id {
    swift_weak_assign(ref_ptr, value)
}

pub extern "C" fn swift_unknown_object_weak_copy_init(dest: *mut Id, src: *mut Id) -> *mut Id {
    let v = if src.is_null() { std::ptr::null_mut() } else { swift_weak_load_strong(src) };
    swift_weak_init(dest, v);
    if !v.is_null() {
        swift_release(v);
    }
    dest
}

pub extern "C" fn swift_unknown_object_weak_copy_assign(dest: *mut Id, src: *mut Id) -> *mut Id {
    if dest != src {
        swift_unknown_object_weak_copy_init(dest, src);
    }
    dest
}

pub extern "C" fn swift_unknown_object_weak_take_init(dest: *mut Id, src: *mut Id) -> *mut Id {
    if !dest.is_null() && !src.is_null() {
        let val = {
            let mut slots = WEAK_SLOTS.lock().unwrap();
            let v = slots.remove(&(src as usize)).unwrap_or_else(|| unsafe { *src as usize });
            if v != 0 {
                slots.insert(dest as usize, v);
            }
            v
        };
        unsafe {
            *dest = val as Id;
            *src = std::ptr::null_mut();
        }
    }
    dest
}

pub extern "C" fn swift_unknown_object_weak_take_assign(dest: *mut Id, src: *mut Id) -> *mut Id {
    if dest != src {
        swift_weak_destroy(dest);
        swift_unknown_object_weak_take_init(dest, src);
    }
    dest
}

pub extern "C" fn swift_unknown_object_weak_load_strong(ref_ptr: *mut Id) -> Id {
    swift_weak_load_strong(ref_ptr)
}

pub extern "C" fn swift_unknown_object_weak_destroy(ref_ptr: *mut Id) {
    swift_weak_destroy(ref_ptr);
}

pub extern "C" fn swift_is_uniquely_referenced_native(obj: Id) -> bool {
    let addr = obj as usize;
    if is_tagged_or_null(addr) {
        return false;
    }
    if is_tracked_swift_object(addr) {
        let ho = unsafe { &*(obj as *const HeapObject) };
        let rc = ho.refcounts.load(Ordering::Acquire);
        return rc != IMMORTAL_REFCOUNT && (rc & STRONG_MASK) == 1 && (rc & DEALLOCATING_BIT) == 0;
    }
    false
}

pub extern "C" fn swift_is_uniquely_referenced_bridge(obj: Id) -> bool {
    let addr = obj as usize;
    if (addr & (1usize << 63)) != 0 {
        return false;
    }
    let raw = normalize_bridge_ptr(addr);
    swift_is_uniquely_referenced_native(raw as Id)
}

// ---- Synchronization & exclusivity -----------------------------------------

/// Swift ABI `_swift_once`: sentinel `-1` (`usize::MAX`) marks initialized.
pub extern "C" fn swift_once(pred: *mut usize, f: extern "C" fn(*mut c_void), ctx: *mut c_void) {
    if pred.is_null() {
        return;
    }
    let atom = unsafe { &*(pred as *const AtomicUsize) };
    if atom.load(Ordering::Acquire) == usize::MAX {
        return;
    }
    match atom.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            f(ctx);
            atom.store(usize::MAX, Ordering::Release);
        }
        Err(_) => {
            while atom.load(Ordering::Acquire) == 1 {
                std::thread::yield_now();
            }
        }
    }
}

pub extern "C" fn swift_begin_access(_pointer: *mut c_void, buffer: *mut c_void, _flags: usize, _pc: *mut c_void) {
    if !buffer.is_null() {
        unsafe { std::ptr::write_bytes(buffer as *mut u8, 0, 32) };
    }
}

pub extern "C" fn swift_end_access(_buffer: *mut c_void) {}

// ---- Error handling & diagnostics ------------------------------------------

pub extern "C" fn swift_alloc_error(
    ty: *const c_void,
    conformance: *const c_void,
    _initial_value: *mut c_void,
    _is_take: bool,
) -> BoxPair {
    let size = crate::metadata::type_size(ty).max(16);
    let box_obj = swift_alloc_object(SWIFT_OBJECT_CLASS.ptr(), 32 + size, 7);
    if !box_obj.is_null() {
        BOX_ERRORS
            .lock()
            .unwrap()
            .insert(box_obj as usize, (ty as usize, conformance as usize));
    }
    let buf = if box_obj.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (box_obj as *mut u8).add(32) as *mut c_void }
    };
    BoxPair {
        heap_object: box_obj,
        buffer: buf,
    }
}

pub extern "C" fn swift_error_retain(err: Id) -> Id {
    swift_retain(err)
}

pub extern "C" fn swift_error_release(err: Id) {
    if !err.is_null() && !is_tagged_or_null(err as usize) {
        BOX_ERRORS.lock().unwrap().remove(&(err as usize));
    }
    swift_release(err);
}

#[repr(C)]
pub struct ErrorValueOut {
    pub value: *const c_void,
    pub ty: *const c_void,
    pub witness: *const c_void,
}

pub extern "C" fn swift_get_error_value(err: Id, _scratch: *mut *mut c_void, out: *mut ErrorValueOut) {
    if out.is_null() || err.is_null() {
        return;
    }
    let meta = BOX_ERRORS.lock().unwrap().get(&(err as usize)).copied();
    unsafe {
        if let Some((ty, w)) = meta {
            (*out).value = (err as *const u8).add(32) as *const c_void;
            (*out).ty = ty as *const c_void;
            (*out).witness = w as *const c_void;
        } else {
            (*out).value = err as *const c_void;
            (*out).ty = crate::metadata::anyobject_metadata();
            (*out).witness = std::ptr::null();
        }
    }
}

pub extern "C" fn swift_will_throw(_ctx: *mut c_void, _err: *mut c_void) {}

pub extern "C" fn swift_unexpected_error(err: Id, _filename: *const c_char, _len: usize, _ascii: bool, _line: usize) {
    eprintln!("[pear-swift-rt] swift_unexpectedError({:#x})", err as usize);
}

pub extern "C" fn swift_deleted_method_error() {
    eprintln!("[pear-swift-rt] warning: _swift_deletedMethodError called");
}

pub extern "C" fn swift_is_escaping_closure_at_file_location(
    _obj: Id,
    _file: *const u8,
    _len: usize,
    _line: u32,
    _col: u32,
    _flags: u32,
) -> bool {
    false
}

pub extern "C" fn swift_stdlib_report_unimplemented_initializer(
    cls_name: *const u8,
    cls_len: usize,
    init_name: *const u8,
    init_len: usize,
) {
    let cls = if !cls_name.is_null() && cls_len > 0 {
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(cls_name, cls_len.min(256)) }).into_owned()
    } else {
        "<class>".into()
    };
    let init = if !init_name.is_null() && init_len > 0 {
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(init_name, init_len.min(256)) }).into_owned()
    } else {
        "init".into()
    };
    eprintln!("[pear-swift-rt] unimplemented initializer {cls}.{init}");
}

// ---- Stdlib OS / math / RNG helpers ----------------------------------------

static RNG_SEED: AtomicU64 = AtomicU64::new(0x5EED_CAFE_BABE_1234);

pub extern "C" fn swift_stdlib_random(buf: *mut u8, bytes: usize) {
    if buf.is_null() || bytes == 0 {
        return;
    }
    let mut x = RNG_SEED.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    if x == 0 {
        x = 0x1234_5678_9ABC_DEF0;
    }
    for i in 0..bytes {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        unsafe { *buf.add(i) = (x >> ((i & 7) * 8)) as u8 };
    }
}

pub extern "C" fn swift_stdlib_strtod_clocale(nptr: *const c_char, endptr: *mut *mut c_char) -> c_double {
    if nptr.is_null() {
        return 0.0;
    }
    unsafe { strtod(nptr, endptr) }
}

pub extern "C" fn swift_stdlib_strtof_clocale(nptr: *const c_char, endptr: *mut *mut c_char) -> c_float {
    if nptr.is_null() {
        return 0.0;
    }
    unsafe { strtof(nptr, endptr) }
}

pub extern "C" fn swift_stdlib_operating_system_version() -> SwiftOsVersion {
    SwiftOsVersion {
        major: 17,
        minor: 0,
        patch: 0,
    }
}

static ONCE_TRACKER: AtomicBool = AtomicBool::new(false);
pub fn touch_runtime_init() {
    let _ = ONCE_TRACKER.load(Ordering::Relaxed);
}

/// Resolves all non-`_$s` Swift 5 runtime symbols (`_swift_*`, `__swift*`, `_TtCs12_SwiftObject`).
pub fn runtime_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        // Root class
        "_OBJC_CLASS_$__TtCs12_SwiftObject" => Some(SWIFT_OBJECT_CLASS.ptr() as usize),
        "_OBJC_METACLASS_$__TtCs12_SwiftObject" => Some(SWIFT_OBJECT_META.ptr() as usize),
        // Globals
        "_swift_isaMask" => Some(&raw const SWIFT_ISA_MASK as usize),
        "__swiftEmptyArrayStorage" => Some(&raw const EMPTY_ARRAY_STORAGE as usize),
        "__swiftEmptyDictionarySingleton" => Some(&raw const EMPTY_DICTIONARY_SINGLETON as usize),
        "__swiftEmptySetSingleton" => Some(&raw const EMPTY_SET_SINGLETON as usize),
        // Alloc / dealloc / box
        "_swift_allocObject" => f(swift_alloc_object as *const ()),
        "_swift_deallocObject" => f(swift_dealloc_object as *const ()),
        "_swift_deallocClassInstance" => f(swift_dealloc_class_instance as *const ()),
        "_swift_deallocPartialClassInstance" => f(swift_dealloc_partial_class_instance as *const ()),
        "_swift_deallocUninitializedObject" => f(swift_dealloc_uninitialized_object as *const ()),
        "_swift_initStackObject" => f(swift_init_stack_object as *const ()),
        "_swift_initStaticObject" => f(swift_init_static_object as *const ()),
        "_swift_setDeallocating" => f(swift_set_deallocating as *const ()),
        "_swift_slowAlloc" | "_swift_coroFrameAlloc" => f(swift_slow_alloc as *const ()),
        "_swift_slowDealloc" => f(swift_slow_dealloc as *const ()),
        "_swift_allocBox" => f(swift_alloc_box as *const ()),
        "_swift_projectBox" => f(swift_project_box as *const ()),
        "_swift_makeBoxUnique" => f(swift_make_box_unique as *const ()),
        // ARC / bridge / unknown / weak / unowned
        "_swift_retain" => f(swift_retain as *const ()),
        "_swift_retain_n" => f(swift_retain_n as *const ()),
        "_swift_release" => f(swift_release as *const ()),
        "_swift_release_n" => f(swift_release_n as *const ()),
        "_swift_bridgeObjectRetain" => f(swift_bridge_object_retain as *const ()),
        "_swift_bridgeObjectRetain_n" => f(swift_bridge_object_retain_n as *const ()),
        "_swift_bridgeObjectRelease" => f(swift_bridge_object_release as *const ()),
        "_swift_bridgeObjectRelease_n" => f(swift_bridge_object_release_n as *const ()),
        "_swift_unknownObjectRetain" => f(swift_unknown_object_retain as *const ()),
        "_swift_unknownObjectRetain_n" => f(swift_unknown_object_retain_n as *const ()),
        "_swift_unknownObjectRelease" => f(swift_unknown_object_release as *const ()),
        "_swift_unknownObjectRelease_n" => f(swift_unknown_object_release_n as *const ()),
        "_swift_unownedRetain" => f(swift_unowned_retain as *const ()),
        "_swift_unownedRelease" => f(swift_unowned_release as *const ()),
        "_swift_unownedRetainStrong" => f(swift_unowned_retain_strong as *const ()),
        "_swift_unknownObjectUnownedInit" => f(swift_unknown_object_unowned_init as *const ()),
        "_swift_unknownObjectUnownedLoadStrong" => f(swift_unknown_object_unowned_load_strong as *const ()),
        "_swift_unknownObjectUnownedDestroy" => f(swift_unknown_object_unowned_destroy as *const ()),
        "_swift_weakInit" => f(swift_weak_init as *const ()),
        "_swift_weakAssign" => f(swift_weak_assign as *const ()),
        "_swift_weakLoadStrong" => f(swift_weak_load_strong as *const ()),
        "_swift_weakDestroy" => f(swift_weak_destroy as *const ()),
        "_swift_unknownObjectWeakInit" => f(swift_unknown_object_weak_init as *const ()),
        "_swift_unknownObjectWeakAssign" => f(swift_unknown_object_weak_assign as *const ()),
        "_swift_unknownObjectWeakCopyInit" => f(swift_unknown_object_weak_copy_init as *const ()),
        "_swift_unknownObjectWeakCopyAssign" => f(swift_unknown_object_weak_copy_assign as *const ()),
        "_swift_unknownObjectWeakTakeInit" => f(swift_unknown_object_weak_take_init as *const ()),
        "_swift_unknownObjectWeakTakeAssign" => f(swift_unknown_object_weak_take_assign as *const ()),
        "_swift_unknownObjectWeakLoadStrong" => f(swift_unknown_object_weak_load_strong as *const ()),
        "_swift_unknownObjectWeakDestroy" => f(swift_unknown_object_weak_destroy as *const ()),
        "_swift_isUniquelyReferenced_nonNull_native" => f(swift_is_uniquely_referenced_native as *const ()),
        "_swift_isUniquelyReferencedNonObjC_nonNull_bridgeObject" => f(swift_is_uniquely_referenced_bridge as *const ()),
        "_swift_errorRetain" => f(swift_error_retain as *const ()),
        "_swift_errorRelease" => f(swift_error_release as *const ()),
        // Sync / exclusivity / errors / stdlib helpers
        "_swift_once" => f(swift_once as *const ()),
        "_swift_beginAccess" => f(swift_begin_access as *const ()),
        "_swift_endAccess" => f(swift_end_access as *const ()),
        "_swift_allocError" => f(swift_alloc_error as *const ()),
        "_swift_getErrorValue" => f(swift_get_error_value as *const ()),
        "_swift_willThrow" => f(swift_will_throw as *const ()),
        "_swift_unexpectedError" => f(swift_unexpected_error as *const ()),
        "_swift_deletedMethodError" => f(swift_deleted_method_error as *const ()),
        "_swift_isEscapingClosureAtFileLocation" => f(swift_is_escaping_closure_at_file_location as *const ()),
        "__swift_stdlib_reportUnimplementedInitializer" => f(swift_stdlib_report_unimplemented_initializer as *const ()),
        "_swift_stdlib_random" => f(swift_stdlib_random as *const ()),
        "__swift_stdlib_strtod_clocale" => f(swift_stdlib_strtod_clocale as *const ()),
        "__swift_stdlib_strtof_clocale" => f(swift_stdlib_strtof_clocale as *const ()),
        "__swift_stdlib_operatingSystemVersion" => f(swift_stdlib_operating_system_version as *const ()),
        _ => None,
    }
}
