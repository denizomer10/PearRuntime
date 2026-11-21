//! Clean-room: public CoreFoundation and Foundation APIs (NSString,
//! NSArray, NSDictionary, NSData, NSNumber, NSBundle, NSNotificationCenter,
//! NSRunLoop, NSDate) implemented in pure Rust on top of `pear-libobjc`.
//!
//! Works identically on macOS and Linux (zero Apple framework dependencies).

use pear_libobjc::abi::*;
use pear_libobjc::object::{dispose, retain, release, EMPTY_CACHE, NSOBJECT_CLASS, NSOBJECT_META, StaticClass};
use pear_libobjc::runtime::{self, sel};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::{Mutex, Once, OnceLock};

macro_rules! foundation_class {
    ($cls:ident, $meta:ident) => {
        pub static $meta: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const NSOBJECT_META as *const ClassT,
            superclass: &raw const NSOBJECT_META as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
        pub static $cls: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const $meta as *const ClassT,
            superclass: &raw const NSOBJECT_CLASS as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
    };
}

foundation_class!(NSSTRING_CLASS, NSSTRING_META);
foundation_class!(NSMUTABLESTRING_CLASS, NSMUTABLESTRING_META);
foundation_class!(NSARRAY_CLASS, NSARRAY_META);
foundation_class!(NSMUTABLEARRAY_CLASS, NSMUTABLEARRAY_META);
foundation_class!(NSDICTIONARY_CLASS, NSDICTIONARY_META);
foundation_class!(NSMUTABLEDICTIONARY_CLASS, NSMUTABLEDICTIONARY_META);
foundation_class!(NSDATA_CLASS, NSDATA_META);
foundation_class!(NSMUTABLEDATA_CLASS, NSMUTABLEDATA_META);
foundation_class!(NSNUMBER_CLASS, NSNUMBER_META);
foundation_class!(NSBUNDLE_CLASS, NSBUNDLE_META);
foundation_class!(NSNOTIFICATIONCENTER_CLASS, NSNOTIFICATIONCENTER_META);
foundation_class!(NSRUNLOOP_CLASS, NSRUNLOOP_META);
foundation_class!(NSDATE_CLASS, NSDATE_META);

// ---- Backing store ---------------------------------------------------------

#[derive(Clone)]
enum Backing {
    Str(String),
    Arr(Vec<usize>),
    Dict(HashMap<String, usize>),
    Bytes(Vec<u8>),
    NumI64(i64),
    NumF64(f64),
    NumBool(bool),
    Date(f64),
}

fn store() -> &'static Mutex<HashMap<usize, Backing>> {
    static S: OnceLock<Mutex<HashMap<usize, Backing>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

unsafe fn alloc_with_backing(cls: *const ClassT, b: Backing) -> Id {
    let obj = pear_libobjc::object::alloc(cls);
    store().lock().unwrap().insert(obj as usize, b);
    obj
}

extern "C" fn f_dealloc(this: Id, _: Sel) {
    let removed = store().lock().unwrap().remove(&(this as usize));
    if let Some(b) = removed {
        match b {
            Backing::Arr(v) => {
                for ptr in v {
                    release(ptr as Id);
                }
            }
            Backing::Dict(m) => {
                for ptr in m.into_values() {
                    release(ptr as Id);
                }
            }
            _ => {}
        }
    }
    unsafe { dispose(this) };
}

unsafe fn send_str(obj: Id, s: &str) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    let sl = sel(s);
    let d = pear_libobjc::message::lookup_for_send(obj, sl);
    let f: extern "C" fn(Id, Sel) -> *const c_char = std::mem::transmute(d.imp);
    let p = f(d.receiver, sl);
    if p.is_null() {
        None
    } else {
        Some(CStr::from_ptr(p).to_string_lossy().into_owned())
    }
}

