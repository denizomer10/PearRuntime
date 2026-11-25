//! NSObject (the root class libobjc provides), object lifetime and the ARC
//! entry points: allocation, retain counts in a side table, dealloc with
//! `.cxx_destruct`, zeroing weak references and autorelease pools.

use crate::abi::*;
use crate::runtime::{self, sel};
use std::cell::{RefCell, UnsafeCell};
use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::sync::{Mutex, Once, OnceLock};

extern "C" {
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
}

// ---- NSObject's class objects ---------------------------------------------

/// A class object the runtime defines (its address is what images bind to).
#[repr(transparent)]
pub struct StaticClass(pub UnsafeCell<ClassT>);
unsafe impl Sync for StaticClass {}

impl StaticClass {
    pub fn ptr(&self) -> *const ClassT {
        self.0.get()
    }
}

/// `_objc_empty_cache`: what clang puts in a class's cache field.
#[repr(transparent)]
pub struct EmptyCache(pub [usize; 4]);
pub static EMPTY_CACHE: EmptyCache = EmptyCache([0; 4]);

pub static NSOBJECT_META: StaticClass = StaticClass(UnsafeCell::new(ClassT {
    isa: &raw const NSOBJECT_META as *const ClassT,
    superclass: &raw const NSOBJECT_CLASS as *const ClassT,
    cache: &raw const EMPTY_CACHE as *const c_void,
    vtable: std::ptr::null(),
    bits: 0,
}));

pub static NSOBJECT_CLASS: StaticClass = StaticClass(UnsafeCell::new(ClassT {
    isa: &raw const NSOBJECT_META as *const ClassT,
    superclass: std::ptr::null(),
    cache: &raw const EMPTY_CACHE as *const c_void,
    vtable: std::ptr::null(),
    bits: 0,
}));

/// Registers the root classes once.
pub fn init_runtime() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let i = |f: *const ()| f as Imp;
        runtime::register_builtin(
            NSOBJECT_CLASS.ptr(),
            NSOBJECT_META.ptr(),
            "NSObject",
            8,
            &[
                ("init", i(m_self as *const ())),
                ("self", i(m_self as *const ())),
                ("class", i(m_class as *const ())),
                ("superclass", i(m_superclass as *const ())),
                ("retain", i(m_retain as *const ())),
                ("release", i(m_release as *const ())),
                ("autorelease", i(m_autorelease as *const ())),
                ("retainCount", i(m_retain_count as *const ())),
                ("dealloc", i(m_dealloc as *const ())),
                ("isKindOfClass:", i(m_is_kind as *const ())),
                ("isMemberOfClass:", i(m_is_member as *const ())),
                ("respondsToSelector:", i(m_responds as *const ())),
                ("conformsToProtocol:", i(m_conforms as *const ())),
                ("isEqual:", i(m_is_equal as *const ())),
                ("hash", i(m_hash as *const ())),
                ("isProxy", i(m_no as *const ())),
                ("zone", i(m_null as *const ())),
                ("performSelector:", i(m_perform as *const ())),
                ("performSelector:withObject:", i(m_perform_with as *const ())),
                ("copy", i(m_copy as *const ())),
                ("mutableCopy", i(m_mutable_copy as *const ())),
                ("methodForSelector:", i(m_method_for as *const ())),
                ("doesNotRecognizeSelector:", i(m_does_not_recognize as *const ())),
                ("forwardingTargetForSelector:", i(m_forwarding_target as *const ())),
            ],
            &[
                ("alloc", i(c_alloc as *const ())),
                ("allocWithZone:", i(c_alloc as *const ())),
                ("new", i(c_new as *const ())),
                ("class", i(m_self as *const ())),
                ("superclass", i(c_superclass as *const ())),
                ("initialize", i(m_void as *const ())),
                ("load", i(m_void as *const ())),
                ("instancesRespondToSelector:", i(c_instances_respond as *const ())),
                ("conformsToProtocol:", i(c_conforms as *const ())),
                ("isSubclassOfClass:", i(c_is_subclass as *const ())),
                ("resolveInstanceMethod:", i(c_resolve as *const ())),
                ("resolveClassMethod:", i(c_resolve as *const ())),
            ],
        );
    });
}

