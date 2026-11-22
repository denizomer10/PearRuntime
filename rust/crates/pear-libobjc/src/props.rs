//! Clean-room: ABI layouts and semantics from Apple's open-source objc4
//! (APSL 2.0, reference only, nothing copied) and the public objc/runtime.h
//! documentation.
//!
//! Atomic/nonatomic property accessors, struct/C++ atomic copy,
//! `@synchronized` recursive locks, fast-enumeration mutation trap, and
//! associated objects.

use crate::abi::*;
use crate::object::{autorelease, release, retain};
use crate::runtime::{self, sel};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::ThreadId;

extern "C" {
    fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
}

// ---- Striped locks for atomic properties / structs -------------------------

const STRIPE_COUNT: usize = 64;

fn property_lock(addr: usize) -> &'static Mutex<()> {
    static LOCKS: OnceLock<[Mutex<()>; STRIPE_COUNT]> = OnceLock::new();
    let arr = LOCKS.get_or_init(|| std::array::from_fn(|_| Mutex::new(())));
    &arr[(addr >> 4) & (STRIPE_COUNT - 1)]
}

unsafe fn send_copy(obj: Id, mutable: bool) -> Id {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let s = sel(if mutable { "mutableCopy" } else { "copy" });
    let d = crate::message::lookup_for_send(obj, s);
    let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
    f(d.receiver, s)
}

pub unsafe extern "C" fn objc_get_property(
    self_: Id,
    _cmd: Sel,
    offset: isize,
    atomic: bool,
) -> Id {
    if self_.is_null() {
        return std::ptr::null_mut();
    }
    let slot = (self_ as *const u8).offset(offset) as *const Id;
    if !atomic {
        return *slot;
    }
    let val = {
        let _g = property_lock(slot as usize).lock().unwrap();
        retain(*slot)
    };
    autorelease(val)
}

pub unsafe extern "C" fn objc_set_property(
    self_: Id,
    _cmd: Sel,
    offset: isize,
    new_value: Id,
    atomic: bool,
    should_copy: i8,
) {
    if self_.is_null() {
        return;
    }
    let slot = (self_ as *mut u8).offset(offset) as *mut Id;
    let new_val = if should_copy != 0 {
        send_copy(new_value, should_copy == 2)
    } else {
        retain(new_value)
    };
    let old = if atomic {
        let _g = property_lock(slot as usize).lock().unwrap();
        let prev = *slot;
        *slot = new_val;
        prev
    } else {
        let prev = *slot;
        *slot = new_val;
        prev
    };
    release(old);
}

pub unsafe extern "C" fn objc_set_property_atomic(
    self_: Id,
    cmd: Sel,
    new_value: Id,
    offset: isize,
) {
    objc_set_property(self_, cmd, offset, new_value, true, 0);
}

pub unsafe extern "C" fn objc_set_property_nonatomic(
    self_: Id,
    cmd: Sel,
    new_value: Id,
    offset: isize,
) {
    objc_set_property(self_, cmd, offset, new_value, false, 0);
}

pub unsafe extern "C" fn objc_set_property_atomic_copy(
    self_: Id,
    cmd: Sel,
    new_value: Id,
    offset: isize,
) {
    objc_set_property(self_, cmd, offset, new_value, true, 1);
}

pub unsafe extern "C" fn objc_set_property_nonatomic_copy(
    self_: Id,
    cmd: Sel,
    new_value: Id,
    offset: isize,
) {
    objc_set_property(self_, cmd, offset, new_value, false, 1);
}

pub unsafe extern "C" fn objc_copy_struct(
    dest: *mut c_void,
    src: *const c_void,
    size: isize,
    atomic: bool,
    _has_strong: bool,
) {
    if size <= 0 || dest.is_null() || src.is_null() {
        return;
    }
    if atomic {
        let _g = property_lock(dest as usize ^ src as usize).lock().unwrap();
        memcpy(dest, src, size as usize);
    } else {
        memcpy(dest, src, size as usize);
    }
}

pub unsafe extern "C" fn objc_copy_cpp_object_atomic(
    dest: *mut c_void,
    src: *const c_void,
    copy_helper: Option<extern "C" fn(*mut c_void, *const c_void)>,
) {
    if let Some(f) = copy_helper {
        let _g = property_lock(dest as usize ^ src as usize).lock().unwrap();
        f(dest, src);
    }
}

// ---- @synchronized (recursive lock per object) -----------------------------

#[derive(Default)]
struct RecLockState {
    owner: Option<(ThreadId, usize)>,
}

struct RecLock {
    st: Mutex<RecLockState>,
    cv: Condvar,
}

