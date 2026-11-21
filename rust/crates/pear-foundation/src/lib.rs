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
foundation_class!(NSERROR_CLASS, NSERROR_META);
foundation_class!(NSFILEMANAGER_CLASS, NSFILEMANAGER_META);
foundation_class!(NSURL_CLASS, NSURL_META);
foundation_class!(NSSET_CLASS, NSSET_META);
foundation_class!(NSMUTABLESET_CLASS, NSMUTABLESET_META);
foundation_class!(NSUSERDEFAULTS_CLASS, NSUSERDEFAULTS_META);
foundation_class!(NSJSONSERIALIZATION_CLASS, NSJSONSERIALIZATION_META);
foundation_class!(NSCHARACTERSET_CLASS, NSCHARACTERSET_META);
foundation_class!(NSLOCALE_CLASS, NSLOCALE_META);
foundation_class!(NSURLREQUEST_CLASS, NSURLREQUEST_META);
foundation_class!(NSMUTABLEURLREQUEST_CLASS, NSMUTABLEURLREQUEST_META);
foundation_class!(NSUUID_CLASS, NSUUID_META);
foundation_class!(NSPROCESSINFO_CLASS, NSPROCESSINFO_META);
foundation_class!(NSTIMER_CLASS, NSTIMER_META);

static K_CF_ALLOCATOR_DEFAULT: usize = 0;