// ---- sending from Rust -----------------------------------------------------

unsafe fn send0(obj: Id, s: Sel) -> Id {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let d = crate::message::lookup_for_send(obj, s);
    let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
    f(d.receiver, s)
}

// ---- NSObject methods ------------------------------------------------------

extern "C" fn m_self(this: Id, _: Sel) -> Id {
    this
}
extern "C" fn m_void(_: Id, _: Sel) {}
extern "C" fn m_null(_: Id, _: Sel) -> Id {
    std::ptr::null_mut()
}
extern "C" fn m_no(_: Id, _: Sel) -> bool {
    false
}
extern "C" fn m_class(this: Id, _: Sel) -> Id {
    unsafe { runtime::object_class(this) as Id }
}
extern "C" fn m_superclass(this: Id, _: Sel) -> Id {
    runtime::superclass(unsafe { runtime::object_class(this) }) as Id
}
extern "C" fn c_superclass(this: Id, _: Sel) -> Id {
    runtime::superclass(this as *const ClassT) as Id
}
extern "C" fn m_retain(this: Id, _: Sel) -> Id {
    retain_default(this)
}
extern "C" fn m_release(this: Id, _: Sel) {
    release_default(this)
}
extern "C" fn m_autorelease(this: Id, _: Sel) -> Id {
    autorelease_default(this)
}

pub(crate) fn default_rr_imps() -> (Imp, Imp, Imp) {
    (
        m_retain as *const () as Imp,
        m_release as *const () as Imp,
        m_autorelease as *const () as Imp,
    )
}
extern "C" fn m_retain_count(this: Id, _: Sel) -> usize {
    state().lock().unwrap().extra.get(&(this as usize)).copied().unwrap_or(0) + 1
}
extern "C" fn m_dealloc(this: Id, _: Sel) {
    unsafe { dispose(this) }
}
extern "C" fn m_is_kind(this: Id, _: Sel, cls: *const ClassT) -> bool {
    is_kind(unsafe { runtime::object_class(this) }, cls)
}
extern "C" fn m_is_member(this: Id, _: Sel, cls: *const ClassT) -> bool {
    unsafe { runtime::object_class(this) == cls }
}
extern "C" fn m_responds(this: Id, _: Sel, s: Sel) -> bool {
    !s.is_null() && runtime::lookup(unsafe { runtime::object_class(this) }, s).is_some()
}
extern "C" fn m_conforms(this: Id, _: Sel, proto: *const ProtocolT) -> bool {
    unsafe { conforms_to(runtime::object_class(this), proto) }
}
extern "C" fn c_conforms(this: Id, _: Sel, proto: *const ProtocolT) -> bool {
    unsafe { conforms_to(this as *const ClassT, proto) }
}
extern "C" fn c_instances_respond(this: Id, _: Sel, s: Sel) -> bool {
    !s.is_null() && runtime::lookup(this as *const ClassT, s).is_some()
}
extern "C" fn c_is_subclass(this: Id, _: Sel, cls: *const ClassT) -> bool {
    is_kind(this as *const ClassT, cls)
}
extern "C" fn m_is_equal(this: Id, _: Sel, other: Id) -> bool {
    this == other
}
extern "C" fn m_hash(this: Id, _: Sel) -> usize {
    this as usize
}
extern "C" fn m_perform(this: Id, _: Sel, s: Sel) -> Id {
    unsafe { send0(this, s) }
}
extern "C" fn m_perform_with(this: Id, _: Sel, s: Sel, arg: Id) -> Id {
    unsafe {
        let d = crate::message::lookup_for_send(this, s);
        let f: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s, arg)
    }
}
extern "C" fn m_copy(this: Id, _: Sel) -> Id {
    unsafe {
        let s = sel("copyWithZone:");
        let d = crate::message::lookup_for_send(this, s);
        let f: extern "C" fn(Id, Sel, *mut c_void) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s, std::ptr::null_mut())
    }
}
extern "C" fn m_mutable_copy(this: Id, _: Sel) -> Id {
    unsafe {
        let s = sel("mutableCopyWithZone:");
        let d = crate::message::lookup_for_send(this, s);
        let f: extern "C" fn(Id, Sel, *mut c_void) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s, std::ptr::null_mut())
    }
}
extern "C" fn m_method_for(this: Id, _: Sel, s: Sel) -> Imp {
    let cls = unsafe { runtime::object_class(this) };
    runtime::lookup(cls, s).unwrap_or(std::ptr::null())
}
extern "C" fn m_does_not_recognize(this: Id, _: Sel, s: Sel) -> ! {
    let cls = unsafe { runtime::object_class(this) };
    let cname = runtime::class(cls).map_or_else(|| "?".to_string(), |c| c.name.clone());
    let sname = runtime::sel_name(s);
    eprintln!("[pear-libobjc] -[{cname} {sname}]: unrecognized selector sent to instance {this:p}");
    std::process::abort();
}
extern "C" fn m_forwarding_target(_: Id, _: Sel, _: Sel) -> Id {
    std::ptr::null_mut()
}
extern "C" fn c_resolve(_: Id, _: Sel, _: Sel) -> bool {
    false
}