impl RecLock {
    fn new() -> Self {
        Self {
            st: Mutex::new(RecLockState::default()),
            cv: Condvar::new(),
        }
    }
    fn lock(&self) {
        let me = std::thread::current().id();
        let mut g = self.st.lock().unwrap();
        loop {
            match g.owner {
                None => {
                    g.owner = Some((me, 1));
                    return;
                }
                Some((t, ref mut n)) if t == me => {
                    *n += 1;
                    return;
                }
                Some(_) => {
                    g = self.cv.wait(g).unwrap();
                }
            }
        }
    }
    fn unlock(&self) {
        let me = std::thread::current().id();
        let mut g = self.st.lock().unwrap();
        if let Some((t, ref mut n)) = g.owner {
            if t == me {
                if *n > 1 {
                    *n -= 1;
                } else {
                    g.owner = None;
                    self.cv.notify_one();
                }
            }
        }
    }
}

fn sync_table() -> &'static Mutex<HashMap<usize, Arc<RecLock>>> {
    static T: OnceLock<Mutex<HashMap<usize, Arc<RecLock>>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

pub extern "C" fn objc_sync_enter(obj: Id) -> i32 {
    if obj.is_null() {
        return 0;
    }
    let lk = {
        let mut t = sync_table().lock().unwrap();
        t.entry(obj as usize).or_insert_with(|| Arc::new(RecLock::new())).clone()
    };
    lk.lock();
    0
}

pub extern "C" fn objc_sync_exit(obj: Id) -> i32 {
    if obj.is_null() {
        return 0;
    }
    let lk = sync_table().lock().unwrap().get(&(obj as usize)).cloned();
    if let Some(lk) = lk {
        lk.unlock();
    }
    0
}

// ---- fast enumeration mutation ---------------------------------------------

pub extern "C" fn objc_enumeration_mutation(obj: Id) -> ! {
    let cls_name = if obj.is_null() {
        "nil".to_string()
    } else {
        let c = unsafe { runtime::object_class(obj) };
        runtime::class(c).map_or_else(|| "?".to_string(), |ci| ci.name.clone())
    };
    eprintln!("[pear-libobjc] collection <{cls_name} {:p}> was mutated while being enumerated", obj);
    std::process::abort();
}

// ---- associated objects ----------------------------------------------------

const OBJC_ASSOCIATION_ASSIGN: usize = 0;
const OBJC_ASSOCIATION_RETAIN_NONATOMIC: usize = 1;
const OBJC_ASSOCIATION_COPY_NONATOMIC: usize = 3;
const OBJC_ASSOCIATION_RETAIN: usize = 0o1401;
const OBJC_ASSOCIATION_COPY: usize = 0o1403;

#[derive(Clone, Copy)]
struct AssocEntry {
    value: usize,
    owned: bool,
}

fn assoc_table() -> &'static Mutex<HashMap<usize, HashMap<usize, AssocEntry>>> {
    static T: OnceLock<Mutex<HashMap<usize, HashMap<usize, AssocEntry>>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

pub unsafe extern "C" fn objc_set_associated_object(
    obj: Id,
    key: *const c_void,
    value: Id,
    policy: usize,
) {
    if obj.is_null() {
        return;
    }
    let (stored, owned) = if value.is_null() {
        (std::ptr::null_mut(), false)
    } else {
        match policy {
            OBJC_ASSOCIATION_ASSIGN => (value, false),
            OBJC_ASSOCIATION_RETAIN_NONATOMIC | OBJC_ASSOCIATION_RETAIN => (retain(value), true),
            OBJC_ASSOCIATION_COPY_NONATOMIC | OBJC_ASSOCIATION_COPY => (send_copy(value, false), true),
            _ => (retain(value), true),
        }
    };

    let old = {
        let mut t = assoc_table().lock().unwrap();
        let map = t.entry(obj as usize).or_default();
        let prev = if stored.is_null() {
            map.remove(&(key as usize))
        } else {
            map.insert(key as usize, AssocEntry { value: stored as usize, owned })
        };
        if map.is_empty() {
            t.remove(&(obj as usize));
        }
        prev
    };
    if let Some(prev) = old {
        if prev.owned {
            release(prev.value as Id);
        }
    }
}

pub unsafe extern "C" fn objc_get_associated_object(obj: Id, key: *const c_void) -> Id {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let t = assoc_table().lock().unwrap();
    t.get(&(obj as usize))
        .and_then(|m| m.get(&(key as usize)))
        .map_or(std::ptr::null_mut(), |e| e.value as Id)
}

pub unsafe extern "C" fn objc_remove_associated_objects(obj: Id) {
    if obj.is_null() {
        return;
    }
    let entries = assoc_table().lock().unwrap().remove(&(obj as usize));
    if let Some(map) = entries {
        for (_, e) in map {
            if e.owned {
                release(e.value as Id);
            }
        }
    }
}