#[repr(C)]
struct MachOCFString {
    _isa: *const c_void,
    info: usize,
    data: *const u8,
    length: usize,
}

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
    #[allow(dead_code)]
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
    unsafe {
        if !is_tagged_pointer(obj) && *(obj as *const *const ClassT) == NSSTRING_CLASS.ptr() {
            let cf = &*(obj as *const MachOCFString);
            if !cf.data.is_null() {
                if (cf.info & 0x10) == 0 {
                    let bytes = std::slice::from_raw_parts(cf.data, cf.length);
                    return Some(String::from_utf8_lossy(bytes).into_owned());
                } else {
                    let units = std::slice::from_raw_parts(cf.data as *const u16, cf.length);
                    return Some(String::from_utf16_lossy(units));
                }
            }
        }
        send_str(obj, "UTF8String")
    }
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
        runtime::register_builtin(
            NSERROR_CLASS.ptr(),
            NSERROR_META.ptr(),
            "NSError",
            16,
            &[("dealloc", i(f_dealloc as *const ()))],
            &[],
        );
        runtime::register_builtin(
            NSFILEMANAGER_CLASS.ptr(),
            NSFILEMANAGER_META.ptr(),
            "NSFileManager",
            16,
            &[],
            &[("defaultManager", i(fm_default as *const ()))],
        );
        runtime::register_builtin(
            NSURL_CLASS.ptr(),
            NSURL_META.ptr(),
            "NSURL",
            16,
            &[("path", i(url_path as *const ())), ("dealloc", i(f_dealloc as *const ()))],
            &[("fileURLWithPath:", i(url_file_with_path as *const ()))],
        );
        runtime::register_builtin(
            NSSET_CLASS.ptr(),
            NSSET_META.ptr(),
            "NSSet",
            16,
            &[("count", i(arr_count as *const ())), ("dealloc", i(f_dealloc as *const ()))],
            &[("set", i(set_empty as *const ()))],
        );
        runtime::register_builtin(
            NSMUTABLESET_CLASS.ptr(),
            NSMUTABLESET_META.ptr(),
            "NSMutableSet",
            16,
            &[
                ("count", i(arr_count as *const ())),
                ("addObject:", i(arr_add as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("set", i(mset_empty as *const ()))],
        );
        runtime::register_builtin(
            NSUSERDEFAULTS_CLASS.ptr(),
            NSUSERDEFAULTS_META.ptr(),
            "NSUserDefaults",
            16,
            &[
                ("objectForKey:", i(ud_obj_for_key as *const ())),
                ("setObject:forKey:", i(ud_set_obj_for_key as *const ())),
            ],
            &[("standardUserDefaults", i(ud_standard as *const ()))],
        );
        runtime::register_builtin(
            NSJSONSERIALIZATION_CLASS.ptr(),
            NSJSONSERIALIZATION_META.ptr(),
            "NSJSONSerialization",
            16,
            &[],
            &[
                ("JSONObjectWithData:options:error:", i(json_decode as *const ())),
                ("dataWithJSONObject:options:error:", i(json_encode as *const ())),
            ],
        );
        runtime::register_builtin(
            NSCHARACTERSET_CLASS.ptr(),
            NSCHARACTERSET_META.ptr(),
            "NSCharacterSet",
            16,
            &[("dealloc", i(f_dealloc as *const ()))],
            &[
                ("whitespaceCharacterSet", i(cs_ws as *const ())),
                ("whitespaceAndNewlineCharacterSet", i(cs_ws_nl as *const ())),
                ("decimalDigitCharacterSet", i(cs_digits as *const ())),
                ("alphanumericCharacterSet", i(cs_alnum as *const ())),
            ],
        );
        runtime::register_builtin(
            NSLOCALE_CLASS.ptr(),
            NSLOCALE_META.ptr(),
            "NSLocale",
            16,
            &[("localeIdentifier", i(locale_id as *const ())), ("dealloc", i(f_dealloc as *const ()))],
            &[
                ("currentLocale", i(locale_current as *const ())),
                ("preferredLanguages", i(locale_langs as *const ())),
            ],
        );
        runtime::register_builtin(
            NSURLREQUEST_CLASS.ptr(),
            NSURLREQUEST_META.ptr(),
            "NSURLRequest",
            16,
            &[
                ("URL", i(req_url as *const ())),
                ("HTTPMethod", i(req_method as *const ())),
                ("HTTPBody", i(req_body as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("requestWithURL:", i(req_with_url as *const ()))],
        );
        runtime::register_builtin(
            NSMUTABLEURLREQUEST_CLASS.ptr(),
            NSMUTABLEURLREQUEST_META.ptr(),
            "NSMutableURLRequest",
            16,
            &[
                ("URL", i(req_url as *const ())),
                ("HTTPMethod", i(req_method as *const ())),
                ("setHTTPMethod:", i(req_set_method as *const ())),
                ("setValue:forHTTPHeaderField:", i(req_set_header as *const ())),
                ("HTTPBody", i(req_body as *const ())),
                ("setHTTPBody:", i(req_set_body as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("requestWithURL:", i(mreq_with_url as *const ()))],
        );
        runtime::register_builtin(
            NSUUID_CLASS.ptr(),
            NSUUID_META.ptr(),
            "NSUUID",
            16,
            &[("UUIDString", i(uuid_string as *const ())), ("dealloc", i(f_dealloc as *const ()))],
            &[("UUID", i(uuid_new as *const ()))],
        );
        runtime::register_builtin(
            NSPROCESSINFO_CLASS.ptr(),
            NSPROCESSINFO_META.ptr(),
            "NSProcessInfo",
            16,
            &[
                ("operatingSystemVersionString", i(pi_os_ver as *const ())),
                ("processorCount", i(pi_cpus as *const ())),
                ("physicalMemory", i(pi_mem as *const ())),
            ],
            &[("processInfo", i(pi_shared as *const ()))],
        );
        runtime::register_builtin(
            NSTIMER_CLASS.ptr(),
            NSTIMER_META.ptr(),
            "NSTimer",
            16,
            &[("invalidate", i(timer_invalidate as *const ())), ("dealloc", i(f_dealloc as *const ()))],
            &[("scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:", i(timer_sched as *const ()))],
        );
    });
}

extern "C" fn timer_sched(
    _self: Id,
    _cmd: Sel,
    _ti: f64,
    _target: Id,
    _sel: Sel,
    _user_info: Id,
    _repeats: bool,
) -> Id {
    unsafe { alloc_with_backing(NSTIMER_CLASS.ptr(), Backing::Arr(Vec::new())) }
}
extern "C" fn timer_invalidate(_self: Id, _cmd: Sel) {}

extern "C" fn fm_default(_self: Id, _cmd: Sel) -> Id {
    static S: OnceLock<usize> = OnceLock::new();
    *S.get_or_init(|| unsafe { alloc_with_backing(NSFILEMANAGER_CLASS.ptr(), Backing::Arr(Vec::new())) } as usize) as Id
}

extern "C" fn url_file_with_path(_self: Id, _cmd: Sel, path: Id) -> Id {
    let s = rust_str_from_nsstring(path).unwrap_or_default();
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(s)) }
}

extern "C" fn url_path(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    nsstring_from_rust(&s)
}

extern "C" fn set_empty(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSSET_CLASS.ptr(), Backing::Arr(Vec::new())) }
}

extern "C" fn mset_empty(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSMUTABLESET_CLASS.ptr(), Backing::Arr(Vec::new())) }
}

extern "C" fn ud_standard(_self: Id, _cmd: Sel) -> Id {
    static S: OnceLock<usize> = OnceLock::new();
    *S.get_or_init(|| unsafe { alloc_with_backing(NSUSERDEFAULTS_CLASS.ptr(), Backing::Dict(HashMap::new())) } as usize) as Id
}