unsafe fn conforms_to(cls: *const ClassT, proto: *const ProtocolT) -> bool {
    if proto.is_null() {
        return false;
    }
    runtime::conforms(cls, CStr::from_ptr((*proto).mangled_name))
}

fn is_kind(mut c: *const ClassT, cls: *const ClassT) -> bool {
    while !c.is_null() {
        if c == cls {
            return true;
        }
        c = runtime::superclass(c);
    }
    false
}

// ---- allocation and lifetime -----------------------------------------------

/// Allocates an instance of `cls` (zeroed, isa set).
pub unsafe fn alloc(cls: *const ClassT) -> Id {
    if cls.is_null() {
        return std::ptr::null_mut();
    }
    runtime::ensure_initialized(cls);
    let size = runtime::class(cls).map_or(16, |c| c.size().max(8));
    let obj = calloc(1, size);
    if !obj.is_null() {
        *(obj as *mut *const ClassT) = cls;
        track_allocated(obj);
    }
    obj
}

pub fn track_allocated(obj: Id) {
    if !obj.is_null() {
        state().lock().unwrap().heap_allocated.insert(obj as usize);
    }
}

extern "C" fn c_alloc(cls: Id, _: Sel) -> Id {
    unsafe { alloc(cls as *const ClassT) }
}
extern "C" fn c_new(cls: Id, _: Sel) -> Id {
    unsafe { send0(send0(cls, sel("alloc")), sel("init")) }
}

#[derive(Default)]
struct State {
    /// Heap-allocated ObjC objects (static/constant Mach-O objects are not in this set).
    heap_allocated: std::collections::HashSet<usize>,
    /// Retain count minus one, for objects retained more than once.
    extra: HashMap<usize, usize>,
    deallocating: std::collections::HashSet<usize>,
    /// Weak variables pointing at each object.
    weak: HashMap<usize, Vec<usize>>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(Default::default)
}

pub fn retain_default(obj: Id) -> Id {
    if !obj.is_null() {
        let mut st = state().lock().unwrap();
        if st.heap_allocated.contains(&(obj as usize)) {
            *st.extra.entry(obj as usize).or_insert(0) += 1;
        }
    }
    obj
}

pub fn retain(obj: Id) -> Id {
    if obj.is_null() || is_tagged_pointer(obj) {
        return obj;
    }
    let cls = unsafe { runtime::object_class(obj) };
    if runtime::has_custom_rr(cls) {
        return unsafe { send0(obj, sel("retain")) };
    }
    retain_default(obj)
}