pub fn rust_str_from_nsstring(obj: Id) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    if let Some(Backing::Str(s)) = store().lock().unwrap().get(&(obj as usize)) {
        return Some(s.clone());
    }
    unsafe { send_str(obj, "UTF8String") }
}

pub fn nsstring_from_rust(s: &str) -> Id {
    init_foundation();
    unsafe { alloc_with_backing(NSSTRING_CLASS.ptr(), Backing::Str(s.to_string())) }
}

// ---- NSString / NSMutableString --------------------------------------------

extern "C" fn str_with_utf8(cls: Id, _: Sel, cstr: *const c_char) -> Id {
    let s = if cstr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(cstr).to_string_lossy().into_owned() }
    };
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Str(s)) }
}

extern "C" fn str_utf8(this: Id, _: Sel) -> *const c_char {
    let s = store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .and_then(|b| match b {
            Backing::Str(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let boxed = CString::new(s).unwrap_or_default().into_boxed_c_str();
    Box::leak(boxed).as_ptr()
}

extern "C" fn str_length(this: Id, _: Sel) -> usize {
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map_or(0, |b| match b {
            Backing::Str(s) => s.encode_utf16().count(),
            _ => 0,
        })
}

extern "C" fn str_is_equal(this: Id, _: Sel, other: Id) -> bool {
    if this == other {
        return true;
    }
    match (rust_str_from_nsstring(this), rust_str_from_nsstring(other)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

extern "C" fn str_append_path(this: Id, _: Sel, comp: Id) -> Id {
    let base = rust_str_from_nsstring(this).unwrap_or_default();
    let add = rust_str_from_nsstring(comp).unwrap_or_default();
    let joined = if base.is_empty() {
        add
    } else if base.ends_with('/') {
        format!("{base}{add}")
    } else {
        format!("{base}/{add}")
    };
    nsstring_from_rust(&joined)
}

extern "C" fn str_append(this: Id, _: Sel, other: Id) {
    let Some(add) = rust_str_from_nsstring(other) else {
        return;
    };
    if let Some(Backing::Str(s)) = store().lock().unwrap().get_mut(&(this as usize)) {
        s.push_str(&add);
    }
}

extern "C" fn str_copy_zone(this: Id, _: Sel, _: *mut c_void) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    nsstring_from_rust(&s)
}

// ---- NSArray / NSMutableArray ----------------------------------------------

extern "C" fn arr_new(cls: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Arr(Vec::new())) }
}

extern "C" fn arr_with_objs(cls: Id, _: Sel, objs: *const Id, count: usize) -> Id {
    let mut vec = Vec::with_capacity(count);
    if !objs.is_null() {
        for i in 0..count {
            let o = unsafe { *objs.add(i) };
            retain(o);
            vec.push(o as usize);
        }
    }
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Arr(vec)) }
}

extern "C" fn arr_count(this: Id, _: Sel) -> usize {
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map_or(0, |b| match b {
            Backing::Arr(v) => v.len(),
            _ => 0,
        })
}

extern "C" fn arr_at(this: Id, _: Sel, idx: usize) -> Id {
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .and_then(|b| match b {
            Backing::Arr(v) => v.get(idx).map(|&p| p as Id),
            _ => None,
        })
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn arr_add(this: Id, _: Sel, obj: Id) {
    if obj.is_null() {
        return;
    }
    retain(obj);
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        v.push(obj as usize);
    }
}

// ---- NSDictionary / NSMutableDictionary ------------------------------------

extern "C" fn dict_new(cls: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(HashMap::new())) }
}

extern "C" fn dict_count(this: Id, _: Sel) -> usize {
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map_or(0, |b| match b {
            Backing::Dict(m) => m.len(),
            _ => 0,
        })
}