extern "C" fn ud_obj_for_key(self_: Id, _cmd: Sel, key: Id) -> Id {
    let k = rust_str_from_nsstring(key).unwrap_or_default();
    let guard = store().lock().unwrap();
    if let Some(Backing::Dict(d)) = guard.get(&(self_ as usize)) {
        return d.get(&k).copied().unwrap_or(0) as Id;
    }
    std::ptr::null_mut()
}

extern "C" fn ud_set_obj_for_key(self_: Id, _cmd: Sel, val: Id, key: Id) {
    let k = rust_str_from_nsstring(key).unwrap_or_default();
    let mut guard = store().lock().unwrap();
    if let Some(Backing::Dict(d)) = guard.get_mut(&(self_ as usize)) {
        if val.is_null() {
            d.remove(&k);
        } else {
            d.insert(k, retain(val) as usize);
        }
    }
}

pub extern "C" fn ns_home_directory() -> Id {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    nsstring_from_rust(&home)
}

pub extern "C" fn ns_temporary_directory() -> Id {
    nsstring_from_rust(&std::env::temp_dir().to_string_lossy())
}

pub extern "C" fn ns_search_path_for_directories_in_domains(
    _dir: usize,
    _domain_mask: usize,
    _expand_tilde: bool,
) -> Id {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let s = nsstring_from_rust(&home);
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec![retain(s) as usize])) }
}

pub unsafe extern "C" fn ns_string_from_class(cls: *const ClassT) -> Id {
    if cls.is_null() {
        return nsstring_from_rust("");
    }
    let name = CStr::from_ptr(pear_libobjc::reflection::class_get_name(cls)).to_string_lossy();
    nsstring_from_rust(&name)
}

pub unsafe extern "C" fn ns_string_from_selector(s: Sel) -> Id {
    if s.is_null() {
        return nsstring_from_rust("");
    }
    let name = CStr::from_ptr(s).to_string_lossy();
    nsstring_from_rust(&name)
}

pub unsafe extern "C" fn ns_log(fmt: Id) {
    if let Some(s) = rust_str_from_nsstring(fmt) {
        eprintln!("[NSLog] {s}");
    }
}

fn ns_localized_description_key_ptr() -> *const () {
    static K: OnceLock<usize> = OnceLock::new();
    let p = *K.get_or_init(|| {
        init_foundation();
        nsstring_from_rust("NSLocalizedDescription") as usize
    });
    p as *const ()
}