pub fn release_default(obj: Id) {
    if obj.is_null() {
        return;
    }
    let dealloc = {
        let mut st = state().lock().unwrap();
        if !st.heap_allocated.contains(&(obj as usize)) {
            return;
        }
        match st.extra.get_mut(&(obj as usize)) {
            Some(n) if *n > 0 => {
                *n -= 1;
                if *n == 0 {
                    st.extra.remove(&(obj as usize));
                }
                false
            }
            _ => st.deallocating.insert(obj as usize),
        }
    };
    if dealloc {
        unsafe {
            send0(obj, sel("dealloc"));
        }
    }
}

pub fn release(obj: Id) {
    if obj.is_null() || is_tagged_pointer(obj) {
        return;
    }
    let cls = unsafe { runtime::object_class(obj) };
    if runtime::has_custom_rr(cls) {
        unsafe {
            send0(obj, sel("release"));
        }
        return;
    }
    release_default(obj);
}

/// NSObject's dealloc: C++/ARC ivar destructors from the object's class up,
/// then weak references are zeroed and the memory freed.
pub unsafe fn dispose(obj: Id) {
    let destruct = sel(".cxx_destruct");
    let mut c = runtime::object_class(obj);
    while !c.is_null() {
        if let Some(imp) = runtime::own_method(c, destruct) {
            let f: extern "C" fn(Id, Sel) = std::mem::transmute(imp);
            f(obj, destruct);
        }
        c = runtime::superclass(c);
    }
    crate::props::objc_remove_associated_objects(obj);
    let was_heap = {
        let mut st = state().lock().unwrap();
        for location in st.weak.remove(&(obj as usize)).unwrap_or_default() {
            *(location as *mut Id) = std::ptr::null_mut();
        }
        st.deallocating.remove(&(obj as usize));
        st.extra.remove(&(obj as usize));
        st.heap_allocated.remove(&(obj as usize))
    };
    if was_heap {
        free(obj);
    }
}

// ---- weak references -------------------------------------------------------

pub unsafe fn store_weak(location: *mut Id, new: Id) -> Id {
    let mut st = state().lock().unwrap();
    let old = *location;
    if !old.is_null() {
        if let Some(list) = st.weak.get_mut(&(old as usize)) {
            list.retain(|&l| l != location as usize);
        }
    }
    if new.is_null() || st.deallocating.contains(&(new as usize)) {
        *location = std::ptr::null_mut();
        return std::ptr::null_mut();
    }
    st.weak.entry(new as usize).or_default().push(location as usize);
    *location = new;
    new
}

pub unsafe fn load_weak_retained(location: *mut Id) -> Id {
    let mut st = state().lock().unwrap();
    let obj = *location;
    if obj.is_null() || st.deallocating.contains(&(obj as usize)) {
        return std::ptr::null_mut();
    }
    *st.extra.entry(obj as usize).or_insert(0) += 1;
    obj
}

// ---- autorelease pools -------------------------------------------------------

thread_local! {
    static POOL: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

const POOL_BOUNDARY: usize = 0;

pub fn autorelease_default(obj: Id) -> Id {
    if !obj.is_null() {
        POOL.with(|p| p.borrow_mut().push(obj as usize));
    }
    obj
}

pub fn autorelease(obj: Id) -> Id {
    if obj.is_null() || is_tagged_pointer(obj) {
        return obj;
    }
    let cls = unsafe { runtime::object_class(obj) };
    if runtime::has_custom_rr(cls) {
        return unsafe { send0(obj, sel("autorelease")) };
    }
    autorelease_default(obj)
}

pub fn pool_push() -> *mut c_void {
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        p.push(POOL_BOUNDARY);
        p.len() as *mut c_void
    })
}

/// Releases everything autoreleased since the matching push (including
/// objects autoreleased while doing so), newest first.
pub fn pool_pop(token: *mut c_void) {
    let depth = (token as usize).saturating_sub(1);
    loop {
        let next = POOL.with(|p| {
            let mut p = p.borrow_mut();
            if p.len() > depth { p.pop() } else { None }
        });
        match next {
            None => break,
            Some(POOL_BOUNDARY) => continue,
            Some(obj) => release(obj as Id),
        }
    }
}