extern "C" fn dict_get(this: Id, _: Sel, key: Id) -> Id {
    let Some(k) = rust_str_from_nsstring(key) else {
        return std::ptr::null_mut();
    };
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .and_then(|b| match b {
            Backing::Dict(m) => m.get(&k).map(|&p| p as Id),
            _ => None,
        })
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn dict_set(this: Id, _: Sel, val: Id, key: Id) {
    let Some(k) = rust_str_from_nsstring(key) else {
        return;
    };
    let old = {
        let mut st = store().lock().unwrap();
        let Some(Backing::Dict(m)) = st.get_mut(&(this as usize)) else {
            return;
        };
        if val.is_null() {
            m.remove(&k)
        } else {
            retain(val);
            m.insert(k, val as usize)
        }
    };
    if let Some(prev) = old {
        release(prev as Id);
    }
}

// ---- NSData / NSMutableData ------------------------------------------------

extern "C" fn data_with_bytes(cls: Id, _: Sel, bytes: *const u8, len: usize) -> Id {
    let v = if bytes.is_null() || len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(bytes, len).to_vec() }
    };
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Bytes(v)) }
}

extern "C" fn data_new(cls: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Bytes(Vec::new())) }
}

extern "C" fn data_length(this: Id, _: Sel) -> usize {
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map_or(0, |b| match b {
            Backing::Bytes(v) => v.len(),
            _ => 0,
        })
}

extern "C" fn data_bytes(this: Id, _: Sel) -> *const c_void {
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .and_then(|b| match b {
            Backing::Bytes(v) => Some(v.as_ptr() as *const c_void),
            _ => None,
        })
        .unwrap_or(std::ptr::null())
}

extern "C" fn data_append(this: Id, _: Sel, bytes: *const u8, len: usize) {
    if bytes.is_null() || len == 0 {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(bytes, len) };
    if let Some(Backing::Bytes(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        v.extend_from_slice(slice);
    }
}

// ---- NSNumber --------------------------------------------------------------

extern "C" fn num_i32(cls: Id, _: Sel, v: i32) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::NumI64(v as i64)) }
}
extern "C" fn num_i64(cls: Id, _: Sel, v: i64) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::NumI64(v)) }
}
extern "C" fn num_f64(cls: Id, _: Sel, v: f64) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::NumF64(v)) }
}
extern "C" fn num_bool(cls: Id, _: Sel, v: bool) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::NumBool(v)) }
}
extern "C" fn num_as_i32(this: Id, _: Sel) -> i32 {
    num_as_i64(this, sel("longLongValue")) as i32
}
extern "C" fn num_as_i64(this: Id, _: Sel) -> i64 {
    match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::NumI64(v)) => *v,
        Some(Backing::NumF64(v)) => *v as i64,
        Some(Backing::NumBool(b)) => *b as i64,
        _ => 0,
    }
}
extern "C" fn num_as_f64(this: Id, _: Sel) -> f64 {
    match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::NumI64(v)) => *v as f64,
        Some(Backing::NumF64(v)) => *v,
        Some(Backing::NumBool(b)) => if *b { 1.0 } else { 0.0 },
        _ => 0.0,
    }
}
extern "C" fn num_as_bool(this: Id, _: Sel) -> bool {
    num_as_i64(this, sel("longLongValue")) != 0
}

// ---- NSBundle / NSNotificationCenter / NSRunLoop / NSDate ------------------

static MAIN_BUNDLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static DEFAULT_NC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MAIN_RL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

extern "C" fn bundle_main(_cls: Id, _: Sel) -> Id {
    let cur = MAIN_BUNDLE.load(std::sync::atomic::Ordering::Acquire);
    if cur != 0 {
        return cur as Id;
    }
    let b = unsafe { pear_libobjc::object::alloc(NSBUNDLE_CLASS.ptr()) };
    MAIN_BUNDLE.store(b as usize, std::sync::atomic::Ordering::Release);
    b
}
extern "C" fn bundle_path(_this: Id, _: Sel) -> Id {
    let cwd = std::env::current_dir().unwrap_or_default();
    nsstring_from_rust(&cwd.to_string_lossy())
}
extern "C" fn bundle_info(_this: Id, _: Sel) -> Id {
    dict_new(NSDICTIONARY_CLASS.ptr() as Id, sel("dictionary"))
}