pub fn symbol(name: &str) -> Option<usize> {
    init_foundation();
    let f = |p: *const ()| Some(p as usize);
    match name {
        "___CFConstantStringClassReference" => Some(NSSTRING_CLASS.ptr() as usize),
        "_kCFAllocatorDefault" => Some(&raw const K_CF_ALLOCATOR_DEFAULT as usize),
        "_NSLocalizedDescriptionKey" => Some(ns_localized_description_key_ptr() as usize),
        "_NSHomeDirectory" => f(ns_home_directory as *const ()),
        "_NSTemporaryDirectory" => f(ns_temporary_directory as *const ()),
        "_NSSearchPathForDirectoriesInDomains" => f(ns_search_path_for_directories_in_domains as *const ()),
        "_NSStringFromClass" => f(ns_string_from_class as *const ()),
        "_NSStringFromSelector" => f(ns_string_from_selector as *const ()),
        "_NSLog" => f(ns_log as *const ()),
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
        "_OBJC_CLASS_$_NSError" => Some(NSERROR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSFileManager" => Some(NSFILEMANAGER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSURL" => Some(NSURL_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSSet" => Some(NSSET_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMutableSet" => Some(NSMUTABLESET_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSUserDefaults" => Some(NSUSERDEFAULTS_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSJSONSerialization" => Some(NSJSONSERIALIZATION_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSCharacterSet" => Some(NSCHARACTERSET_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSLocale" => Some(NSLOCALE_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSURLRequest" => Some(NSURLREQUEST_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMutableURLRequest" => Some(NSMUTABLEURLREQUEST_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSUUID" => Some(NSUUID_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSProcessInfo" => Some(NSPROCESSINFO_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSTimer" => Some(NSTIMER_CLASS.ptr() as usize),
        "___NSDictionary0__" => Some(empty_dict_singleton() as usize),
        "_kCFBooleanTrue" => Some(cf_bool_true_ptr() as usize),
        "_kCFBooleanFalse" => Some(cf_bool_false_ptr() as usize),
        "_NSDefaultRunLoopMode" => Some(const_nsstr("kCFRunLoopDefaultMode") as usize),
        "_NSLocaleCountryCode" => Some(const_nsstr("NSLocaleCountryCode") as usize),
        "_NSURLContentModificationDateKey" => Some(const_nsstr("NSURLContentModificationDateKey") as usize),
        "_CFRetain" => f(cf_retain as *const ()),
        "_CFRelease" => f(cf_release as *const ()),
        "_CFStringCreateWithCString" => f(cf_string_create_with_cstring as *const ()),
        "_CFStringGetCStringPtr" => f(cf_string_get_cstring_ptr as *const ()),
        "_CFStringGetCString" => f(cf_string_get_cstring as *const ()),
        "_CFStringGetLength" => f(cf_string_get_length as *const ()),
        "_CFStringGetMaximumSizeForEncoding" => f(cf_string_get_max_size as *const ()),
        "_CFStringCreateWithFormat" => f(cf_string_create_with_format as *const ()),
        "_CFRunLoopGetMain" => f(cf_runloop_get_main as *const ()),
        "_CFReadStreamOpen" => f(cf_read_stream_open as *const ()),
        "_CFReadStreamClose" => f(cf_read_stream_close as *const ()),
        "_CFReadStreamGetStatus" => f(cf_read_stream_get_status as *const ()),
        "_CFReadStreamHasBytesAvailable" => f(cf_read_stream_has_bytes as *const ()),
        "_CFReadStreamRead" => f(cf_read_stream_read as *const ()),
        "_CFReadStreamCopyProperty" => f(cf_read_stream_copy_prop as *const ()),
        "_CFReadStreamSetProperty" => f(cf_read_stream_set_prop as *const ()),
        "_CFReadStreamScheduleWithRunLoop" => f(cf_read_stream_sched as *const ()),
        "_CFURLCreateWithBytes" => f(cf_url_create_with_bytes as *const ()),
        "_CFURLCopyUserName" => f(cf_url_copy_null as *const ()),
        "_CFURLCopyPassword" => f(cf_url_copy_null as *const ()),
        "_CFDataCreate" => f(cf_data_create as *const ()),
        "_CFDataGetBytePtr" => f(cf_data_get_byte_ptr as *const ()),
        "_CFDataGetLength" => f(cf_data_get_length as *const ()),
        "_CFArrayGetCount" => f(cf_array_get_count as *const ()),
        "_CFArrayGetValueAtIndex" => f(cf_array_get_value_at_index as *const ()),
        "_CFDictionaryCreate" => f(cf_dictionary_create as *const ()),
        "_CFDictionaryCreateMutable" => f(cf_dictionary_create_mutable as *const ()),
        "_CFDictionarySetValue" => f(cf_dictionary_set_value as *const ()),
        "_CFDictionaryGetValue" => f(cf_dictionary_get_value as *const ()),
        "_CFNumberCreate" => f(cf_number_create as *const ()),
        "_CFNumberGetValue" => f(cf_number_get_value as *const ()),
        _ => None,
    }
}

// ---- JSON / CharacterSet / Locale / URLRequest / UUID / ProcessInfo / CF ----

fn json_skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && b[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn json_parse_str(b: &[u8], i: &mut usize) -> Option<String> {
    if b.get(*i) != Some(&b'"') { return None; }
    *i += 1;
    let mut out = String::new();
    while *i < b.len() {
        let c = b[*i];
        *i += 1;
        if c == b'"' { return Some(out); }
        if c == b'\\' && *i < b.len() {
            let esc = b[*i];
            *i += 1;
            match esc {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                _ => out.push(esc as char),
            }
        } else {
            out.push(c as char);
        }
    }
    None
}

unsafe fn json_parse_val(b: &[u8], i: &mut usize) -> Option<Id> {
    json_skip_ws(b, i);
    let c = *b.get(*i)?;
    match c {
        b'"' => json_parse_str(b, i).map(|s| nsstring_from_rust(&s)),
        b'{' => {
            *i += 1;
            let mut map = HashMap::new();
            json_skip_ws(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Some(alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map)));
            }
            loop {
                json_skip_ws(b, i);
                let k = json_parse_str(b, i)?;
                json_skip_ws(b, i);
                if b.get(*i) != Some(&b':') { return None; }
                *i += 1;
                let v = json_parse_val(b, i)?;
                map.insert(k, retain(v) as usize);
                json_skip_ws(b, i);
                match b.get(*i) {
                    Some(&b',') => *i += 1,
                    Some(&b'}') => { *i += 1; break; }
                    _ => return None,
                }
            }
            Some(alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map)))
        }
        b'[' => {
            *i += 1;
            let mut vec = Vec::new();
            json_skip_ws(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Some(alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec)));
            }
            loop {
                let v = json_parse_val(b, i)?;
                vec.push(retain(v) as usize);
                json_skip_ws(b, i);
                match b.get(*i) {
                    Some(&b',') => *i += 1,
                    Some(&b']') => { *i += 1; break; }
                    _ => return None,
                }
            }
            Some(alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec)))
        }
        b't' if b[*i..].starts_with(b"true") => {
            *i += 4;
            Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(true)))
        }
        b'f' if b[*i..].starts_with(b"false") => {
            *i += 5;
            Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(false)))
        }
        b'n' if b[*i..].starts_with(b"null") => {
            *i += 4;
            Some(std::ptr::null_mut())
        }
        b'-' | b'0'..=b'9' => {
            let start = *i;
            while *i < b.len() && (b[*i].is_ascii_digit() || matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E')) {
                *i += 1;
            }
            let s = std::str::from_utf8(&b[start..*i]).ok()?;
            if s.contains('.') || s.contains('e') || s.contains('E') {
                let v: f64 = s.parse().ok()?;
                Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumF64(v)))
            } else {
                let v: i64 = s.parse().ok()?;
                Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumI64(v)))
            }
        }
        _ => None,
    }
}

fn json_serialize_val(obj: Id, out: &mut String) {
    if obj.is_null() {
        out.push_str("null");
        return;
    }
    let backing = store().lock().unwrap().get(&(obj as usize)).cloned();
    match backing {
        Some(Backing::Str(s)) => {
            out.push('"');
            for ch in s.chars() {
                match ch {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    _ => out.push(ch),
                }
            }
            out.push('"');
        }
        Some(Backing::NumBool(b)) => out.push_str(if b { "true" } else { "false" }),
        Some(Backing::NumI64(n)) => out.push_str(&n.to_string()),
        Some(Backing::NumF64(n)) => out.push_str(&n.to_string()),
        Some(Backing::Arr(arr)) => {
            out.push('[');
            for (idx, p) in arr.into_iter().enumerate() {
                if idx > 0 { out.push(','); }
                json_serialize_val(p as Id, out);
            }
            out.push(']');
        }
        Some(Backing::Dict(d)) => {
            out.push('{');
            for (idx, (k, p)) in d.into_iter().enumerate() {
                if idx > 0 { out.push(','); }
                out.push('"');
                out.push_str(&k);
                out.push_str("\":");
                json_serialize_val(p as Id, out);
            }
            out.push('}');
        }
        _ => out.push_str("null"),
    }
}

extern "C" fn json_decode(_self: Id, _cmd: Sel, data: Id, _opts: usize, _err: *mut Id) -> Id {
    let bytes = match store().lock().unwrap().get(&(data as usize)).cloned() {
        Some(Backing::Bytes(b)) => b,
        _ => return std::ptr::null_mut(),
    };
    let mut i = 0;
    unsafe { json_parse_val(&bytes, &mut i) }.unwrap_or(std::ptr::null_mut())
}

extern "C" fn json_encode(_self: Id, _cmd: Sel, obj: Id, _opts: usize, _err: *mut Id) -> Id {
    let mut out = String::new();
    json_serialize_val(obj, &mut out);
    unsafe { alloc_with_backing(NSDATA_CLASS.ptr(), Backing::Bytes(out.into_bytes())) }
}

extern "C" fn cs_ws(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSCHARACTERSET_CLASS.ptr(), Backing::Str(" \t".into())) }
}
extern "C" fn cs_ws_nl(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSCHARACTERSET_CLASS.ptr(), Backing::Str(" \t\r\n".into())) }
}
extern "C" fn cs_digits(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSCHARACTERSET_CLASS.ptr(), Backing::Str("0123456789".into())) }
}
extern "C" fn cs_alnum(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSCHARACTERSET_CLASS.ptr(), Backing::Str("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".into())) }
}

extern "C" fn locale_current(_self: Id, _cmd: Sel) -> Id {
    let id = std::env::var("LANG").unwrap_or_else(|_| "en_US".to_string());
    unsafe { alloc_with_backing(NSLOCALE_CLASS.ptr(), Backing::Str(id)) }
}
extern "C" fn locale_id(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_else(|| "en_US".to_string());
    nsstring_from_rust(&s)
}
extern "C" fn locale_langs(_self: Id, _cmd: Sel) -> Id {
    let en = nsstring_from_rust("en");
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec![retain(en) as usize])) }
}