struct Observer {
    target: usize,
    selector: usize,
    name: Option<String>,
}
fn observers() -> &'static Mutex<Vec<Observer>> {
    static O: OnceLock<Mutex<Vec<Observer>>> = OnceLock::new();
    O.get_or_init(Default::default)
}

extern "C" fn nc_default(_cls: Id, _: Sel) -> Id {
    let cur = DEFAULT_NC.load(std::sync::atomic::Ordering::Acquire);
    if cur != 0 {
        return cur as Id;
    }
    let n = unsafe { pear_libobjc::object::alloc(NSNOTIFICATIONCENTER_CLASS.ptr()) };
    DEFAULT_NC.store(n as usize, std::sync::atomic::Ordering::Release);
    n
}

extern "C" fn nc_add_obs(_this: Id, _: Sel, target: Id, selector: Sel, name: Id, _obj: Id) {
    if target.is_null() || selector.is_null() {
        return;
    }
    observers().lock().unwrap().push(Observer {
        target: target as usize,
        selector: selector as usize,
        name: rust_str_from_nsstring(name),
    });
}

extern "C" fn nc_post(_this: Id, _: Sel, name: Id, obj: Id, _info: Id) {
    let n = rust_str_from_nsstring(name);
    let targets: Vec<(usize, usize)> = observers()
        .lock()
        .unwrap()
        .iter()
        .filter(|o| o.name.is_none() || o.name == n)
        .map(|o| (o.target, o.selector))
        .collect();
    for (t, s) in targets {
        unsafe {
            let sl = s as Sel;
            let d = pear_libobjc::message::lookup_for_send(t as Id, sl);
            let f: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d.imp);
            f(d.receiver, sl, obj);
        }
    }
}

extern "C" fn rl_main(_cls: Id, _: Sel) -> Id {
    let cur = MAIN_RL.load(std::sync::atomic::Ordering::Acquire);
    if cur != 0 {
        return cur as Id;
    }
    let r = unsafe { pear_libobjc::object::alloc(NSRUNLOOP_CLASS.ptr()) };
    MAIN_RL.store(r as usize, std::sync::atomic::Ordering::Release);
    r
}
extern "C" fn rl_run_mode(_this: Id, _: Sel, _mode: Id, _date: Id) -> bool {
    false
}

extern "C" fn date_now(cls: Id, _: Sel) -> Id {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Date(secs)) }
}

// ---- CoreFoundation C API --------------------------------------------------

pub extern "C" fn cf_retain(cf: *const c_void) -> *const c_void {
    retain(cf as Id) as *const c_void
}
pub extern "C" fn cf_release(cf: *const c_void) {
    release(cf as Id);
}
pub extern "C" fn cf_string_create_with_cstring(
    _alloc: *const c_void,
    c_str: *const c_char,
    _encoding: u32,
) -> *const c_void {
    str_with_utf8(NSSTRING_CLASS.ptr() as Id, sel("stringWithUTF8String:"), c_str) as *const c_void
}
pub extern "C" fn cf_string_get_cstring_ptr(the_str: *const c_void, _encoding: u32) -> *const c_char {
    if the_str.is_null() {
        return std::ptr::null();
    }
    str_utf8(the_str as Id, sel("UTF8String"))
}

// ---- Registration & Symbol Table -------------------------------------------