extern "C" fn req_with_url(_self: Id, _cmd: Sel, url: Id) -> Id {
    let mut map = HashMap::new();
    map.insert("url".into(), retain(url) as usize);
    map.insert("method".into(), retain(nsstring_from_rust("GET")) as usize);
    unsafe { alloc_with_backing(NSURLREQUEST_CLASS.ptr(), Backing::Dict(map)) }
}
extern "C" fn mreq_with_url(_self: Id, _cmd: Sel, url: Id) -> Id {
    let mut map = HashMap::new();
    map.insert("url".into(), retain(url) as usize);
    map.insert("method".into(), retain(nsstring_from_rust("GET")) as usize);
    unsafe { alloc_with_backing(NSMUTABLEURLREQUEST_CLASS.ptr(), Backing::Dict(map)) }
}
extern "C" fn req_url(self_: Id, _cmd: Sel) -> Id {
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get(&(self_ as usize)) {
        return d.get("url").copied().unwrap_or(0) as Id;
    }
    std::ptr::null_mut()
}
extern "C" fn req_method(self_: Id, _cmd: Sel) -> Id {
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get(&(self_ as usize)) {
        return d.get("method").copied().unwrap_or(0) as Id;
    }
    nsstring_from_rust("GET")
}
extern "C" fn req_set_method(self_: Id, _cmd: Sel, m: Id) {
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get_mut(&(self_ as usize)) {
        d.insert("method".into(), retain(m) as usize);
    }
}
extern "C" fn req_set_header(self_: Id, _cmd: Sel, v: Id, field: Id) {
    let f = rust_str_from_nsstring(field).unwrap_or_default();
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get_mut(&(self_ as usize)) {
        d.insert(format!("hdr:{f}"), retain(v) as usize);
    }
}
extern "C" fn req_body(self_: Id, _cmd: Sel) -> Id {
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get(&(self_ as usize)) {
        return d.get("body").copied().unwrap_or(0) as Id;
    }
    std::ptr::null_mut()
}
extern "C" fn req_set_body(self_: Id, _cmd: Sel, body: Id) {
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get_mut(&(self_ as usize)) {
        d.insert("body".into(), retain(body) as usize);
    }
}

extern "C" fn uuid_new(_self: Id, _cmd: Sel) -> Id {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let s = format!("50656172-5369-4c69-8000-{n:012x}");
    unsafe { alloc_with_backing(NSUUID_CLASS.ptr(), Backing::Str(s)) }
}
extern "C" fn uuid_string(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    nsstring_from_rust(&s)
}

extern "C" fn pi_shared(_self: Id, _cmd: Sel) -> Id {
    static S: OnceLock<usize> = OnceLock::new();
    *S.get_or_init(|| unsafe { alloc_with_backing(NSPROCESSINFO_CLASS.ptr(), Backing::Arr(Vec::new())) } as usize) as Id
}
extern "C" fn pi_os_ver(_self: Id, _cmd: Sel) -> Id {
    nsstring_from_rust("PearRuntime 1.0 (iOS 17.0 compatible)")
}
extern "C" fn pi_cpus(_self: Id, _cmd: Sel) -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}
extern "C" fn pi_mem(_self: Id, _cmd: Sel) -> u64 {
    4u64 * 1024 * 1024 * 1024
}

pub unsafe extern "C" fn cf_data_create(_alloc: *const c_void, bytes: *const u8, len: isize) -> Id {
    let slice = if bytes.is_null() || len <= 0 {
        &[]
    } else {
        std::slice::from_raw_parts(bytes, len as usize)
    };
    alloc_with_backing(NSDATA_CLASS.ptr(), Backing::Bytes(slice.to_vec()))
}
pub extern "C" fn cf_data_get_length(data: Id) -> isize {
    if let Some(Backing::Bytes(b)) = store().lock().unwrap().get(&(data as usize)) {
        return b.len() as isize;
    }
    0
}
pub extern "C" fn cf_data_get_byte_ptr(data: Id) -> *const u8 {
    static EMPTY: [u8; 0] = [];
    if let Some(Backing::Bytes(b)) = store().lock().unwrap().get(&(data as usize)) {
        return b.as_ptr();
    }
    EMPTY.as_ptr()
}
pub extern "C" fn cf_array_get_count(arr: Id) -> isize {
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get(&(arr as usize)) {
        return v.len() as isize;
    }
    0
}
pub extern "C" fn cf_array_get_value_at_index(arr: Id, idx: isize) -> *const c_void {
    if idx < 0 { return std::ptr::null(); }
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get(&(arr as usize)) {
        return v.get(idx as usize).copied().unwrap_or(0) as *const c_void;
    }
    std::ptr::null()
}
pub extern "C" fn cf_dictionary_get_value(dict: Id, key: Id) -> *const c_void {
    let k = rust_str_from_nsstring(key).unwrap_or_default();
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get(&(dict as usize)) {
        return d.get(&k).copied().unwrap_or(0) as *const c_void;
    }
    std::ptr::null()
}

pub unsafe extern "C" fn cf_dictionary_create(
    _alloc: *const c_void,
    keys: *const Id,
    values: *const Id,
    num_values: isize,
    _key_cb: *const c_void,
    _val_cb: *const c_void,
) -> Id {
    let mut map = HashMap::new();
    if !keys.is_null() && !values.is_null() && num_values > 0 {
        for idx in 0..num_values as usize {
            let k = rust_str_from_nsstring(*keys.add(idx)).unwrap_or_default();
            let v = *values.add(idx);
            map.insert(k, retain(v) as usize);
        }
    }
    alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map))
}

pub unsafe extern "C" fn cf_dictionary_create_mutable(
    _alloc: *const c_void,
    _capacity: isize,
    _key_cb: *const c_void,
    _val_cb: *const c_void,
) -> Id {
    alloc_with_backing(NSMUTABLEDICTIONARY_CLASS.ptr(), Backing::Dict(HashMap::new()))
}

pub unsafe extern "C" fn cf_dictionary_set_value(dict: Id, key: Id, val: Id) {
    let k = rust_str_from_nsstring(key).unwrap_or_default();
    if let Some(Backing::Dict(d)) = store().lock().unwrap().get_mut(&(dict as usize)) {
        if val.is_null() {
            d.remove(&k);
        } else {
            d.insert(k, retain(val) as usize);
        }
    }
}

pub unsafe extern "C" fn cf_number_create(
    _alloc: *const c_void,
    the_type: isize,
    value_ptr: *const c_void,
) -> Id {
    if value_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let backing = match the_type {
        1 | 7 => Backing::NumI64(*(value_ptr as *const i8) as i64),
        2 | 8 => Backing::NumI64(*(value_ptr as *const i16) as i64),
        3 | 9 => Backing::NumI64(*(value_ptr as *const i32) as i64),
        4 | 10 | 11 | 14 => Backing::NumI64(*(value_ptr as *const i64)),
        5 | 12 => Backing::NumF64(*(value_ptr as *const f32) as f64),
        6 | 13 => Backing::NumF64(*(value_ptr as *const f64)),
        _ => Backing::NumI64(*(value_ptr as *const i32) as i64),
    };
    alloc_with_backing(NSNUMBER_CLASS.ptr(), backing)
}

pub unsafe extern "C" fn cf_number_get_value(
    num: Id,
    the_type: isize,
    value_ptr: *mut c_void,
) -> bool {
    if value_ptr.is_null() {
        return false;
    }
    let backing = store().lock().unwrap().get(&(num as usize)).cloned();
    let (i, f) = match backing {
        Some(Backing::NumI64(n)) => (n, n as f64),
        Some(Backing::NumF64(x)) => (x as i64, x),
        Some(Backing::NumBool(b)) => (i64::from(b), if b { 1.0 } else { 0.0 }),
        _ => return false,
    };
    match the_type {
        1 | 7 => *(value_ptr as *mut i8) = i as i8,
        2 | 8 => *(value_ptr as *mut i16) = i as i16,
        3 | 9 => *(value_ptr as *mut i32) = i as i32,
        4 | 10 | 11 | 14 => *(value_ptr as *mut i64) = i,
        5 | 12 => *(value_ptr as *mut f32) = f as f32,
        6 | 13 => *(value_ptr as *mut f64) = f,
        _ => *(value_ptr as *mut i32) = i as i32,
    }
    true
}

fn const_nsstr(s: &'static str) -> usize {
    static MAP: OnceLock<Mutex<HashMap<&'static str, usize>>> = OnceLock::new();
    let m = MAP.get_or_init(Default::default);
    let mut g = m.lock().unwrap();
    *g.entry(s).or_insert_with(|| nsstring_from_rust(s) as usize)
}

fn empty_dict_singleton() -> usize {
    static D: OnceLock<usize> = OnceLock::new();
    *D.get_or_init(|| unsafe { alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(HashMap::new())) } as usize)
}

fn cf_bool_true_ptr() -> usize {
    static B: OnceLock<usize> = OnceLock::new();
    *B.get_or_init(|| unsafe { alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(true)) } as usize)
}

fn cf_bool_false_ptr() -> usize {
    static B: OnceLock<usize> = OnceLock::new();
    *B.get_or_init(|| unsafe { alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(false)) } as usize)
}

pub extern "C" fn cf_string_get_length(s: Id) -> isize {
    rust_str_from_nsstring(s).map(|x| x.encode_utf16().count() as isize).unwrap_or(0)
}