pub fn init_foundation() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        pear_libobjc::object::init_runtime();
        let i = |f: *const ()| f as Imp;
        runtime::register_builtin(
            NSSTRING_CLASS.ptr(),
            NSSTRING_META.ptr(),
            "NSString",
            16,
            &[
                ("UTF8String", i(str_utf8 as *const ())),
                ("length", i(str_length as *const ())),
                ("isEqualToString:", i(str_is_equal as *const ())),
                ("stringByAppendingPathComponent:", i(str_append_path as *const ())),
                ("copyWithZone:", i(str_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("stringWithUTF8String:", i(str_with_utf8 as *const ())),
            ],
        );
        runtime::register_builtin(
            NSMUTABLESTRING_CLASS.ptr(),
            NSMUTABLESTRING_META.ptr(),
            "NSMutableString",
            16,
            &[
                ("UTF8String", i(str_utf8 as *const ())),
                ("length", i(str_length as *const ())),
                ("appendString:", i(str_append as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("stringWithUTF8String:", i(str_with_utf8 as *const ())),
            ],
        );
        runtime::register_builtin(
            NSARRAY_CLASS.ptr(),
            NSARRAY_META.ptr(),
            "NSArray",
            16,
            &[
                ("count", i(arr_count as *const ())),
                ("objectAtIndex:", i(arr_at as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("array", i(arr_new as *const ())),
                ("arrayWithObjects:count:", i(arr_with_objs as *const ())),
            ],
        );
        runtime::register_builtin(
            NSMUTABLEARRAY_CLASS.ptr(),
            NSMUTABLEARRAY_META.ptr(),
            "NSMutableArray",
            16,
            &[
                ("count", i(arr_count as *const ())),
                ("objectAtIndex:", i(arr_at as *const ())),
                ("addObject:", i(arr_add as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("array", i(arr_new as *const ()))],
        );
        runtime::register_builtin(
            NSDICTIONARY_CLASS.ptr(),
            NSDICTIONARY_META.ptr(),
            "NSDictionary",
            16,
            &[
                ("count", i(dict_count as *const ())),
                ("objectForKey:", i(dict_get as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("dictionary", i(dict_new as *const ()))],
        );
        runtime::register_builtin(
            NSMUTABLEDICTIONARY_CLASS.ptr(),
            NSMUTABLEDICTIONARY_META.ptr(),
            "NSMutableDictionary",
            16,
            &[
                ("count", i(dict_count as *const ())),
                ("objectForKey:", i(dict_get as *const ())),
                ("setObject:forKey:", i(dict_set as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("dictionary", i(dict_new as *const ()))],
        );
        runtime::register_builtin(
            NSDATA_CLASS.ptr(),
            NSDATA_META.ptr(),
            "NSData",
            16,
            &[
                ("length", i(data_length as *const ())),
                ("bytes", i(data_bytes as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("data", i(data_new as *const ())),
                ("dataWithBytes:length:", i(data_with_bytes as *const ())),
            ],
        );
        runtime::register_builtin(
            NSMUTABLEDATA_CLASS.ptr(),
            NSMUTABLEDATA_META.ptr(),
            "NSMutableData",
            16,
            &[
                ("length", i(data_length as *const ())),
                ("bytes", i(data_bytes as *const ())),
                ("appendBytes:length:", i(data_append as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("data", i(data_new as *const ()))],
        );
        runtime::register_builtin(
            NSNUMBER_CLASS.ptr(),
            NSNUMBER_META.ptr(),
            "NSNumber",
            16,
            &[
                ("intValue", i(num_as_i32 as *const ())),
                ("integerValue", i(num_as_i64 as *const ())),
                ("longLongValue", i(num_as_i64 as *const ())),
                ("doubleValue", i(num_as_f64 as *const ())),
                ("boolValue", i(num_as_bool as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("numberWithInt:", i(num_i32 as *const ())),
                ("numberWithInteger:", i(num_i64 as *const ())),
                ("numberWithLongLong:", i(num_i64 as *const ())),
                ("numberWithDouble:", i(num_f64 as *const ())),
                ("numberWithBool:", i(num_bool as *const ())),
            ],
        );
        runtime::register_builtin(
            NSBUNDLE_CLASS.ptr(),
            NSBUNDLE_META.ptr(),
            "NSBundle",
            16,
            &[
                ("bundlePath", i(bundle_path as *const ())),
                ("resourcePath", i(bundle_path as *const ())),
                ("infoDictionary", i(bundle_info as *const ())),
            ],
            &[("mainBundle", i(bundle_main as *const ()))],
        );
        runtime::register_builtin(
            NSNOTIFICATIONCENTER_CLASS.ptr(),
            NSNOTIFICATIONCENTER_META.ptr(),
            "NSNotificationCenter",
            16,
            &[
                ("addObserver:selector:name:object:", i(nc_add_obs as *const ())),
                ("postNotificationName:object:userInfo:", i(nc_post as *const ())),
            ],
            &[("defaultCenter", i(nc_default as *const ()))],
        );
        runtime::register_builtin(
            NSRUNLOOP_CLASS.ptr(),
            NSRUNLOOP_META.ptr(),
            "NSRunLoop",
            16,
            &[("runMode:beforeDate:", i(rl_run_mode as *const ()))],
            &[
                ("mainRunLoop", i(rl_main as *const ())),
                ("currentRunLoop", i(rl_main as *const ())),
            ],
        );
        runtime::register_builtin(
            NSDATE_CLASS.ptr(),
            NSDATE_META.ptr(),
            "NSDate",
            16,
            &[("dealloc", i(f_dealloc as *const ()))],
            &[("date", i(date_now as *const ()))],
        );
    });
}

pub fn symbol(name: &str) -> Option<usize> {
    init_foundation();
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_OBJC_CLASS_$_NSString" => Some(NSSTRING_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMutableString" => Some(NSMUTABLESTRING_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSArray" => Some(NSARRAY_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMutableArray" => Some(NSMUTABLEARRAY_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSDictionary" => Some(NSDICTIONARY_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMutableDictionary" => Some(NSMUTABLEDICTIONARY_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSData" => Some(NSDATA_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMutableData" => Some(NSMUTABLEDATA_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSNumber" => Some(NSNUMBER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSBundle" => Some(NSBUNDLE_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSNotificationCenter" => Some(NSNOTIFICATIONCENTER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSRunLoop" => Some(NSRUNLOOP_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSDate" => Some(NSDATE_CLASS.ptr() as usize),
        "_CFRetain" => f(cf_retain as *const ()),
        "_CFRelease" => f(cf_release as *const ()),
        "_CFStringCreateWithCString" => f(cf_string_create_with_cstring as *const ()),
        "_CFStringGetCStringPtr" => f(cf_string_get_cstring_ptr as *const ()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pear_libobjc::message::lookup_for_send;

    unsafe fn send1<A1, R>(obj: Id, name: &str, a1: A1) -> R {
        let s = sel(name);
        let d = lookup_for_send(obj, s);
        let f: extern "C" fn(Id, Sel, A1) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s, a1)
    }

    unsafe fn send0<R>(obj: Id, name: &str) -> R {
        let s = sel(name);
        let d = lookup_for_send(obj, s);
        let f: extern "C" fn(Id, Sel) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s)
    }

    #[test]
    fn foundation_strings_arrays_dicts_and_numbers() {
        init_foundation();
        unsafe {
            let s1: Id = send1(NSSTRING_CLASS.ptr() as Id, "stringWithUTF8String:", c"hello".as_ptr());
            let s2: Id = send1(NSSTRING_CLASS.ptr() as Id, "stringWithUTF8String:", c"world".as_ptr());
            let joined: Id = send1(s1, "stringByAppendingPathComponent:", s2);
            assert_eq!(rust_str_from_nsstring(joined).as_deref(), Some("hello/world"));

            let arr: Id = send0(NSMUTABLEARRAY_CLASS.ptr() as Id, "array");
            let _: () = send1(arr, "addObject:", joined);
            let n: usize = send0(arr, "count");
            assert_eq!(n, 1);

            let num: Id = send1(NSNUMBER_CLASS.ptr() as Id, "numberWithLongLong:", 42i64);
            let v: i64 = send0(num, "longLongValue");
            assert_eq!(v, 42);
        }
    }
}