pub unsafe extern "C" fn cf_string_get_cstring(s: Id, buf: *mut c_char, buf_size: isize, _enc: u32) -> bool {
    if buf.is_null() || buf_size <= 0 {
        return false;
    }
    let Some(text) = rust_str_from_nsstring(s) else {
        return false;
    };
    let bytes = text.as_bytes();
    if bytes.len() + 1 > buf_size as usize {
        return false;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
    *buf.add(bytes.len()) = 0;
    true
}

pub extern "C" fn cf_string_get_max_size(len: isize, _enc: u32) -> isize {
    len.max(0) * 4 + 1
}

pub extern "C" fn cf_string_create_with_format(_alloc: *const c_void, _opts: *const c_void, fmt: Id) -> Id {
    let s = rust_str_from_nsstring(fmt).unwrap_or_default();
    nsstring_from_rust(&s)
}

pub extern "C" fn cf_runloop_get_main() -> Id {
    rl_main(NSRUNLOOP_CLASS.ptr() as Id, sel("mainRunLoop"))
}

pub extern "C" fn cf_read_stream_open(_s: *const c_void) -> bool { false }
pub extern "C" fn cf_read_stream_close(_s: *const c_void) {}
pub extern "C" fn cf_read_stream_get_status(_s: *const c_void) -> isize { 6 }
pub extern "C" fn cf_read_stream_has_bytes(_s: *const c_void) -> bool { false }
pub extern "C" fn cf_read_stream_read(_s: *const c_void, _buf: *mut u8, _len: isize) -> isize { 0 }
pub extern "C" fn cf_read_stream_copy_prop(_s: *const c_void, _name: Id) -> *const c_void { std::ptr::null() }
pub extern "C" fn cf_read_stream_set_prop(_s: *const c_void, _name: Id, _val: *const c_void) -> bool { false }
pub extern "C" fn cf_read_stream_sched(_s: *const c_void, _rl: *const c_void, _mode: Id) {}

pub unsafe extern "C" fn cf_url_create_with_bytes(
    _alloc: *const c_void,
    bytes: *const u8,
    len: isize,
    _enc: u32,
    _base: Id,
) -> Id {
    let s = if bytes.is_null() || len <= 0 {
        String::new()
    } else {
        String::from_utf8_lossy(std::slice::from_raw_parts(bytes, len as usize)).into_owned()
    };
    alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(s))
}

pub extern "C" fn cf_url_copy_null(_url: Id) -> Id { std::ptr::null_mut() }

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

            let ud: Id = send0(NSUSERDEFAULTS_CLASS.ptr() as Id, "standardUserDefaults");
            let _: () = send2(ud, "setObject:forKey:", joined, s1);
            let got: Id = send1(ud, "objectForKey:", s1);
            assert_eq!(rust_str_from_nsstring(got).as_deref(), Some("hello/world"));

            let u: Id = send1(NSURL_CLASS.ptr() as Id, "fileURLWithPath:", s2);
            let up: Id = send0(u, "path");
            assert_eq!(rust_str_from_nsstring(up).as_deref(), Some("world"));

            let cls_name = ns_string_from_class(NSSTRING_CLASS.ptr());
            assert_eq!(rust_str_from_nsstring(cls_name).as_deref(), Some("NSString"));

            let jraw = b"{\"ok\":true,\"n\":7,\"arr\":[1,\"hi\"]}";
            let jdata = cf_data_create(std::ptr::null(), jraw.as_ptr(), jraw.len() as isize);
            let parsed: Id = send3(
                NSJSONSERIALIZATION_CLASS.ptr() as Id,
                "JSONObjectWithData:options:error:",
                jdata,
                0usize,
                std::ptr::null_mut::<Id>(),
            );
            let k_arr = nsstring_from_rust("arr");
            let arr_ptr = cf_dictionary_get_value(parsed, k_arr) as Id;
            assert_eq!(cf_array_get_count(arr_ptr), 2);
            let encoded: Id = send3(
                NSJSONSERIALIZATION_CLASS.ptr() as Id,
                "dataWithJSONObject:options:error:",
                parsed,
                0usize,
                std::ptr::null_mut::<Id>(),
            );
            assert!(cf_data_get_length(encoded) > 10);

            let uuid: Id = send0(NSUUID_CLASS.ptr() as Id, "UUID");
            let ustr: Id = send0(uuid, "UUIDString");
            assert!(rust_str_from_nsstring(ustr).unwrap().starts_with("50656172-"));
        }
    }

    unsafe fn send3<A1, A2, A3, R>(obj: Id, name: &str, a1: A1, a2: A2, a3: A3) -> R {
        let s = sel(name);
        let d = lookup_for_send(obj, s);
        let f: extern "C" fn(Id, Sel, A1, A2, A3) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s, a1, a2, a3)
    }

    unsafe fn send2<A1, A2, R>(obj: Id, name: &str, a1: A1, a2: A2) -> R {
        let s = sel(name);
        let d = lookup_for_send(obj, s);
        let f: extern "C" fn(Id, Sel, A1, A2) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s, a1, a2)
    }
}
