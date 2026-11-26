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

mod cf_ext;

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
foundation_class!(NSAUTORELEASEPOOL_CLASS, NSAUTORELEASEPOOL_META);
foundation_class!(NSFILECOORDINATOR_CLASS, NSFILECOORDINATOR_META);
foundation_class!(NSFILEVERSION_CLASS, NSFILEVERSION_META);
foundation_class!(NSMETADATAQUERY_CLASS, NSMETADATAQUERY_META);
foundation_class!(NSNUMBERFORMATTER_CLASS, NSNUMBERFORMATTER_META);
foundation_class!(NSPREDICATE_CLASS, NSPREDICATE_META);
foundation_class!(NSTHREAD_CLASS, NSTHREAD_META);
foundation_class!(NSCALENDAR_CLASS, NSCALENDAR_META);
foundation_class!(NSDATECOMPONENTS_CLASS, NSDATECOMPONENTS_META);
foundation_class!(NSEXCEPTION_CLASS, NSEXCEPTION_META);
foundation_class!(NSENUMERATOR_CLASS, NSENUMERATOR_META);
foundation_class!(NSURLSESSION_CLASS, NSURLSESSION_META);
foundation_class!(NSURLSESSIONCONFIGURATION_CLASS, NSURLSESSIONCONFIGURATION_META);
foundation_class!(NSOPERATIONQUEUE_CLASS, NSOPERATIONQUEUE_META);
foundation_class!(NSKEYEDUNARCHIVER_CLASS, NSKEYEDUNARCHIVER_META);
foundation_class!(NSASSERTIONHANDLER_CLASS, NSASSERTIONHANDLER_META);
foundation_class!(NSNULL_CLASS, NSNULL_META);

#[repr(C)]
struct ObjcEhType {
    vtable: *const c_void,
    name: *const c_char,
    cls: *const ClassT,
}

unsafe impl Sync for ObjcEhType {}

static OBJC_EHTYPE_NSEXCEPTION: ObjcEhType = ObjcEhType {
    vtable: std::ptr::null(),
    name: c"NSException".as_ptr(),
    cls: &raw const NSEXCEPTION_CLASS as *const ClassT,
};

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

fn backing_or_cfstring(obj: Id) -> Option<String> {
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
    }
    None
}

pub fn rust_str_from_nsstring(obj: Id) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    if let Some(s) = backing_or_cfstring(obj) {
        return Some(s);
    }
    unsafe {
        if !is_tagged_pointer(obj) {
            let cls = runtime::object_class(obj);
            if runtime::is_meta(cls) || runtime::lookup(cls, sel("UTF8String")).is_none() {
                return None;
            }
        }
        send_str(obj, "UTF8String")
    }
}

pub fn nsstring_from_rust(s: &str) -> Id {
    init_foundation();
    unsafe { alloc_with_backing(NSSTRING_CLASS.ptr(), Backing::Str(s.to_string())) }
}

pub fn nsdata_from_bytes(bytes: &[u8]) -> Id {
    init_foundation();
    unsafe { alloc_with_backing(NSDATA_CLASS.ptr(), Backing::Bytes(bytes.to_vec())) }
}

pub fn rust_bytes_from_nsdata(obj: Id) -> Option<Vec<u8>> {
    if obj.is_null() {
        return None;
    }
    if let Some(Backing::Bytes(b)) = store().lock().unwrap().get(&(obj as usize)) {
        return Some(b.clone());
    }
    None
}

pub fn nsdict_lookup_str(dict: Id, key: &str) -> Option<Id> {
    if dict.is_null() {
        return None;
    }
    if let Some(Backing::Dict(m)) = store().lock().unwrap().get(&(dict as usize)) {
        return m.get(key).copied().map(|p| p as Id);
    }
    None
}

pub fn nsdict_from_pairs(pairs: Vec<(String, Id)>) -> Id {
    init_foundation();
    let map: HashMap<String, usize> = pairs.into_iter().map(|(k, v)| (k, v as usize)).collect();
    unsafe { alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map)) }
}

pub fn nsarray_from_ids(items: Vec<Id>) -> Id {
    init_foundation();
    let vec: Vec<usize> = items.into_iter().map(|p| p as usize).collect();
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec)) }
}

pub fn nsarray_ids(arr: Id) -> Option<Vec<Id>> {
    if arr.is_null() {
        return None;
    }
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get(&(arr as usize)) {
        return Some(v.iter().map(|&p| p as Id).collect());
    }
    None
}

pub fn nsdict_entries(dict: Id) -> Option<HashMap<String, Id>> {
    if dict.is_null() {
        return None;
    }
    if let Some(Backing::Dict(m)) = store().lock().unwrap().get(&(dict as usize)) {
        return Some(m.iter().map(|(k, &v)| (k.clone(), v as Id)).collect());
    }
    None
}

pub fn const_nsstr_ptr(s: &'static str) -> usize {
    init_foundation();
    const_nsstr(s)
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
    let s = backing_or_cfstring(this).unwrap_or_default();
    let boxed = CString::new(s).unwrap_or_default().into_boxed_c_str();
    Box::leak(boxed).as_ptr()
}

extern "C" fn str_data_using_encoding(this: Id, _: Sel, _encoding: usize) -> Id {
    let s = backing_or_cfstring(this).unwrap_or_default();
    unsafe { alloc_with_backing(NSDATA_CLASS.ptr(), Backing::Bytes(s.into_bytes())) }
}

extern "C" fn str_length(this: Id, _: Sel) -> usize {
    backing_or_cfstring(this).map_or(0, |s| s.encode_utf16().count())
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

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSRange {
    location: usize,
    length: usize,
}

extern "C" fn str_empty(cls: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Str(String::new())) }
}

extern "C" fn str_with_string(cls: Id, _: Sel, other: Id) -> Id {
    let s = rust_str_from_nsstring(other).unwrap_or_default();
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Str(s)) }
}

extern "C" fn str_with_cstring_enc(cls: Id, sel_id: Sel, cstr: *const c_char, _enc: usize) -> Id {
    str_with_utf8(cls, sel_id, cstr)
}

extern "C" fn str_init_utf8(this: Id, _: Sel, cstr: *const c_char) -> Id {
    let s = if cstr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(cstr).to_string_lossy().into_owned() }
    };
    unsafe { alloc_with_backing(if this.is_null() { NSSTRING_CLASS.ptr() } else { *(this as *const *const ClassT) }, Backing::Str(s)) }
}

extern "C" fn str_init_with_string(this: Id, _: Sel, other: Id) -> Id {
    let s = rust_str_from_nsstring(other).unwrap_or_default();
    unsafe { alloc_with_backing(if this.is_null() { NSSTRING_CLASS.ptr() } else { *(this as *const *const ClassT) }, Backing::Str(s)) }
}

/// Safe `stringWithFormat:` / `initWithFormat:`: preserves format string safely
/// without dereferencing unaligned Apple-ARM64 stack varargs on Linux AAPCS64.
extern "C" fn str_with_format(cls: Id, _: Sel, fmt: Id) -> Id {
    let s = rust_str_from_nsstring(fmt).unwrap_or_default();
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Str(s)) }
}

extern "C" fn str_init_with_format(_this: Id, _: Sel, fmt: Id) -> Id {
    let s = rust_str_from_nsstring(fmt).unwrap_or_default();
    nsstring_from_rust(&s)
}

extern "C" fn str_init_with_format_args(_this: Id, _: Sel, fmt: Id, _args: *const c_void) -> Id {
    let s = rust_str_from_nsstring(fmt).unwrap_or_default();
    nsstring_from_rust(&s)
}

extern "C" fn str_init_with_format_loc_args(
    _this: Id,
    _: Sel,
    fmt: Id,
    _loc: Id,
    _args: *const c_void,
) -> Id {
    let s = rust_str_from_nsstring(fmt).unwrap_or_default();
    nsstring_from_rust(&s)
}

extern "C" fn str_has_prefix(this: Id, _: Sel, prefix: Id) -> bool {
    match (rust_str_from_nsstring(this), rust_str_from_nsstring(prefix)) {
        (Some(a), Some(b)) => a.starts_with(&b),
        _ => false,
    }
}

extern "C" fn str_has_suffix(this: Id, _: Sel, suffix: Id) -> bool {
    match (rust_str_from_nsstring(this), rust_str_from_nsstring(suffix)) {
        (Some(a), Some(b)) => a.ends_with(&b),
        _ => false,
    }
}

extern "C" fn str_append_str(this: Id, _: Sel, other: Id) -> Id {
    let a = rust_str_from_nsstring(this).unwrap_or_default();
    let b = rust_str_from_nsstring(other).unwrap_or_default();
    nsstring_from_rust(&format!("{a}{b}"))
}

extern "C" fn str_lowercase(this: Id, _: Sel) -> Id {
    let a = rust_str_from_nsstring(this).unwrap_or_default();
    nsstring_from_rust(&a.to_lowercase())
}

extern "C" fn str_uppercase(this: Id, _: Sel) -> Id {
    let a = rust_str_from_nsstring(this).unwrap_or_default();
    nsstring_from_rust(&a.to_uppercase())
}

extern "C" fn str_range_of(this: Id, _: Sel, needle: Id) -> NSRange {
    let hay = rust_str_from_nsstring(this).unwrap_or_default();
    let sub = rust_str_from_nsstring(needle).unwrap_or_default();
    if !sub.is_empty() {
        if let Some(byte_idx) = hay.find(&sub) {
            let loc = hay[..byte_idx].encode_utf16().count();
            let len = sub.encode_utf16().count();
            return NSRange { location: loc, length: len };
        }
    }
    NSRange { location: usize::MAX, length: 0 }
}

extern "C" fn str_components_sep(this: Id, _: Sel, sep: Id) -> Id {
    let hay = rust_str_from_nsstring(this).unwrap_or_default();
    let s = rust_str_from_nsstring(sep).unwrap_or_default();
    let parts: Vec<usize> = if s.is_empty() {
        vec![nsstring_from_rust(&hay) as usize]
    } else {
        hay.split(&s).map(|p| nsstring_from_rust(p) as usize).collect()
    };
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(parts)) }
}

extern "C" fn str_int_val(this: Id, _: Sel) -> i32 {
    rust_str_from_nsstring(this).unwrap_or_default().trim().parse::<i32>().unwrap_or(0)
}
extern "C" fn str_integer_val(this: Id, _: Sel) -> isize {
    rust_str_from_nsstring(this).unwrap_or_default().trim().parse::<isize>().unwrap_or(0)
}
extern "C" fn str_double_val(this: Id, _: Sel) -> f64 {
    rust_str_from_nsstring(this).unwrap_or_default().trim().parse::<f64>().unwrap_or(0.0)
}
extern "C" fn str_float_val(this: Id, _: Sel) -> f32 {
    rust_str_from_nsstring(this).unwrap_or_default().trim().parse::<f32>().unwrap_or(0.0)
}
extern "C" fn str_bool_val(this: Id, _: Sel) -> bool {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let t = s.trim_start();
    t.starts_with('Y') || t.starts_with('y') || t.starts_with('T') || t.starts_with('t') || t.chars().next().is_some_and(|c| c.is_ascii_digit() && c != '0')
}
extern "C" fn str_description(this: Id, _: Sel) -> Id {
    this
}
extern "C" fn str_hash(this: Id, _: Sel) -> usize {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let mut h: usize = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as usize);
    }
    h
}

extern "C" fn str_path_with_components(_cls: Id, _: Sel, comps: Id) -> Id {
    let count = arr_count(comps, sel("count"));
    let mut parts = Vec::with_capacity(count);
    for idx in 0..count {
        if let Some(s) = rust_str_from_nsstring(arr_at(comps, sel("objectAtIndex:"), idx)) {
            if !s.is_empty() {
                parts.push(s);
            }
        }
    }
    let joined = parts.join("/");
    nsstring_from_rust(&joined.replace("//", "/"))
}

extern "C" fn str_path_components(this: Id, _: Sel) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let parts: Vec<usize> = s
        .split('/')
        .filter(|p| !p.is_empty())
        .map(|p| nsstring_from_rust(p) as usize)
        .collect();
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(parts)) }
}

extern "C" fn str_last_path_comp(this: Id, _: Sel) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let last = std::path::Path::new(&s)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    nsstring_from_rust(&last)
}

extern "C" fn str_del_last_path_comp(this: Id, _: Sel) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let parent = std::path::Path::new(&s)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    nsstring_from_rust(&parent)
}

extern "C" fn str_path_ext(this: Id, _: Sel) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let ext = std::path::Path::new(&s)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    nsstring_from_rust(&ext)
}

extern "C" fn str_del_path_ext(this: Id, _: Sel) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let p = std::path::Path::new(&s);
    let stem = p.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
    let out = match p.parent() {
        Some(par) if !par.as_os_str().is_empty() => par.join(stem).to_string_lossy().into_owned(),
        _ => stem,
    };
    nsstring_from_rust(&out)
}

extern "C" fn str_append_path_ext(this: Id, _: Sel, ext: Id) -> Id {
    let base = rust_str_from_nsstring(this).unwrap_or_default();
    let e = rust_str_from_nsstring(ext).unwrap_or_default();
    if e.is_empty() {
        nsstring_from_rust(&base)
    } else {
        nsstring_from_rust(&format!("{base}.{e}"))
    }
}

extern "C" fn str_replace_occ(this: Id, _: Sel, target: Id, rep: Id) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let t = rust_str_from_nsstring(target).unwrap_or_default();
    let r = rust_str_from_nsstring(rep).unwrap_or_default();
    if t.is_empty() {
        nsstring_from_rust(&s)
    } else {
        nsstring_from_rust(&s.replace(&t, &r))
    }
}

extern "C" fn str_trim_chars(this: Id, _: Sel, _set: Id) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    nsstring_from_rust(s.trim())
}

extern "C" fn str_substr_from(this: Id, _: Sel, from: usize) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let start = from.min(utf16.len());
    nsstring_from_rust(&String::from_utf16_lossy(&utf16[start..]))
}

extern "C" fn str_substr_to(this: Id, _: Sel, to: usize) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let end = to.min(utf16.len());
    nsstring_from_rust(&String::from_utf16_lossy(&utf16[..end]))
}

extern "C" fn str_substr_range(this: Id, _: Sel, range: NSRange) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let start = range.location.min(utf16.len());
    let end = start.saturating_add(range.length).min(utf16.len());
    nsstring_from_rust(&String::from_utf16_lossy(&utf16[start..end]))
}

extern "C" fn str_compare(this: Id, _: Sel, other: Id) -> isize {
    let a = rust_str_from_nsstring(this).unwrap_or_default();
    let b = rust_str_from_nsstring(other).unwrap_or_default();
    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

extern "C" fn str_case_compare(this: Id, _: Sel, other: Id) -> isize {
    let a = rust_str_from_nsstring(this).unwrap_or_default().to_lowercase();
    let b = rust_str_from_nsstring(other).unwrap_or_default().to_lowercase();
    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn compare_strings_opts(mut a: String, mut b: String, opts: usize) -> isize {
    if (opts & 1) != 0 {
        a = a.to_lowercase();
        b = b.to_lowercase();
    }
    if (opts & 64) != 0 {
        // NSNumericSearch: split alternating non-digit / digit segments
        fn split_num(s: &str) -> Vec<(bool, &str)> {
            let mut res = Vec::new();
            let bytes = s.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let is_num = bytes[i].is_ascii_digit();
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() == is_num {
                    i += 1;
                }
                res.push((is_num, &s[start..i]));
            }
            res
        }
        let pa = split_num(&a);
        let pb = split_num(&b);
        for (sa, sb) in pa.iter().zip(pb.iter()) {
            let ord = if sa.0 && sb.0 {
                let na = sa.1.parse::<u64>().unwrap_or(0);
                let nb = sb.1.parse::<u64>().unwrap_or(0);
                na.cmp(&nb).then_with(|| sa.1.len().cmp(&sb.1.len()))
            } else {
                sa.1.cmp(sb.1)
            };
            if ord != std::cmp::Ordering::Equal {
                return match ord {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Greater => 1,
                    std::cmp::Ordering::Equal => 0,
                };
            }
        }
        return match pa.len().cmp(&pb.len()) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        };
    }
    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

extern "C" fn str_compare_opts(this: Id, _: Sel, other: Id, opts: usize) -> isize {
    let a = rust_str_from_nsstring(this).unwrap_or_default();
    let b = rust_str_from_nsstring(other).unwrap_or_default();
    compare_strings_opts(a, b, opts)
}

extern "C" fn str_compare_opts_range(this: Id, _: Sel, other: Id, opts: usize, range: NSRange) -> isize {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let u16s: Vec<u16> = s.encode_utf16().collect();
    let start = range.location.min(u16s.len());
    let end = start.saturating_add(range.length).min(u16s.len());
    let a = String::from_utf16_lossy(&u16s[start..end]);
    let b = rust_str_from_nsstring(other).unwrap_or_default();
    compare_strings_opts(a, b, opts)
}

extern "C" fn str_contains_string(this: Id, _: Sel, sub: Id) -> bool {
    let a = rust_str_from_nsstring(this).unwrap_or_default();
    let b = rust_str_from_nsstring(sub).unwrap_or_default();
    !b.is_empty() && a.contains(&b)
}

extern "C" fn str_localized_ci_contains(this: Id, _: Sel, sub: Id) -> bool {
    let a = rust_str_from_nsstring(this).unwrap_or_default().to_lowercase();
    let b = rust_str_from_nsstring(sub).unwrap_or_default().to_lowercase();
    !b.is_empty() && a.contains(&b)
}

extern "C" fn str_range_of_opts(this: Id, _: Sel, needle: Id, opts: usize) -> NSRange {
    let hay = rust_str_from_nsstring(this).unwrap_or_default();
    let sub = rust_str_from_nsstring(needle).unwrap_or_default();
    if sub.is_empty() {
        return NSRange { location: usize::MAX, length: 0 };
    }
    let ci = (opts & 1) != 0; // NSCaseInsensitiveSearch = 1
    let backwards = (opts & 4) != 0; // NSBackwardsSearch = 4
    let (h, s) = if ci { (hay.to_lowercase(), sub.to_lowercase()) } else { (hay.clone(), sub.clone()) };
    let idx = if backwards { h.rfind(&s) } else { h.find(&s) };
    if let Some(byte_idx) = idx {
        let loc = hay[..byte_idx].encode_utf16().count();
        let len = sub.encode_utf16().count();
        NSRange { location: loc, length: len }
    } else {
        NSRange { location: usize::MAX, length: 0 }
    }
}

extern "C" fn str_range_of_charset(this: Id, _: Sel, set: Id) -> NSRange {
    let hay = rust_str_from_nsstring(this).unwrap_or_default();
    let chars = rust_str_from_nsstring(set).unwrap_or_default();
    if !chars.is_empty() {
        for (byte_idx, ch) in hay.char_indices() {
            if chars.contains(ch) {
                let loc = hay[..byte_idx].encode_utf16().count();
                let len = ch.len_utf16();
                return NSRange { location: loc, length: len };
            }
        }
    }
    NSRange { location: usize::MAX, length: 0 }
}

extern "C" fn str_range_of_charset_opts(this: Id, sel_id: Sel, set: Id, _opts: usize) -> NSRange {
    str_range_of_charset(this, sel_id, set)
}

extern "C" fn str_char_at(this: Id, _: Sel, idx: usize) -> u16 {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    s.encode_utf16().nth(idx).unwrap_or(0)
}

extern "C" fn str_get_chars(this: Id, _: Sel, buf: *mut u16, range: NSRange) {
    if buf.is_null() {
        return;
    }
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let start = range.location.min(utf16.len());
    let n = range.length.min(utf16.len() - start);
    unsafe {
        std::ptr::copy_nonoverlapping(utf16[start..].as_ptr(), buf, n);
    }
}

extern "C" fn str_data_lossy(this: Id, sel_id: Sel, enc: usize, _lossy: bool) -> Id {
    str_data_using_encoding(this, sel_id, enc)
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

extern "C" fn arr_with_obj(cls: Id, _: Sel, obj: Id) -> Id {
    let mut vec = Vec::new();
    if !obj.is_null() {
        retain(obj);
        vec.push(obj as usize);
    }
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Arr(vec)) }
}

extern "C" fn arr_first(this: Id, sel_id: Sel) -> Id {
    arr_at(this, sel_id, 0)
}

extern "C" fn arr_last(this: Id, _: Sel) -> Id {
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .and_then(|b| match b {
            Backing::Arr(v) => v.last().map(|&p| p as Id),
            _ => None,
        })
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn arr_index_of(this: Id, _: Sel, obj: Id) -> usize {
    let target = obj as usize;
    store()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .and_then(|b| match b {
            Backing::Arr(v) => v.iter().position(|&p| p == target),
            _ => None,
        })
        .unwrap_or(usize::MAX)
}

extern "C" fn arr_contains(this: Id, sel_id: Sel, obj: Id) -> bool {
    arr_index_of(this, sel_id, obj) != usize::MAX
}

extern "C" fn arr_remove_at(this: Id, _: Sel, idx: usize) {
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        if idx < v.len() {
            v.remove(idx);
        }
    }
}

extern "C" fn arr_remove_all(this: Id, _: Sel) {
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        v.clear();
    }
}

extern "C" fn arr_set_subscript(this: Id, _: Sel, obj: Id, idx: usize) {
    if obj.is_null() { return; }
    retain(obj);
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        if idx < v.len() {
            v[idx] = obj as usize;
        } else if idx == v.len() {
            v.push(obj as usize);
        }
    }
}

extern "C" fn arr_replace_at(this: Id, sel_id: Sel, idx: usize, obj: Id) {
    arr_set_subscript(this, sel_id, obj, idx);
}

extern "C" fn arr_insert_at(this: Id, _: Sel, obj: Id, idx: usize) {
    if obj.is_null() { return; }
    retain(obj);
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        let pos = idx.min(v.len());
        v.insert(pos, obj as usize);
    }
}

extern "C" fn arr_remove_obj(this: Id, _: Sel, obj: Id) {
    let target = obj as usize;
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        v.retain(|&p| p != target);
    }
}

extern "C" fn arr_remove_last(this: Id, _: Sel) {
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        v.pop();
    }
}

extern "C" fn arr_exchange(this: Id, _: Sel, i: usize, j: usize) {
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(this as usize)) {
        if i < v.len() && j < v.len() {
            v.swap(i, j);
        }
    }
}

extern "C" fn arr_subarray(this: Id, _: Sel, range: NSRange) -> Id {
    let sub = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Arr(v)) => {
            let start = range.location.min(v.len());
            let end = start.saturating_add(range.length).min(v.len());
            v[start..end].to_vec()
        }
        _ => Vec::new(),
    };
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(sub)) }
}

extern "C" fn arr_is_equal(this: Id, _: Sel, other: Id) -> bool {
    if this == other { return true; }
    if other.is_null() { return false; }
    let g = store().lock().unwrap();
    match (g.get(&(this as usize)), g.get(&(other as usize))) {
        (Some(Backing::Arr(a)), Some(Backing::Arr(b))) => a == b,
        _ => false,
    }
}

extern "C" fn arr_copy(this: Id, _: Sel) -> Id {
    let v = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Arr(v)) => v.clone(),
        _ => Vec::new(),
    };
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(v)) }
}

extern "C" fn arr_mcopy(this: Id, _: Sel) -> Id {
    let v = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Arr(v)) => v.clone(),
        _ => Vec::new(),
    };
    unsafe { alloc_with_backing(NSMUTABLEARRAY_CLASS.ptr(), Backing::Arr(v)) }
}

extern "C" fn arr_copy_zone(this: Id, sel_id: Sel, _: *mut c_void) -> Id {
    arr_copy(this, sel_id)
}

unsafe fn collect_va_objects(first: Id, va: *const usize) -> Vec<usize> {
    let mut vec = Vec::new();
    if first.is_null() {
        return vec;
    }
    retain(first);
    vec.push(first as usize);
    if !va.is_null() {
        for idx in 0..256 {
            let p = *va.add(idx) as Id;
            if p.is_null() {
                break;
            }
            retain(p);
            vec.push(p as usize);
        }
    }
    vec
}

extern "C" fn arr_init_objs_va(this: Id, _cmd: Sel, first: Id, va: *const usize) -> Id {
    let vec = unsafe { collect_va_objects(first, va) };
    store().lock().unwrap().insert(this as usize, Backing::Arr(vec));
    this
}
extern "C" fn arr_with_objs_va(cls: Id, _cmd: Sel, first: Id, va: *const usize) -> Id {
    let vec = unsafe { collect_va_objects(first, va) };
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Arr(vec)) }
}
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(naked)]
extern "C" fn darwin_arr_init_objs() -> ! {
    core::arch::naked_asm!("mov x3, sp", "b {dst}", dst = sym arr_init_objs_va);
}
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(naked)]
extern "C" fn darwin_arr_with_objs() -> ! {
    core::arch::naked_asm!("mov x3, sp", "b {dst}", dst = sym arr_with_objs_va);
}
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
extern "C" fn darwin_arr_init_objs(this: Id, sel_id: Sel, first: Id) -> Id {
    arr_init_objs_va(this, sel_id, first, std::ptr::null())
}
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
extern "C" fn darwin_arr_with_objs(cls: Id, sel_id: Sel, first: Id) -> Id {
    arr_with_objs_va(cls, sel_id, first, std::ptr::null())
}

extern "C" fn arr_init_count(this: Id, _cmd: Sel, objs: *const Id, count: usize) -> Id {
    let mut vec = Vec::with_capacity(count);
    if !objs.is_null() {
        for i in 0..count {
            let o = unsafe { *objs.add(i) };
            if !o.is_null() {
                retain(o);
                vec.push(o as usize);
            }
        }
    }
    store().lock().unwrap().insert(this as usize, Backing::Arr(vec));
    this
}
extern "C" fn arr_init_array(this: Id, _cmd: Sel, other: Id) -> Id {
    let v = if other.is_null() {
        Vec::new()
    } else if let Some(Backing::Arr(src)) = store().lock().unwrap().get(&(other as usize)) {
        src.clone()
    } else {
        Vec::new()
    };
    store().lock().unwrap().insert(this as usize, Backing::Arr(v));
    this
}
extern "C" fn arr_with_array(cls: Id, sel_id: Sel, other: Id) -> Id {
    let obj = unsafe { alloc_with_backing(cls as *const ClassT, Backing::Arr(Vec::new())) };
    arr_init_array(obj, sel_id, other)
}
extern "C" fn arr_init_cap(this: Id, _cmd: Sel, _cap: usize) -> Id {
    store().lock().unwrap().insert(this as usize, Backing::Arr(Vec::new()));
    this
}
extern "C" fn arr_with_cap(cls: Id, _cmd: Sel, _cap: usize) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Arr(Vec::new())) }
}
extern "C" fn arr_add_from_array(this: Id, _cmd: Sel, other: Id) {
    if other.is_null() { return; }
    let src = match store().lock().unwrap().get(&(other as usize)) {
        Some(Backing::Arr(v)) => v.clone(),
        _ => return,
    };
    if let Some(Backing::Arr(dst)) = store().lock().unwrap().get_mut(&(this as usize)) {
        dst.extend(src);
    }
}
extern "C" fn arr_joined_by(this: Id, _cmd: Sel, sep: Id) -> Id {
    let sep_str = rust_str_from_nsstring(sep).unwrap_or_default();
    let ptrs: Vec<usize> = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Arr(v)) => v.clone(),
        _ => Vec::new(),
    };
    let parts: Vec<String> = ptrs
        .into_iter()
        .filter_map(|p| rust_str_from_nsstring(p as Id))
        .collect();
    nsstring_from_rust(&parts.join(&sep_str))
}

unsafe fn collect_va_dict(first_val: Id, va: *const usize) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let vals_and_keys = collect_va_objects(first_val, va);
    for pair in vals_and_keys.chunks_exact(2) {
        let v = pair[0] as Id;
        let k = pair[1] as Id;
        if let Some(ks) = rust_str_from_nsstring(k) {
            map.insert(ks, v as usize);
        }
    }
    map
}
extern "C" fn dict_init_objs_keys_va(this: Id, _cmd: Sel, first: Id, va: *const usize) -> Id {
    let map = unsafe { collect_va_dict(first, va) };
    store().lock().unwrap().insert(this as usize, Backing::Dict(map));
    this
}
extern "C" fn dict_with_objs_keys_va(cls: Id, _cmd: Sel, first: Id, va: *const usize) -> Id {
    let map = unsafe { collect_va_dict(first, va) };
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(map)) }
}
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(naked)]
extern "C" fn darwin_dict_init_objs_keys() -> ! {
    core::arch::naked_asm!("mov x3, sp", "b {dst}", dst = sym dict_init_objs_keys_va);
}
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(naked)]
extern "C" fn darwin_dict_with_objs_keys() -> ! {
    core::arch::naked_asm!("mov x3, sp", "b {dst}", dst = sym dict_with_objs_keys_va);
}
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
extern "C" fn darwin_dict_init_objs_keys(this: Id, sel_id: Sel, first: Id) -> Id {
    dict_init_objs_keys_va(this, sel_id, first, std::ptr::null())
}
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
extern "C" fn darwin_dict_with_objs_keys(cls: Id, sel_id: Sel, first: Id) -> Id {
    dict_with_objs_keys_va(cls, sel_id, first, std::ptr::null())
}
extern "C" fn dict_init_cap(this: Id, _cmd: Sel, _cap: usize) -> Id {
    store().lock().unwrap().insert(this as usize, Backing::Dict(HashMap::new()));
    this
}
extern "C" fn dict_with_cap(cls: Id, _cmd: Sel, _cap: usize) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(HashMap::new())) }
}
extern "C" fn dict_init_dict(this: Id, _cmd: Sel, other: Id) -> Id {
    let m = if other.is_null() {
        HashMap::new()
    } else if let Some(Backing::Dict(src)) = store().lock().unwrap().get(&(other as usize)) {
        src.clone()
    } else {
        HashMap::new()
    };
    store().lock().unwrap().insert(this as usize, Backing::Dict(m));
    this
}
extern "C" fn dict_with_dict(cls: Id, sel_id: Sel, other: Id) -> Id {
    let d = unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(HashMap::new())) };
    dict_init_dict(d, sel_id, other)
}
fn plist_read_be_uint(b: &[u8]) -> usize {
    let mut v = 0usize;
    for &byte in b {
        v = (v << 8) | (byte as usize);
    }
    v
}

unsafe fn plist_parse_bplist_obj(
    data: &[u8],
    offsets: &[usize],
    ref_size: usize,
    idx: usize,
    depth: usize,
) -> Option<Id> {
    if depth > 32 || idx >= offsets.len() {
        return None;
    }
    let off = offsets[idx];
    if off >= data.len() {
        return None;
    }
    let marker = data[off];
    let hi = marker >> 4;
    let lo = marker & 0x0F;
    let mut pos = off + 1;
    let count = if hi >= 0x4 && lo == 0xF {
        if pos >= data.len() { return None; }
        let int_sz = 1usize << (data[pos] & 0x0F);
        pos += 1;
        if pos + int_sz > data.len() { return None; }
        let n = plist_read_be_uint(&data[pos..pos + int_sz]);
        pos += int_sz;
        n
    } else {
        lo as usize
    };
    match hi {
        0x0 => match lo {
            0x0 => Some(std::ptr::null_mut()),
            0x8 => Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(false))),
            0x9 => Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(true))),
            _ => None,
        },
        0x1 => {
            let sz = 1usize << (lo & 0x0F);
            if pos + sz > data.len() { return None; }
            let val = plist_read_be_uint(&data[pos..pos + sz]) as i64;
            Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumI64(val)))
        }
        0x2 => {
            let sz = 1usize << (lo & 0x0F);
            if pos + sz > data.len() { return None; }
            let val = if sz == 4 {
                f32::from_bits(plist_read_be_uint(&data[pos..pos + 4]) as u32) as f64
            } else if sz == 8 {
                f64::from_bits(plist_read_be_uint(&data[pos..pos + 8]) as u64)
            } else {
                0.0
            };
            Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumF64(val)))
        }
        0x4 => {
            let end = (pos + count).min(data.len());
            Some(alloc_with_backing(NSDATA_CLASS.ptr(), Backing::Bytes(data[pos..end].to_vec())))
        }
        0x5 => {
            let end = (pos + count).min(data.len());
            Some(nsstring_from_rust(&String::from_utf8_lossy(&data[pos..end])))
        }
        0x6 => {
            let end = (pos + count * 2).min(data.len());
            let mut u16s = Vec::with_capacity(count);
            for ch in data[pos..end].chunks_exact(2) {
                u16s.push(u16::from_be_bytes([ch[0], ch[1]]));
            }
            Some(nsstring_from_rust(&String::from_utf16_lossy(&u16s)))
        }
        0xA | 0xC => {
            let mut vec = Vec::with_capacity(count.min(4096));
            for i in 0..count.min(4096) {
                if pos + (i + 1) * ref_size > data.len() { break; }
                let r = plist_read_be_uint(&data[pos + i * ref_size..pos + (i + 1) * ref_size]);
                if let Some(o) = plist_parse_bplist_obj(data, offsets, ref_size, r, depth + 1) {
                    if !o.is_null() { vec.push(retain(o) as usize); }
                }
            }
            Some(alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec)))
        }
        0xD => {
            let mut map = HashMap::new();
            let kbase = pos;
            let vbase = pos + count * ref_size;
            for i in 0..count.min(4096) {
                if vbase + (i + 1) * ref_size > data.len() { break; }
                let kr = plist_read_be_uint(&data[kbase + i * ref_size..kbase + (i + 1) * ref_size]);
                let vr = plist_read_be_uint(&data[vbase + i * ref_size..vbase + (i + 1) * ref_size]);
                let kobj = plist_parse_bplist_obj(data, offsets, ref_size, kr, depth + 1);
                let vobj = plist_parse_bplist_obj(data, offsets, ref_size, vr, depth + 1);
                if let (Some(k), Some(v)) = (kobj.and_then(rust_str_from_nsstring), vobj) {
                    if !v.is_null() {
                        map.insert(k, retain(v) as usize);
                    }
                }
            }
            Some(alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map)))
        }
        _ => None,
    }
}

fn plist_parse_xml_tag<'a>(s: &'a str, i: &mut usize) -> Option<&'a str> {
    let lt = s[*i..].find('<')? + *i;
    let gt = s[lt + 1..].find('>')? + lt + 1;
    *i = gt + 1;
    Some(s[lt + 1..gt].trim())
}

fn plist_xml_text_until<'a>(s: &'a str, i: &mut usize, end_tag: &str) -> String {
    let close = format!("</{end_tag}>");
    if let Some(pos) = s[*i..].find(&close) {
        let body = &s[*i..*i + pos];
        *i += pos + close.len();
        body.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
    } else {
        *i = s.len();
        String::new()
    }
}

unsafe fn plist_parse_xml_node(s: &str, i: &mut usize, depth: usize) -> Option<Id> {
    if depth > 32 { return None; }
    loop {
        let tag = plist_parse_xml_tag(s, i)?;
        if tag.starts_with('?') || tag.starts_with('!') || tag.starts_with("plist") || tag == "/plist" {
            continue;
        }
        if tag.starts_with('/') {
            return None;
        }
        let name = tag.split_whitespace().next().unwrap_or(tag).trim_end_matches('/');
        match name {
            "true" => return Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(true))),
            "false" => return Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(false))),
            "string" => {
                if tag.ends_with('/') { return Some(nsstring_from_rust("")); }
                return Some(nsstring_from_rust(&plist_xml_text_until(s, i, "string")));
            }
            "integer" => {
                let v = plist_xml_text_until(s, i, "integer").trim().parse::<i64>().unwrap_or(0);
                return Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumI64(v)));
            }
            "real" => {
                let v = plist_xml_text_until(s, i, "real").trim().parse::<f64>().unwrap_or(0.0);
                return Some(alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumF64(v)));
            }
            "data" => {
                let _ = plist_xml_text_until(s, i, "data");
                return Some(alloc_with_backing(NSDATA_CLASS.ptr(), Backing::Bytes(Vec::new())));
            }
            "date" => {
                let _ = plist_xml_text_until(s, i, "date");
                return Some(nsstring_from_rust("1970-01-01T00:00:00Z"));
            }
            "array" => {
                let mut vec = Vec::new();
                if !tag.ends_with('/') {
                    while let Some(v) = plist_parse_xml_node(s, i, depth + 1) {
                        if !v.is_null() { vec.push(retain(v) as usize); }
                    }
                }
                return Some(alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec)));
            }
            "dict" => {
                let mut map = HashMap::new();
                if !tag.ends_with('/') {
                    while let Some(kt) = plist_parse_xml_tag(s, i) {
                        if kt == "/dict" { break; }
                        if kt.starts_with("key") {
                            let k = plist_xml_text_until(s, i, "key");
                            if let Some(v) = plist_parse_xml_node(s, i, depth + 1) {
                                if !v.is_null() { map.insert(k, retain(v) as usize); }
                            }
                        }
                    }
                }
                return Some(alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map)));
            }
            _ => {
                if !tag.ends_with('/') {
                    let _ = plist_xml_text_until(s, i, name);
                }
            }
        }
    }
}

fn plist_parse_bytes(data: &[u8]) -> Option<Id> {
    if data.starts_with(b"bplist00") && data.len() >= 40 {
        let tr = &data[data.len() - 32..];
        let off_sz = tr[6] as usize;
        let ref_sz = tr[7] as usize;
        let num_objs = plist_read_be_uint(&tr[8..16]);
        let top_obj = plist_read_be_uint(&tr[16..24]);
        let off_tbl = plist_read_be_uint(&tr[24..32]);
        if off_sz > 0 && ref_sz > 0 && off_tbl + num_objs * off_sz <= data.len() - 32 {
            let mut offsets = Vec::with_capacity(num_objs.min(65536));
            for i in 0..num_objs.min(65536) {
                offsets.push(plist_read_be_uint(&data[off_tbl + i * off_sz..off_tbl + (i + 1) * off_sz]));
            }
            return unsafe { plist_parse_bplist_obj(data, &offsets, ref_sz, top_obj, 0) };
        }
        return None;
    }
    let s = std::str::from_utf8(data).ok()?;
    let trimmed = s.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let mut idx = 0;
        return unsafe { json_parse_val(trimmed.as_bytes(), &mut idx) };
    }
    if trimmed.starts_with('<') {
        let mut idx = 0;
        return unsafe { plist_parse_xml_node(trimmed, &mut idx, 0) };
    }
    None
}

fn path_from_str_or_url(obj: Id) -> Option<String> {
    let s = rust_str_from_nsstring(obj)?;
    Some(s.strip_prefix("file://").unwrap_or(&s).to_string())
}

extern "C" fn dict_with_file(_cls: Id, _cmd: Sel, path: Id) -> Id {
    let Some(p) = path_from_str_or_url(path) else { return std::ptr::null_mut(); };
    let Ok(bytes) = std::fs::read(&p) else { return std::ptr::null_mut(); };
    plist_parse_bytes(&bytes).unwrap_or(std::ptr::null_mut())
}
extern "C" fn dict_with_url_err(cls: Id, sel_id: Sel, url: Id, err: *mut Id) -> Id {
    let res = dict_with_file(cls, sel_id, url);
    if res.is_null() && !err.is_null() {
        unsafe { *err = std::ptr::null_mut(); }
    }
    res
}
extern "C" fn dict_init_with_file(this: Id, sel_id: Sel, path: Id) -> Id {
    let _ = this;
    dict_with_file(NSDICTIONARY_CLASS.ptr() as Id, sel_id, path)
}
extern "C" fn dict_init_with_url_err(this: Id, sel_id: Sel, url: Id, err: *mut Id) -> Id {
    let _ = this;
    dict_with_url_err(NSDICTIONARY_CLASS.ptr() as Id, sel_id, url, err)
}
extern "C" fn dict_write_to_file(_this: Id, _cmd: Sel, _path: Id, _atom: bool) -> bool { true }
extern "C" fn dict_write_to_url_err(_this: Id, _cmd: Sel, _url: Id, _err: *mut Id) -> bool { true }

extern "C" fn dict_add_entries(this: Id, _cmd: Sel, other: Id) {
    if other.is_null() { return; }
    let src = match store().lock().unwrap().get(&(other as usize)) {
        Some(Backing::Dict(m)) => m.clone(),
        _ => return,
    };
    if let Some(Backing::Dict(dst)) = store().lock().unwrap().get_mut(&(this as usize)) {
        for (k, v) in src {
            dst.insert(k, v);
        }
    }
}

#[repr(C)]
struct NSFastEnumerationState {
    state: usize,
    items_ptr: *mut Id,
    mutations_ptr: *mut usize,
    extra: [usize; 5],
}

static mut FAST_ENUM_MUTATION: usize = 0;

extern "C" fn fast_enum_arr(
    this: Id,
    _: Sel,
    state: *mut NSFastEnumerationState,
    stackbuf: *mut Id,
    len: usize,
) -> usize {
    if state.is_null() || stackbuf.is_null() || len == 0 {
        return 0;
    }
    unsafe {
        let pos = (*state).state;
        let items: Vec<usize> = match store().lock().unwrap().get(&(this as usize)) {
            Some(Backing::Arr(v)) => v.clone(),
            _ => Vec::new(),
        };
        if pos >= items.len() {
            return 0;
        }
        let n = (items.len() - pos).min(len);
        for i in 0..n {
            *stackbuf.add(i) = items[pos + i] as Id;
        }
        (*state).state = pos + n;
        (*state).items_ptr = stackbuf;
        (*state).mutations_ptr = &raw mut FAST_ENUM_MUTATION;
        n
    }
}

extern "C" fn fast_enum_dict(
    this: Id,
    _: Sel,
    state: *mut NSFastEnumerationState,
    stackbuf: *mut Id,
    len: usize,
) -> usize {
    if state.is_null() || stackbuf.is_null() || len == 0 {
        return 0;
    }
    unsafe {
        let pos = (*state).state;
        let keys: Vec<String> = match store().lock().unwrap().get(&(this as usize)) {
            Some(Backing::Dict(m)) => m.keys().cloned().collect(),
            _ => Vec::new(),
        };
        if pos >= keys.len() {
            return 0;
        }
        let n = (keys.len() - pos).min(len);
        for i in 0..n {
            *stackbuf.add(i) = nsstring_from_rust(&keys[pos + i]);
        }
        (*state).state = pos + n;
        (*state).items_ptr = stackbuf;
        (*state).mutations_ptr = &raw mut FAST_ENUM_MUTATION;
        n
    }
}

// ---- NSDictionary / NSMutableDictionary ------------------------------------

extern "C" fn dict_new(cls: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(HashMap::new())) }
}

extern "C" fn dict_with_objs_keys(
    cls: Id,
    _: Sel,
    objs: *const Id,
    keys: *const Id,
    count: usize,
) -> Id {
    let mut map = HashMap::new();
    if !objs.is_null() && !keys.is_null() {
        for i in 0..count {
            let k_obj = unsafe { *keys.add(i) };
            let v_obj = unsafe { *objs.add(i) };
            if let Some(k) = rust_str_from_nsstring(k_obj) {
                if !v_obj.is_null() {
                    retain(v_obj);
                    map.insert(k, v_obj as usize);
                }
            }
        }
    }
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(map)) }
}

extern "C" fn dict_with_obj_key(cls: Id, _: Sel, obj: Id, key: Id) -> Id {
    let mut map = HashMap::new();
    if let Some(k) = rust_str_from_nsstring(key) {
        if !obj.is_null() {
            retain(obj);
            map.insert(k, obj as usize);
        }
    }
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(map)) }
}

extern "C" fn dict_all_keys(this: Id, _: Sel) -> Id {
    let key_names: Vec<String> = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Dict(m)) => m.keys().cloned().collect(),
        _ => Vec::new(),
    };
    let keys: Vec<usize> = key_names.iter().map(|k| nsstring_from_rust(k) as usize).collect();
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(keys)) }
}

extern "C" fn dict_all_values(this: Id, _: Sel) -> Id {
    let vals: Vec<usize> = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Dict(m)) => m.values().copied().collect(),
        _ => Vec::new(),
    };
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vals)) }
}

extern "C" fn dict_remove_key(this: Id, _: Sel, key: Id) {
    if let Some(k) = rust_str_from_nsstring(key) {
        if let Some(Backing::Dict(m)) = store().lock().unwrap().get_mut(&(this as usize)) {
            m.remove(&k);
        }
    }
}

extern "C" fn dict_remove_all(this: Id, _: Sel) {
    if let Some(Backing::Dict(m)) = store().lock().unwrap().get_mut(&(this as usize)) {
        m.clear();
    }
}

extern "C" fn dict_copy(this: Id, _: Sel) -> Id {
    let m = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Dict(d)) => d.clone(),
        _ => HashMap::new(),
    };
    unsafe { alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(m)) }
}

extern "C" fn dict_mcopy(this: Id, _: Sel) -> Id {
    let m = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Dict(d)) => d.clone(),
        _ => HashMap::new(),
    };
    unsafe { alloc_with_backing(NSMUTABLEDICTIONARY_CLASS.ptr(), Backing::Dict(m)) }
}

extern "C" fn dict_copy_zone(this: Id, sel_id: Sel, _: *mut c_void) -> Id {
    dict_copy(this, sel_id)
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
extern "C" fn num_f32(cls: Id, _: Sel, v: f32) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::NumF64(v as f64)) }
}
extern "C" fn num_u32(cls: Id, _: Sel, v: u32) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::NumI64(v as i64)) }
}
extern "C" fn num_as_i32(this: Id, _: Sel) -> i32 {
    num_as_i64(this, sel("longLongValue")) as i32
}
extern "C" fn num_as_u32(this: Id, _: Sel) -> u32 {
    num_as_i64(this, sel("longLongValue")) as u32
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
extern "C" fn num_as_f32(this: Id, _: Sel) -> f32 {
    num_as_f64(this, sel("doubleValue")) as f32
}
extern "C" fn num_as_bool(this: Id, _: Sel) -> bool {
    num_as_i64(this, sel("longLongValue")) != 0
}
extern "C" fn num_string_value(this: Id, _: Sel) -> Id {
    let s = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::NumI64(v)) => v.to_string(),
        Some(Backing::NumF64(v)) => v.to_string(),
        Some(Backing::NumBool(b)) => if *b { "1".into() } else { "0".into() },
        _ => "0".into(),
    };
    nsstring_from_rust(&s)
}
extern "C" fn num_is_equal(this: Id, _: Sel, other: Id) -> bool {
    if this == other { return true; }
    if other.is_null() { return false; }
    num_as_f64(this, sel("doubleValue")) == num_as_f64(other, sel("doubleValue"))
}
extern "C" fn num_compare(this: Id, _: Sel, other: Id) -> isize {
    let a = num_as_f64(this, sel("doubleValue"));
    let b = if other.is_null() { 0.0 } else { num_as_f64(other, sel("doubleValue")) };
    if a < b { -1 } else if a > b { 1 } else { 0 }
}
extern "C" fn copy_retain(this: Id, _: Sel) -> Id {
    retain(this)
}
extern "C" fn copy_zone_retain(this: Id, _: Sel, _: *mut c_void) -> Id {
    retain(this)
}
extern "C" fn data_with_file(cls: Id, _: Sel, path: Id) -> Id {
    let Some(p) = path_from_str_or_url(path) else { return std::ptr::null_mut(); };
    let Ok(bytes) = std::fs::read(&p) else { return std::ptr::null_mut(); };
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Bytes(bytes)) }
}
extern "C" fn data_with_file_opts_err(cls: Id, sel_id: Sel, path: Id, _opts: usize, err: *mut Id) -> Id {
    let res = data_with_file(cls, sel_id, path);
    if res.is_null() && !err.is_null() {
        unsafe { *err = std::ptr::null_mut(); }
    }
    res
}
extern "C" fn data_init_with_file(this: Id, sel_id: Sel, path: Id) -> Id {
    let _ = this;
    data_with_file(NSDATA_CLASS.ptr() as Id, sel_id, path)
}
extern "C" fn data_write_file(this: Id, _: Sel, path: Id, _atomically: bool) -> bool {
    let Some(p) = rust_str_from_nsstring(path) else { return false; };
    let guard = store().lock().unwrap();
    if let Some(Backing::Bytes(b)) = guard.get(&(this as usize)) {
        return std::fs::write(&p, b).is_ok();
    }
    false
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
fn bundle_base_dir() -> String {
    std::env::var("PEAR_BUNDLE_PATH")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned())
}
extern "C" fn bundle_path(_this: Id, _: Sel) -> Id {
    nsstring_from_rust(&bundle_base_dir())
}
extern "C" fn bundle_id(_this: Id, _: Sel) -> Id {
    nsstring_from_rust("com.pearruntime.app")
}
extern "C" fn bundle_exec_path(_this: Id, _: Sel) -> Id {
    let p = std::path::Path::new(&bundle_base_dir()).join("App");
    nsstring_from_rust(&p.to_string_lossy())
}
extern "C" fn bundle_url(_this: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(bundle_base_dir())) }
}
extern "C" fn bundle_receipt_url(_this: Id, _: Sel) -> Id {
    let p = std::path::Path::new(&bundle_base_dir()).join("StoreKit/receipt");
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(p.to_string_lossy().into_owned())) }
}
extern "C" fn bundle_localized_string(_this: Id, _: Sel, key: Id, value: Id, _table: Id) -> Id {
    if let Some(v) = rust_str_from_nsstring(value).filter(|s| !s.is_empty()) {
        return nsstring_from_rust(&v);
    }
    nsstring_from_rust(&rust_str_from_nsstring(key).unwrap_or_default())
}
extern "C" fn bundle_localizations(_this: Id, _: Sel) -> Id {
    let en = nsstring_from_rust("en");
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec![retain(en) as usize])) }
}
extern "C" fn bundle_for_arg(cls: Id, _: Sel, _arg: Id) -> Id {
    bundle_main(cls, sel("mainBundle"))
}
extern "C" fn bundle_info(_this: Id, _: Sel) -> Id {
    static INFO: OnceLock<usize> = OnceLock::new();
    *INFO.get_or_init(|| {
        let plist_path = std::path::Path::new(&bundle_base_dir()).join("Info.plist");
        if let Ok(bytes) = std::fs::read(&plist_path) {
            if let Some(d) = plist_parse_bytes(&bytes) {
                if !d.is_null() {
                    return retain(d) as usize;
                }
            }
        }
        dict_new(NSDICTIONARY_CLASS.ptr() as Id, sel("dictionary")) as usize
    }) as Id
}
extern "C" fn bundle_obj_for_info(this: Id, _: Sel, key: Id) -> Id {
    let d = bundle_info(this, sel("infoDictionary"));
    dict_get(d, sel("objectForKey:"), key)
}
fn resolve_bundle_file(sub: Option<&str>, name: Id, ext: Id) -> Option<String> {
    let mut dir = std::path::PathBuf::from(bundle_base_dir());
    if let Some(s) = sub.filter(|s| !s.is_empty()) {
        dir = dir.join(s);
    }
    let n = rust_str_from_nsstring(name).unwrap_or_default();
    let e = rust_str_from_nsstring(ext).unwrap_or_default();
    let fname = if e.is_empty() { n } else { format!("{n}.{e}") };
    let full = dir.join(fname);
    full.exists().then(|| full.to_string_lossy().into_owned())
}
extern "C" fn bundle_path_for_res(_this: Id, _: Sel, name: Id, ext: Id) -> Id {
    resolve_bundle_file(None, name, ext)
        .map(|s| nsstring_from_rust(&s))
        .unwrap_or(std::ptr::null_mut())
}
extern "C" fn bundle_path_for_res_in_dir(_this: Id, _: Sel, name: Id, ext: Id, dir: Id) -> Id {
    let sub = rust_str_from_nsstring(dir);
    resolve_bundle_file(sub.as_deref(), name, ext)
        .map(|s| nsstring_from_rust(&s))
        .unwrap_or(std::ptr::null_mut())
}
extern "C" fn bundle_url_for_res(_this: Id, _: Sel, name: Id, ext: Id) -> Id {
    resolve_bundle_file(None, name, ext)
        .map(|s| unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(s)) })
        .unwrap_or(std::ptr::null_mut())
}
extern "C" fn bundle_url_for_res_in_dir(_this: Id, _: Sel, name: Id, ext: Id, dir: Id) -> Id {
    let sub = rust_str_from_nsstring(dir);
    resolve_bundle_file(sub.as_deref(), name, ext)
        .map(|s| unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(s)) })
        .unwrap_or(std::ptr::null_mut())
}
extern "C" fn bundle_paths_for_res(_this: Id, _: Sel, _ext: Id, _dir: Id) -> Id {
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(Vec::new())) }
}
extern "C" fn nc_remove_obs(_this: Id, _: Sel, _obs: Id) {}
extern "C" fn nc_remove_obs_name_obj(_this: Id, _: Sel, _obs: Id, _name: Id, _obj: Id) {}
extern "C" fn nc_post_no_info(this: Id, _: Sel, name: Id, obj: Id) {
    nc_post(this, sel("postNotificationName:object:userInfo:"), name, obj, std::ptr::null_mut());
}
extern "C" fn nc_add_obs_block(
    _this: Id,
    _: Sel,
    _name: Id,
    _obj: Id,
    _queue: Id,
    _block: Id,
) -> Id {
    unsafe { pear_libobjc::object::alloc(NSOBJECT_CLASS.ptr()) }
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

const APPLE_EPOCH_DELTA: f64 = 978_307_200.0;

fn date_secs(obj: Id) -> f64 {
    store()
        .lock()
        .unwrap()
        .get(&(obj as usize))
        .map_or(0.0, |b| match b {
            Backing::Date(s) => *s,
            _ => 0.0,
        })
}

extern "C" fn date_since_1970(this: Id, _: Sel) -> f64 {
    date_secs(this)
}
extern "C" fn date_since_ref(this: Id, _: Sel) -> f64 {
    date_secs(this) - APPLE_EPOCH_DELTA
}
extern "C" fn date_cls_since_ref(_cls: Id, _: Sel) -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
        - APPLE_EPOCH_DELTA
}
extern "C" fn date_since_now(this: Id, _: Sel) -> f64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    date_secs(this) - now
}
extern "C" fn date_since_date(this: Id, _: Sel, other: Id) -> f64 {
    date_secs(this) - date_secs(other)
}
extern "C" fn date_adding(this: Id, _: Sel, dt: f64) -> Id {
    unsafe { alloc_with_backing(NSDATE_CLASS.ptr(), Backing::Date(date_secs(this) + dt)) }
}
extern "C" fn date_with_1970(cls: Id, _: Sel, secs: f64) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Date(secs)) }
}
extern "C" fn date_with_now(cls: Id, _: Sel, dt: f64) -> Id {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Date(now + dt)) }
}
extern "C" fn date_with_ref(cls: Id, _: Sel, secs: f64) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Date(secs + APPLE_EPOCH_DELTA)) }
}
extern "C" fn date_distant_past(cls: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Date(-62_135_769_600.0)) }
}
extern "C" fn date_distant_future(cls: Id, _: Sel) -> Id {
    unsafe { alloc_with_backing(cls as *const ClassT, Backing::Date(64_092_211_200.0)) }
}
extern "C" fn date_is_equal(this: Id, _: Sel, other: Id) -> bool {
    (date_secs(this) - date_secs(other)).abs() < 1e-6
}
extern "C" fn date_compare(this: Id, _: Sel, other: Id) -> isize {
    let a = date_secs(this);
    let b = date_secs(other);
    if a < b { -1 } else if a > b { 1 } else { 0 }
}
extern "C" fn date_copy_zone(this: Id, _: Sel, _: *mut c_void) -> Id {
    unsafe { alloc_with_backing(NSDATE_CLASS.ptr(), Backing::Date(date_secs(this))) }
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
                ("fileSystemRepresentation", i(str_utf8 as *const ())),
                ("cStringUsingEncoding:", i(str_utf8 as *const ())),
                ("canBeConvertedToEncoding:", i(str_can_be_converted as *const ())),
                ("getCString:maxLength:encoding:", i(str_get_cstring as *const ())),
                ("dataUsingEncoding:", i(str_data_using_encoding as *const ())),
                ("dataUsingEncoding:allowLossyConversion:", i(str_data_lossy as *const ())),
                ("stringByStandardizingPath", i(str_identity_path as *const ())),
                ("stringByResolvingSymlinksInPath", i(str_identity_path as *const ())),
                ("stringByExpandingTildeInPath", i(str_expand_tilde as *const ())),
                ("initWithCString:encoding:", i(str_init_cstring_enc as *const ())),
                ("initWithData:encoding:", i(str_init_data_enc as *const ())),
                ("length", i(str_length as *const ())),
                ("characterAtIndex:", i(str_char_at as *const ())),
                ("getCharacters:range:", i(str_get_chars as *const ())),
                ("isEqualToString:", i(str_is_equal as *const ())),
                ("isEqual:", i(str_is_equal as *const ())),
                ("hasPrefix:", i(str_has_prefix as *const ())),
                ("hasSuffix:", i(str_has_suffix as *const ())),
                ("containsString:", i(str_contains_string as *const ())),
                ("localizedCaseInsensitiveContainsString:", i(str_localized_ci_contains as *const ())),
                ("rangeOfString:options:", i(str_range_of_opts as *const ())),
                ("rangeOfCharacterFromSet:", i(str_range_of_charset as *const ())),
                ("rangeOfCharacterFromSet:options:", i(str_range_of_charset_opts as *const ())),
                ("characterAtIndex:", i(str_char_at as *const ())),
                ("getCharacters:range:", i(str_get_chars as *const ())),
                ("dataUsingEncoding:allowLossyConversion:", i(str_data_lossy as *const ())),
                ("stringByAppendingString:", i(str_append_str as *const ())),
                ("stringByAppendingPathComponent:", i(str_append_path as *const ())),
                ("pathComponents", i(str_path_components as *const ())),
                ("lastPathComponent", i(str_last_path_comp as *const ())),
                ("stringByDeletingLastPathComponent", i(str_del_last_path_comp as *const ())),
                ("pathExtension", i(str_path_ext as *const ())),
                ("stringByDeletingPathExtension", i(str_del_path_ext as *const ())),
                ("stringByAppendingPathExtension:", i(str_append_path_ext as *const ())),
                ("stringByReplacingOccurrencesOfString:withString:", i(str_replace_occ as *const ())),
                ("stringByTrimmingCharactersInSet:", i(str_trim_chars as *const ())),
                ("substringFromIndex:", i(str_substr_from as *const ())),
                ("substringToIndex:", i(str_substr_to as *const ())),
                ("substringWithRange:", i(str_substr_range as *const ())),
                ("compare:", i(str_compare as *const ())),
                ("compare:options:", i(str_compare_opts as *const ())),
                ("compare:options:range:", i(str_compare_opts_range as *const ())),
                ("localizedCompare:", i(str_compare as *const ())),
                ("localizedCaseInsensitiveCompare:", i(str_case_compare as *const ())),
                ("caseInsensitiveCompare:", i(str_case_compare as *const ())),
                ("copy", i(copy_retain as *const ())),
                ("lowercaseString", i(str_lowercase as *const ())),
                ("uppercaseString", i(str_uppercase as *const ())),
                ("rangeOfString:", i(str_range_of as *const ())),
                ("componentsSeparatedByString:", i(str_components_sep as *const ())),
                ("intValue", i(str_int_val as *const ())),
                ("integerValue", i(str_integer_val as *const ())),
                ("doubleValue", i(str_double_val as *const ())),
                ("floatValue", i(str_float_val as *const ())),
                ("boolValue", i(str_bool_val as *const ())),
                ("description", i(str_description as *const ())),
                ("hash", i(str_hash as *const ())),
                ("initWithUTF8String:", i(str_init_utf8 as *const ())),
                ("initWithString:", i(str_init_with_string as *const ())),
                ("initWithFormat:", i(str_init_with_format as *const ())),
                ("initWithFormat:arguments:", i(str_init_with_format_args as *const ())),
                ("initWithFormat:locale:arguments:", i(str_init_with_format_loc_args as *const ())),
                ("copyWithZone:", i(str_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("string", i(str_empty as *const ())),
                ("stringWithString:", i(str_with_string as *const ())),
                ("stringWithUTF8String:", i(str_with_utf8 as *const ())),
                ("stringWithCString:encoding:", i(str_with_cstring_enc as *const ())),
                ("stringWithFormat:", i(str_with_format as *const ())),
                ("localizedStringWithFormat:", i(str_with_format as *const ())),
                ("pathWithComponents:", i(str_path_with_components as *const ())),
            ],
        );
        runtime::register_builtin(
            NSMUTABLESTRING_CLASS.ptr(),
            NSMUTABLESTRING_META.ptr(),
            "NSMutableString",
            16,
            &[
                ("UTF8String", i(str_utf8 as *const ())),
                ("fileSystemRepresentation", i(str_utf8 as *const ())),
                ("cStringUsingEncoding:", i(str_utf8 as *const ())),
                ("canBeConvertedToEncoding:", i(str_can_be_converted as *const ())),
                ("getCString:maxLength:encoding:", i(str_get_cstring as *const ())),
                ("dataUsingEncoding:", i(str_data_using_encoding as *const ())),
                ("dataUsingEncoding:allowLossyConversion:", i(str_data_lossy as *const ())),
                ("stringByStandardizingPath", i(str_identity_path as *const ())),
                ("stringByResolvingSymlinksInPath", i(str_identity_path as *const ())),
                ("stringByExpandingTildeInPath", i(str_expand_tilde as *const ())),
                ("initWithCString:encoding:", i(str_init_cstring_enc as *const ())),
                ("initWithData:encoding:", i(str_init_data_enc as *const ())),
                ("length", i(str_length as *const ())),
                ("characterAtIndex:", i(str_char_at as *const ())),
                ("getCharacters:range:", i(str_get_chars as *const ())),
                ("isEqualToString:", i(str_is_equal as *const ())),
                ("isEqual:", i(str_is_equal as *const ())),
                ("hasPrefix:", i(str_has_prefix as *const ())),
                ("hasSuffix:", i(str_has_suffix as *const ())),
                ("containsString:", i(str_contains_string as *const ())),
                ("localizedCaseInsensitiveContainsString:", i(str_localized_ci_contains as *const ())),
                ("rangeOfString:options:", i(str_range_of_opts as *const ())),
                ("rangeOfCharacterFromSet:", i(str_range_of_charset as *const ())),
                ("rangeOfCharacterFromSet:options:", i(str_range_of_charset_opts as *const ())),
                ("characterAtIndex:", i(str_char_at as *const ())),
                ("getCharacters:range:", i(str_get_chars as *const ())),
                ("dataUsingEncoding:allowLossyConversion:", i(str_data_lossy as *const ())),
                ("stringByAppendingString:", i(str_append_str as *const ())),
                ("stringByAppendingPathComponent:", i(str_append_path as *const ())),
                ("pathComponents", i(str_path_components as *const ())),
                ("lastPathComponent", i(str_last_path_comp as *const ())),
                ("stringByDeletingLastPathComponent", i(str_del_last_path_comp as *const ())),
                ("pathExtension", i(str_path_ext as *const ())),
                ("stringByDeletingPathExtension", i(str_del_path_ext as *const ())),
                ("stringByAppendingPathExtension:", i(str_append_path_ext as *const ())),
                ("stringByReplacingOccurrencesOfString:withString:", i(str_replace_occ as *const ())),
                ("stringByTrimmingCharactersInSet:", i(str_trim_chars as *const ())),
                ("substringFromIndex:", i(str_substr_from as *const ())),
                ("substringToIndex:", i(str_substr_to as *const ())),
                ("substringWithRange:", i(str_substr_range as *const ())),
                ("compare:", i(str_compare as *const ())),
                ("compare:options:", i(str_compare_opts as *const ())),
                ("compare:options:range:", i(str_compare_opts_range as *const ())),
                ("localizedCompare:", i(str_compare as *const ())),
                ("localizedCaseInsensitiveCompare:", i(str_case_compare as *const ())),
                ("caseInsensitiveCompare:", i(str_case_compare as *const ())),
                ("copy", i(copy_retain as *const ())),
                ("appendString:", i(str_append as *const ())),
                ("initWithUTF8String:", i(str_init_utf8 as *const ())),
                ("initWithString:", i(str_init_with_string as *const ())),
                ("initWithFormat:", i(str_init_with_format as *const ())),
                ("initWithFormat:arguments:", i(str_init_with_format_args as *const ())),
                ("initWithFormat:locale:arguments:", i(str_init_with_format_loc_args as *const ())),
                ("copyWithZone:", i(str_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("string", i(str_empty as *const ())),
                ("stringWithString:", i(str_with_string as *const ())),
                ("stringWithUTF8String:", i(str_with_utf8 as *const ())),
                ("stringWithCString:encoding:", i(str_with_cstring_enc as *const ())),
                ("stringWithFormat:", i(str_with_format as *const ())),
                ("localizedStringWithFormat:", i(str_with_format as *const ())),
                ("pathWithComponents:", i(str_path_with_components as *const ())),
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
                ("objectAtIndexedSubscript:", i(arr_at as *const ())),
                ("firstObject", i(arr_first as *const ())),
                ("lastObject", i(arr_last as *const ())),
                ("containsObject:", i(arr_contains as *const ())),
                ("indexOfObject:", i(arr_index_of as *const ())),
                ("subarrayWithRange:", i(arr_subarray as *const ())),
                ("isEqualToArray:", i(arr_is_equal as *const ())),
                ("isEqual:", i(arr_is_equal as *const ())),
                ("componentsJoinedByString:", i(arr_joined_by as *const ())),
                ("initWithObjects:", i(darwin_arr_init_objs as *const ())),
                ("initWithObjects:count:", i(arr_init_count as *const ())),
                ("initWithArray:", i(arr_init_array as *const ())),
                ("initWithContentsOfFile:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:error:", i(dict_init_with_url_err as *const ())),
                ("writeToFile:atomically:", i(dict_write_to_file as *const ())),
                ("writeToURL:error:", i(dict_write_to_url_err as *const ())),
                ("countByEnumeratingWithState:objects:count:", i(fast_enum_arr as *const ())),
                ("objectEnumerator", i(col_object_enumerator as *const ())),
                ("reverseObjectEnumerator", i(arr_reverse_object_enumerator as *const ())),
                ("copy", i(arr_copy as *const ())),
                ("mutableCopy", i(arr_mcopy as *const ())),
                ("copyWithZone:", i(arr_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("array", i(arr_new as *const ())),
                ("arrayWithObject:", i(arr_with_obj as *const ())),
                ("arrayWithObjects:", i(darwin_arr_with_objs as *const ())),
                ("arrayWithObjects:count:", i(arr_with_objs as *const ())),
                ("arrayWithArray:", i(arr_with_array as *const ())),
                ("arrayWithContentsOfFile:", i(dict_with_file as *const ())),
                ("arrayWithContentsOfURL:", i(dict_with_file as *const ())),
                ("arrayWithContentsOfURL:error:", i(dict_with_url_err as *const ())),
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
                ("objectAtIndexedSubscript:", i(arr_at as *const ())),
                ("firstObject", i(arr_first as *const ())),
                ("lastObject", i(arr_last as *const ())),
                ("containsObject:", i(arr_contains as *const ())),
                ("indexOfObject:", i(arr_index_of as *const ())),
                ("subarrayWithRange:", i(arr_subarray as *const ())),
                ("isEqualToArray:", i(arr_is_equal as *const ())),
                ("isEqual:", i(arr_is_equal as *const ())),
                ("componentsJoinedByString:", i(arr_joined_by as *const ())),
                ("initWithObjects:", i(darwin_arr_init_objs as *const ())),
                ("initWithObjects:count:", i(arr_init_count as *const ())),
                ("initWithArray:", i(arr_init_array as *const ())),
                ("initWithContentsOfFile:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:error:", i(dict_init_with_url_err as *const ())),
                ("writeToFile:atomically:", i(dict_write_to_file as *const ())),
                ("writeToURL:error:", i(dict_write_to_url_err as *const ())),
                ("initWithCapacity:", i(arr_init_cap as *const ())),
                ("addObject:", i(arr_add as *const ())),
                ("addObjectsFromArray:", i(arr_add_from_array as *const ())),
                ("setObject:atIndexedSubscript:", i(arr_set_subscript as *const ())),
                ("replaceObjectAtIndex:withObject:", i(arr_replace_at as *const ())),
                ("insertObject:atIndex:", i(arr_insert_at as *const ())),
                ("exchangeObjectAtIndex:withObjectAtIndex:", i(arr_exchange as *const ())),
                ("subarrayWithRange:", i(arr_subarray as *const ())),
                ("isEqualToArray:", i(arr_is_equal as *const ())),
                ("isEqual:", i(arr_is_equal as *const ())),
                ("countByEnumeratingWithState:objects:count:", i(fast_enum_arr as *const ())),
                ("objectEnumerator", i(col_object_enumerator as *const ())),
                ("reverseObjectEnumerator", i(arr_reverse_object_enumerator as *const ())),
                ("removeObject:", i(arr_remove_obj as *const ())),
                ("removeLastObject", i(arr_remove_last as *const ())),
                ("removeObjectAtIndex:", i(arr_remove_at as *const ())),
                ("removeAllObjects", i(arr_remove_all as *const ())),
                ("copy", i(arr_copy as *const ())),
                ("mutableCopy", i(arr_mcopy as *const ())),
                ("copyWithZone:", i(arr_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("array", i(arr_new as *const ())),
                ("arrayWithObject:", i(arr_with_obj as *const ())),
                ("arrayWithObjects:", i(darwin_arr_with_objs as *const ())),
                ("arrayWithObjects:count:", i(arr_with_objs as *const ())),
                ("arrayWithArray:", i(arr_with_array as *const ())),
                ("arrayWithCapacity:", i(arr_with_cap as *const ())),
            ],
        );
        runtime::register_builtin(
            NSDICTIONARY_CLASS.ptr(),
            NSDICTIONARY_META.ptr(),
            "NSDictionary",
            16,
            &[
                ("count", i(dict_count as *const ())),
                ("objectForKey:", i(dict_get as *const ())),
                ("valueForKey:", i(dict_get as *const ())),
                ("objectForKeyedSubscript:", i(dict_get as *const ())),
                ("allKeys", i(dict_all_keys as *const ())),
                ("allValues", i(dict_all_values as *const ())),
                ("initWithObjectsAndKeys:", i(darwin_dict_init_objs_keys as *const ())),
                ("initWithDictionary:", i(dict_init_dict as *const ())),
                ("initWithContentsOfFile:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:error:", i(dict_init_with_url_err as *const ())),
                ("writeToFile:atomically:", i(dict_write_to_file as *const ())),
                ("writeToURL:error:", i(dict_write_to_url_err as *const ())),
                ("countByEnumeratingWithState:objects:count:", i(fast_enum_dict as *const ())),
                ("keyEnumerator", i(dict_key_enumerator as *const ())),
                ("objectEnumerator", i(dict_object_enumerator as *const ())),
                ("copy", i(dict_copy as *const ())),
                ("mutableCopy", i(dict_mcopy as *const ())),
                ("copyWithZone:", i(dict_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("dictionary", i(dict_new as *const ())),
                ("dictionaryWithObject:forKey:", i(dict_with_obj_key as *const ())),
                ("dictionaryWithObjectsAndKeys:", i(darwin_dict_with_objs_keys as *const ())),
                ("dictionaryWithObjects:forKeys:count:", i(dict_with_objs_keys as *const ())),
                ("dictionaryWithDictionary:", i(dict_with_dict as *const ())),
                ("dictionaryWithContentsOfFile:", i(dict_with_file as *const ())),
                ("dictionaryWithContentsOfURL:", i(dict_with_file as *const ())),
                ("dictionaryWithContentsOfURL:error:", i(dict_with_url_err as *const ())),
            ],
        );
        runtime::register_builtin(
            NSMUTABLEDICTIONARY_CLASS.ptr(),
            NSMUTABLEDICTIONARY_META.ptr(),
            "NSMutableDictionary",
            16,
            &[
                ("count", i(dict_count as *const ())),
                ("objectForKey:", i(dict_get as *const ())),
                ("valueForKey:", i(dict_get as *const ())),
                ("objectForKeyedSubscript:", i(dict_get as *const ())),
                ("allKeys", i(dict_all_keys as *const ())),
                ("allValues", i(dict_all_values as *const ())),
                ("initWithObjectsAndKeys:", i(darwin_dict_init_objs_keys as *const ())),
                ("initWithDictionary:", i(dict_init_dict as *const ())),
                ("initWithContentsOfFile:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:", i(dict_init_with_file as *const ())),
                ("initWithContentsOfURL:error:", i(dict_init_with_url_err as *const ())),
                ("writeToFile:atomically:", i(dict_write_to_file as *const ())),
                ("writeToURL:error:", i(dict_write_to_url_err as *const ())),
                ("initWithCapacity:", i(dict_init_cap as *const ())),
                ("countByEnumeratingWithState:objects:count:", i(fast_enum_dict as *const ())),
                ("keyEnumerator", i(dict_key_enumerator as *const ())),
                ("objectEnumerator", i(dict_object_enumerator as *const ())),
                ("setObject:forKey:", i(dict_set as *const ())),
                ("setValue:forKey:", i(dict_set as *const ())),
                ("setObject:forKeyedSubscript:", i(dict_set as *const ())),
                ("addEntriesFromDictionary:", i(dict_add_entries as *const ())),
                ("removeObjectForKey:", i(dict_remove_key as *const ())),
                ("removeAllObjects", i(dict_remove_all as *const ())),
                ("copy", i(dict_copy as *const ())),
                ("mutableCopy", i(dict_mcopy as *const ())),
                ("copyWithZone:", i(dict_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("dictionary", i(dict_new as *const ())),
                ("dictionaryWithObject:forKey:", i(dict_with_obj_key as *const ())),
                ("dictionaryWithObjectsAndKeys:", i(darwin_dict_with_objs_keys as *const ())),
                ("dictionaryWithObjects:forKeys:count:", i(dict_with_objs_keys as *const ())),
                ("dictionaryWithDictionary:", i(dict_with_dict as *const ())),
                ("dictionaryWithCapacity:", i(dict_with_cap as *const ())),
            ],
        );
        runtime::register_builtin(
            NSDATA_CLASS.ptr(),
            NSDATA_META.ptr(),
            "NSData",
            16,
            &[
                ("length", i(data_length as *const ())),
                ("bytes", i(data_bytes as *const ())),
                ("initWithContentsOfFile:", i(data_init_with_file as *const ())),
                ("initWithContentsOfURL:", i(data_init_with_file as *const ())),
                ("writeToFile:atomically:", i(data_write_file as *const ())),
                ("copy", i(copy_retain as *const ())),
                ("copyWithZone:", i(copy_zone_retain as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("data", i(data_new as *const ())),
                ("dataWithBytes:length:", i(data_with_bytes as *const ())),
                ("dataWithContentsOfFile:", i(data_with_file as *const ())),
                ("dataWithContentsOfURL:", i(data_with_file as *const ())),
                ("dataWithContentsOfFile:options:error:", i(data_with_file_opts_err as *const ())),
                ("dataWithContentsOfURL:options:error:", i(data_with_file_opts_err as *const ())),
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
                ("writeToFile:atomically:", i(data_write_file as *const ())),
                ("copy", i(copy_retain as *const ())),
                ("copyWithZone:", i(copy_zone_retain as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("data", i(data_new as *const ())),
                ("dataWithBytes:length:", i(data_with_bytes as *const ())),
                ("dataWithContentsOfFile:", i(data_with_file as *const ())),
                ("dataWithContentsOfURL:", i(data_with_file as *const ())),
                ("dataWithContentsOfFile:options:error:", i(data_with_file_opts_err as *const ())),
                ("dataWithContentsOfURL:options:error:", i(data_with_file_opts_err as *const ())),
            ],
        );
        runtime::register_builtin(
            NSNUMBER_CLASS.ptr(),
            NSNUMBER_META.ptr(),
            "NSNumber",
            16,
            &[
                ("intValue", i(num_as_i32 as *const ())),
                ("unsignedIntValue", i(num_as_u32 as *const ())),
                ("integerValue", i(num_as_i64 as *const ())),
                ("unsignedIntegerValue", i(num_as_i64 as *const ())),
                ("longValue", i(num_as_i64 as *const ())),
                ("unsignedLongValue", i(num_as_i64 as *const ())),
                ("longLongValue", i(num_as_i64 as *const ())),
                ("unsignedLongLongValue", i(num_as_i64 as *const ())),
                ("doubleValue", i(num_as_f64 as *const ())),
                ("floatValue", i(num_as_f32 as *const ())),
                ("boolValue", i(num_as_bool as *const ())),
                ("stringValue", i(num_string_value as *const ())),
                ("description", i(num_string_value as *const ())),
                ("isEqualToNumber:", i(num_is_equal as *const ())),
                ("isEqual:", i(num_is_equal as *const ())),
                ("compare:", i(num_compare as *const ())),
                ("copy", i(copy_retain as *const ())),
                ("copyWithZone:", i(copy_zone_retain as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("numberWithInt:", i(num_i32 as *const ())),
                ("numberWithUnsignedInt:", i(num_u32 as *const ())),
                ("numberWithInteger:", i(num_i64 as *const ())),
                ("numberWithUnsignedInteger:", i(num_i64 as *const ())),
                ("numberWithLong:", i(num_i64 as *const ())),
                ("numberWithUnsignedLong:", i(num_i64 as *const ())),
                ("numberWithLongLong:", i(num_i64 as *const ())),
                ("numberWithUnsignedLongLong:", i(num_i64 as *const ())),
                ("numberWithDouble:", i(num_f64 as *const ())),
                ("numberWithFloat:", i(num_f32 as *const ())),
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
                ("executablePath", i(bundle_exec_path as *const ())),
                ("bundleIdentifier", i(bundle_id as *const ())),
                ("bundleURL", i(bundle_url as *const ())),
                ("resourceURL", i(bundle_url as *const ())),
                ("appStoreReceiptURL", i(bundle_receipt_url as *const ())),
                ("infoDictionary", i(bundle_info as *const ())),
                ("objectForInfoDictionaryKey:", i(bundle_obj_for_info as *const ())),
                ("localizedStringForKey:value:table:", i(bundle_localized_string as *const ())),
                ("preferredLocalizations", i(bundle_localizations as *const ())),
                ("localizations", i(bundle_localizations as *const ())),
                ("pathForResource:ofType:", i(bundle_path_for_res as *const ())),
                ("pathForResource:ofType:inDirectory:", i(bundle_path_for_res_in_dir as *const ())),
                ("URLForResource:withExtension:", i(bundle_url_for_res as *const ())),
                ("URLForResource:withExtension:subdirectory:", i(bundle_url_for_res_in_dir as *const ())),
                ("pathsForResourcesOfType:inDirectory:", i(bundle_paths_for_res as *const ())),
            ],
            &[
                ("mainBundle", i(bundle_main as *const ())),
                ("bundleForClass:", i(bundle_for_arg as *const ())),
                ("bundleWithPath:", i(bundle_for_arg as *const ())),
                ("bundleWithIdentifier:", i(bundle_for_arg as *const ())),
            ],
        );
        runtime::register_builtin(
            NSNOTIFICATIONCENTER_CLASS.ptr(),
            NSNOTIFICATIONCENTER_META.ptr(),
            "NSNotificationCenter",
            16,
            &[
                ("addObserver:selector:name:object:", i(nc_add_obs as *const ())),
                ("addObserverForName:object:queue:usingBlock:", i(nc_add_obs_block as *const ())),
                ("postNotificationName:object:", i(nc_post_no_info as *const ())),
                ("postNotificationName:object:userInfo:", i(nc_post as *const ())),
                ("removeObserver:", i(nc_remove_obs as *const ())),
                ("removeObserver:name:object:", i(nc_remove_obs_name_obj as *const ())),
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
            &[
                ("timeIntervalSince1970", i(date_since_1970 as *const ())),
                ("timeIntervalSinceReferenceDate", i(date_since_ref as *const ())),
                ("timeIntervalSinceNow", i(date_since_now as *const ())),
                ("timeIntervalSinceDate:", i(date_since_date as *const ())),
                ("dateByAddingTimeInterval:", i(date_adding as *const ())),
                ("isEqualToDate:", i(date_is_equal as *const ())),
                ("compare:", i(date_compare as *const ())),
                ("copyWithZone:", i(date_copy_zone as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("date", i(date_now as *const ())),
                ("dateWithTimeIntervalSince1970:", i(date_with_1970 as *const ())),
                ("dateWithTimeIntervalSinceNow:", i(date_with_now as *const ())),
                ("dateWithTimeIntervalSinceReferenceDate:", i(date_with_ref as *const ())),
                ("distantPast", i(date_distant_past as *const ())),
                ("distantFuture", i(date_distant_future as *const ())),
                ("timeIntervalSinceReferenceDate", i(date_cls_since_ref as *const ())),
            ],
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
            &[
                ("fileExistsAtPath:", i(fm_file_exists as *const ())),
                ("fileExistsAtPath:isDirectory:", i(fm_file_exists_is_dir as *const ())),
                ("createDirectoryAtPath:withIntermediateDirectories:attributes:error:", i(fm_create_dir as *const ())),
                ("changeCurrentDirectoryPath:", i(fm_change_dir as *const ())),
                ("currentDirectoryPath", i(fm_current_dir as *const ())),
                ("removeItemAtPath:error:", i(fm_remove_item as *const ())),
                ("copyItemAtPath:toPath:error:", i(fm_copy_item as *const ())),
                ("moveItemAtPath:toPath:error:", i(fm_move_item as *const ())),
                ("contentsOfDirectoryAtPath:error:", i(fm_contents_of_dir as *const ())),
                ("attributesOfItemAtPath:error:", i(fm_attributes_of_item as *const ())),
                ("URLsForDirectory:inDomains:", i(fm_urls_for_dir as *const ())),
            ],
            &[("defaultManager", i(fm_default as *const ()))],
        );
        runtime::register_builtin(
            NSURL_CLASS.ptr(),
            NSURL_META.ptr(),
            "NSURL",
            16,
            &[
                ("path", i(url_path as *const ())),
                ("absoluteString", i(url_path as *const ())),
                ("description", i(url_path as *const ())),
                ("absoluteURL", i(copy_retain as *const ())),
                ("standardizedURL", i(copy_retain as *const ())),
                ("lastPathComponent", i(str_last_path_comp as *const ())),
                ("pathExtension", i(str_path_ext as *const ())),
                ("pathComponents", i(str_path_components as *const ())),
                ("URLByAppendingPathComponent:", i(url_appending_path_component as *const ())),
                ("URLByAppendingPathComponent:isDirectory:", i(url_appending_path_component_dir as *const ())),
                ("URLByDeletingLastPathComponent", i(url_del_last_comp as *const ())),
                ("URLByAppendingPathExtension:", i(url_append_ext as *const ())),
                ("URLByDeletingPathExtension", i(url_del_ext as *const ())),
                ("isFileURL", i(url_is_file as *const ())),
                ("checkResourceIsReachableAndReturnError:", i(url_check_reachable as *const ())),
                ("scheme", i(url_scheme as *const ())),
                ("host", i(url_host as *const ())),
                ("query", i(url_query as *const ())),
                ("isEqual:", i(str_is_equal as *const ())),
                ("hash", i(str_hash as *const ())),
                ("copy", i(copy_retain as *const ())),
                ("copyWithZone:", i(copy_zone_retain as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("fileURLWithPath:", i(url_file_with_path as *const ())),
                ("fileURLWithPath:isDirectory:", i(url_file_with_path_dir as *const ())),
                ("URLWithString:", i(url_file_with_path as *const ())),
                ("URLWithString:relativeToURL:", i(url_with_str_rel as *const ())),
            ],
        );
        runtime::register_builtin(
            NSSET_CLASS.ptr(),
            NSSET_META.ptr(),
            "NSSet",
            16,
            &[
                ("count", i(arr_count as *const ())),
                ("countByEnumeratingWithState:objects:count:", i(fast_enum_arr as *const ())),
                ("objectEnumerator", i(col_object_enumerator as *const ())),
                ("reverseObjectEnumerator", i(arr_reverse_object_enumerator as *const ())),
                ("containsObject:", i(arr_contains as *const ())),
                ("anyObject", i(set_any_object as *const ())),
                ("allObjects", i(set_all_objects as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("set", i(set_empty as *const ())),
                ("setWithObject:", i(set_with_object as *const ())),
            ],
        );
        runtime::register_builtin(
            NSMUTABLESET_CLASS.ptr(),
            NSMUTABLESET_META.ptr(),
            "NSMutableSet",
            16,
            &[
                ("count", i(arr_count as *const ())),
                ("countByEnumeratingWithState:objects:count:", i(fast_enum_arr as *const ())),
                ("objectEnumerator", i(col_object_enumerator as *const ())),
                ("reverseObjectEnumerator", i(arr_reverse_object_enumerator as *const ())),
                ("containsObject:", i(arr_contains as *const ())),
                ("anyObject", i(set_any_object as *const ())),
                ("allObjects", i(set_all_objects as *const ())),
                ("addObject:", i(arr_add as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("set", i(mset_empty as *const ())),
                ("setWithObject:", i(set_with_object as *const ())),
            ],
        );
        runtime::register_builtin(
            NSENUMERATOR_CLASS.ptr(),
            NSENUMERATOR_META.ptr(),
            "NSEnumerator",
            16,
            &[
                ("nextObject", i(enum_next_object as *const ())),
                ("allObjects", i(enum_all_objects as *const ())),
                ("countByEnumeratingWithState:objects:count:", i(fast_enum_arr as *const ())),
                ("dealloc", i(enum_dealloc as *const ())),
            ],
            &[],
        );
        runtime::register_builtin(
            NSUSERDEFAULTS_CLASS.ptr(),
            NSUSERDEFAULTS_META.ptr(),
            "NSUserDefaults",
            16,
            &[
                ("objectForKey:", i(ud_obj_for_key as *const ())),
                ("stringForKey:", i(ud_string_for_key as *const ())),
                ("boolForKey:", i(ud_bool_for_key as *const ())),
                ("integerForKey:", i(ud_int_for_key as *const ())),
                ("setObject:forKey:", i(ud_set_obj_for_key as *const ())),
                ("synchronize", i(ud_sync as *const ())),
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
            &[
                ("characterIsMember:", i(cs_char_is_member as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
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
            &[
                ("localeIdentifier", i(locale_id as *const ())),
                ("objectForKey:", i(locale_obj_for_key as *const ())),
                ("countryCode", i(locale_country_code as *const ())),
                ("languageCode", i(locale_language_code as *const ())),
                ("currencyCode", i(locale_currency_code as *const ())),
                ("currencySymbol", i(locale_currency_symbol as *const ())),
                ("decimalSeparator", i(locale_decimal_sep as *const ())),
                ("groupingSeparator", i(locale_grouping_sep as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("currentLocale", i(locale_current as *const ())),
                ("autoupdatingCurrentLocale", i(locale_current as *const ())),
                ("systemLocale", i(locale_current as *const ())),
                ("localeWithLocaleIdentifier:", i(locale_with_id as *const ())),
                ("preferredLanguages", i(locale_langs as *const ())),
                ("ISOCountryCodes", i(locale_country_codes as *const ())),
                ("ISOLanguageCodes", i(locale_langs as *const ())),
                ("availableLocaleIdentifiers", i(locale_langs as *const ())),
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
                ("environment", i(pi_env as *const ())),
                ("arguments", i(pi_args as *const ())),
                ("processName", i(pi_name as *const ())),
                ("operatingSystemVersion", i(pi_os_version_struct as *const ())),
                ("operatingSystemVersionString", i(pi_os_ver as *const ())),
                ("isOperatingSystemAtLeastVersion:", i(pi_is_os_at_least as *const ())),
                ("processorCount", i(pi_cpus as *const ())),
                ("activeProcessorCount", i(pi_cpus as *const ())),
                ("physicalMemory", i(pi_mem as *const ())),
                ("systemUptime", i(pi_uptime as *const ())),
                ("globallyUniqueString", i(pi_unique_str as *const ())),
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
        runtime::register_builtin(
            NSAUTORELEASEPOOL_CLASS.ptr(),
            NSAUTORELEASEPOOL_META.ptr(),
            "NSAutoreleasePool",
            16,
            &[("drain", i(timer_invalidate as *const ())), ("dealloc", i(f_dealloc as *const ()))],
            &[],
        );
        runtime::register_builtin(
            NSTHREAD_CLASS.ptr(),
            NSTHREAD_META.ptr(),
            "NSThread",
            16,
            &[
                ("start", i(timer_invalidate as *const ())),
                ("setName:", i(noop_arg1 as *const ())),
                ("name", i(thread_name as *const ())),
                ("isMainThread", i(thread_is_main as *const ())),
                ("isMultiThreaded", i(thread_is_main as *const ())),
                ("setThreadPriority:", i(noop_f64 as *const ())),
                ("threadPriority", i(ret_one_f64 as *const ())),
                ("threadDictionary", i(empty_dict_obj as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("isMainThread", i(thread_is_main as *const ())),
                ("isMultiThreaded", i(thread_is_main as *const ())),
                ("setThreadPriority:", i(noop_f64 as *const ())),
                ("currentThread", i(thread_current as *const ())),
                ("mainThread", i(thread_current as *const ())),
                ("sleepForTimeInterval:", i(thread_sleep as *const ())),
                ("detachNewThreadSelector:toTarget:withObject:", i(thread_detach as *const ())),
            ],
        );
        runtime::register_builtin(
            NSOPERATIONQUEUE_CLASS.ptr(),
            NSOPERATIONQUEUE_META.ptr(),
            "NSOperationQueue",
            16,
            &[
                ("setName:", i(opqueue_set_name as *const ())),
                ("name", i(opqueue_name as *const ())),
                ("setMaxConcurrentOperationCount:", i(noop_arg1 as *const ())),
                ("maxConcurrentOperationCount", i(ret_one_isize as *const ())),
                ("setQualityOfService:", i(noop_arg1 as *const ())),
                ("addOperationWithBlock:", i(noop_arg1 as *const ())),
                ("addOperation:", i(noop_arg1 as *const ())),
                ("cancelAllOperations", i(timer_invalidate as *const ())),
                ("waitUntilAllOperationsAreFinished", i(timer_invalidate as *const ())),
                ("setSuspended:", i(noop_arg1 as *const ())),
                ("isSuspended", i(ret_false as *const ())),
                ("operations", i(empty_arr_obj as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("mainQueue", i(opqueue_main as *const ())),
                ("currentQueue", i(opqueue_main as *const ())),
            ],
        );
        runtime::register_builtin(
            NSURLSESSIONCONFIGURATION_CLASS.ptr(),
            NSURLSESSIONCONFIGURATION_META.ptr(),
            "NSURLSessionConfiguration",
            16,
            &[
                ("setTimeoutIntervalForRequest:", i(noop_f64 as *const ())),
                ("setTimeoutIntervalForResource:", i(noop_f64 as *const ())),
                ("setHTTPAdditionalHeaders:", i(noop_arg1 as *const ())),
                ("setRequestCachePolicy:", i(noop_arg1 as *const ())),
                ("setURLCache:", i(noop_arg1 as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("defaultSessionConfiguration", i(sess_cfg_default as *const ())),
                ("ephemeralSessionConfiguration", i(sess_cfg_default as *const ())),
                ("backgroundSessionConfigurationWithIdentifier:", i(sess_cfg_bg as *const ())),
            ],
        );
        runtime::register_builtin(
            NSURLSESSION_CLASS.ptr(),
            NSURLSESSION_META.ptr(),
            "NSURLSession",
            16,
            &[
                ("dataTaskWithRequest:", i(sess_task as *const ())),
                ("dataTaskWithRequest:completionHandler:", i(sess_task2 as *const ())),
                ("dataTaskWithURL:", i(sess_task as *const ())),
                ("dataTaskWithURL:completionHandler:", i(sess_task2 as *const ())),
                ("invalidateAndCancel", i(timer_invalidate as *const ())),
                ("finishTasksAndInvalidate", i(timer_invalidate as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("sharedSession", i(sess_shared as *const ())),
                ("sessionWithConfiguration:", i(sess_with_cfg as *const ())),
                ("sessionWithConfiguration:delegate:delegateQueue:", i(sess_with_cfg3 as *const ())),
            ],
        );
        for (cls, meta, name) in [
            (&NSFILECOORDINATOR_CLASS, &NSFILECOORDINATOR_META, "NSFileCoordinator"),
            (&NSFILEVERSION_CLASS, &NSFILEVERSION_META, "NSFileVersion"),
            (&NSMETADATAQUERY_CLASS, &NSMETADATAQUERY_META, "NSMetadataQuery"),
            (&NSNUMBERFORMATTER_CLASS, &NSNUMBERFORMATTER_META, "NSNumberFormatter"),
            (&NSPREDICATE_CLASS, &NSPREDICATE_META, "NSPredicate"),
            (&NSCALENDAR_CLASS, &NSCALENDAR_META, "NSCalendar"),
            (&NSDATECOMPONENTS_CLASS, &NSDATECOMPONENTS_META, "NSDateComponents"),
            (&NSKEYEDUNARCHIVER_CLASS, &NSKEYEDUNARCHIVER_META, "NSKeyedUnarchiver"),
        ] {
            runtime::register_builtin(cls.ptr(), meta.ptr(), name, 16, &[("dealloc", i(f_dealloc as *const ()))], &[]);
        }
        runtime::register_builtin(
            NSEXCEPTION_CLASS.ptr(),
            NSEXCEPTION_META.ptr(),
            "NSException",
            16,
            &[
                ("initWithName:reason:userInfo:", i(exc_init as *const ())),
                ("name", i(exc_name as *const ())),
                ("reason", i(exc_reason as *const ())),
                ("userInfo", i(exc_userinfo as *const ())),
                ("description", i(exc_reason as *const ())),
                ("callStackSymbols", i(empty_arr_obj as *const ())),
                ("raise", i(exc_raise_self as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[
                ("exceptionWithName:reason:userInfo:", i(exc_with as *const ())),
                ("raise:format:", i(exc_raise_fmt as *const ())),
                ("raise:format:arguments:", i(exc_raise_fmt as *const ())),
            ],
        );
        runtime::register_builtin(
            NSASSERTIONHANDLER_CLASS.ptr(),
            NSASSERTIONHANDLER_META.ptr(),
            "NSAssertionHandler",
            16,
            &[
                ("handleFailureInMethod:object:file:lineNumber:description:", i(assert_fail as *const ())),
                ("handleFailureInFunction:file:lineNumber:description:", i(assert_fail as *const ())),
                ("dealloc", i(f_dealloc as *const ())),
            ],
            &[("currentHandler", i(assert_current as *const ()))],
        );
        runtime::register_builtin(
            NSNULL_CLASS.ptr(),
            NSNULL_META.ptr(),
            "NSNull",
            16,
            &[
                ("copy", i(copy_retain as *const ())),
                ("copyWithZone:", i(copy_zone_retain as *const ())),
            ],
            &[("null", i(ns_null_singleton as *const ()))],
        );
    });
}

extern "C" fn exc_init(this: Id, _cmd: Sel, name: Id, reason: Id, info: Id) -> Id {
    let mut map = HashMap::new();
    if !name.is_null() { map.insert("name".into(), retain(name) as usize); }
    if !reason.is_null() { map.insert("reason".into(), retain(reason) as usize); }
    if !info.is_null() { map.insert("userInfo".into(), retain(info) as usize); }
    store().lock().unwrap().insert(this as usize, Backing::Dict(map));
    this
}
extern "C" fn exc_with(cls: Id, sel_id: Sel, name: Id, reason: Id, info: Id) -> Id {
    let obj = unsafe { alloc_with_backing(cls as *const ClassT, Backing::Dict(HashMap::new())) };
    exc_init(obj, sel_id, name, reason, info)
}
fn exc_field(this: Id, k: &str) -> Id {
    store().lock().unwrap().get(&(this as usize)).and_then(|b| match b {
        Backing::Dict(m) => m.get(k).copied(),
        _ => None,
    }).unwrap_or(0) as Id
}
extern "C" fn exc_name(this: Id, _cmd: Sel) -> Id { exc_field(this, "name") }
extern "C" fn exc_reason(this: Id, _cmd: Sel) -> Id { exc_field(this, "reason") }
extern "C" fn exc_userinfo(this: Id, _cmd: Sel) -> Id { exc_field(this, "userInfo") }
extern "C-unwind" fn exc_raise_self(this: Id, _cmd: Sel) -> ! {
    let n = rust_str_from_nsstring(exc_name(this, sel("name"))).unwrap_or_default();
    let r = rust_str_from_nsstring(exc_reason(this, sel("reason"))).unwrap_or_default();
    eprintln!("[NSException] raise name={n:?} reason={r:?}");
    unsafe { pear_libobjc::exceptions::objc_exception_throw(this) }
}
extern "C-unwind" fn exc_raise_fmt(cls: Id, _cmd: Sel, name: Id, fmt: Id) -> ! {
    let n = rust_str_from_nsstring(name).unwrap_or_default();
    let r = rust_str_from_nsstring(fmt).unwrap_or_default();
    eprintln!("[NSException] raise:format: name={n:?} reason={r:?}");
    let exc = exc_with(cls, sel("exceptionWithName:reason:userInfo:"), name, fmt, std::ptr::null_mut());
    unsafe { pear_libobjc::exceptions::objc_exception_throw(exc) }
}
extern "C" fn assert_current(_cls: Id, _cmd: Sel) -> Id {
    static A: OnceLock<usize> = OnceLock::new();
    *A.get_or_init(|| unsafe { alloc_with_backing(NSASSERTIONHANDLER_CLASS.ptr(), Backing::Arr(Vec::new())) } as usize) as Id
}
extern "C" fn assert_fail(_self: Id, _cmd: Sel, _a: Id, _b: Id, _c: Id, _d: isize, desc: Id) {
    let msg = rust_str_from_nsstring(desc).unwrap_or_default();
    eprintln!("[NSAssertionHandler] failure: {msg}");
}

extern "C" fn ns_null_singleton(_self: Id, _cmd: Sel) -> Id {
    static N: OnceLock<usize> = OnceLock::new();
    *N.get_or_init(|| unsafe { alloc_with_backing(NSNULL_CLASS.ptr(), Backing::Arr(Vec::new())) } as usize) as Id
}

extern "C" fn thread_is_main(_self: Id, _cmd: Sel) -> bool { true }
extern "C" fn thread_current(_self: Id, _cmd: Sel) -> Id {
    static T: OnceLock<usize> = OnceLock::new();
    *T.get_or_init(|| unsafe { alloc_with_backing(NSTHREAD_CLASS.ptr(), Backing::Str("main".into())) } as usize) as Id
}
extern "C" fn thread_name(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_else(|| "main".to_string());
    nsstring_from_rust(&s)
}
extern "C" fn thread_sleep(_self: Id, _cmd: Sel, secs: f64) {
    if secs > 0.0 && secs < 60.0 {
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
    }
}
extern "C" fn thread_detach(_self: Id, _cmd: Sel, _sel: Sel, _target: Id, _arg: Id) {}
extern "C" fn noop_arg1(_self: Id, _cmd: Sel, _arg: usize) {}
extern "C" fn noop_f64(_self: Id, _cmd: Sel, _arg: f64) {}
extern "C" fn ret_one_isize(_self: Id, _cmd: Sel) -> isize { 1 }
extern "C" fn ret_one_f64(_self: Id, _cmd: Sel) -> f64 { 0.5 }
extern "C" fn ret_false(_self: Id, _cmd: Sel) -> bool { false }
extern "C" fn empty_arr_obj(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(Vec::new())) }
}
extern "C" fn empty_dict_obj(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSMUTABLEDICTIONARY_CLASS.ptr(), Backing::Dict(HashMap::new())) }
}
extern "C" fn opqueue_main(_self: Id, _cmd: Sel) -> Id {
    static Q: OnceLock<usize> = OnceLock::new();
    *Q.get_or_init(|| unsafe { alloc_with_backing(NSOPERATIONQUEUE_CLASS.ptr(), Backing::Str("main".into())) } as usize) as Id
}
extern "C" fn opqueue_set_name(self_: Id, _cmd: Sel, name: Id) {
    let s = rust_str_from_nsstring(name).unwrap_or_default();
    store().lock().unwrap().insert(self_ as usize, Backing::Str(s));
}
extern "C" fn opqueue_name(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    nsstring_from_rust(&s)
}
extern "C" fn sess_cfg_default(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSURLSESSIONCONFIGURATION_CLASS.ptr(), Backing::Dict(HashMap::new())) }
}
extern "C" fn sess_cfg_bg(_self: Id, _cmd: Sel, _ident: Id) -> Id {
    sess_cfg_default(_self, _cmd)
}
extern "C" fn sess_shared(_self: Id, _cmd: Sel) -> Id {
    static S: OnceLock<usize> = OnceLock::new();
    *S.get_or_init(|| unsafe { alloc_with_backing(NSURLSESSION_CLASS.ptr(), Backing::Dict(HashMap::new())) } as usize) as Id
}
extern "C" fn sess_with_cfg(_self: Id, _cmd: Sel, _cfg: Id) -> Id {
    unsafe { alloc_with_backing(NSURLSESSION_CLASS.ptr(), Backing::Dict(HashMap::new())) }
}
extern "C" fn sess_with_cfg3(_self: Id, _cmd: Sel, _cfg: Id, _del: Id, _q: Id) -> Id {
    sess_with_cfg(_self, _cmd, _cfg)
}
extern "C" fn sess_task(_self: Id, _cmd: Sel, _arg: Id) -> Id { std::ptr::null_mut() }
extern "C" fn sess_task2(_self: Id, _cmd: Sel, _arg: Id, _cb: Id) -> Id { std::ptr::null_mut() }

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

extern "C" fn url_file_with_path_dir(cls: Id, sel_id: Sel, path: Id, _is_dir: bool) -> Id {
    url_file_with_path(cls, sel_id, path)
}

extern "C" fn url_with_str_rel(cls: Id, _cmd: Sel, s: Id, rel: Id) -> Id {
    let part = rust_str_from_nsstring(s).unwrap_or_default();
    let base = if rel.is_null() { None } else { rust_str_from_nsstring(rel) };
    let full = match base {
        Some(b) if !b.is_empty() && !part.contains("://") && !part.starts_with('/') => {
            format!("{}/{}", b.trim_end_matches('/'), part)
        }
        _ => part,
    };
    let _ = cls;
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(full)) }
}

extern "C" fn url_path(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    nsstring_from_rust(&s)
}

extern "C" fn url_appending_path_component(self_: Id, _cmd: Sel, comp: Id) -> Id {
    let base = rust_str_from_nsstring(self_).unwrap_or_default();
    let tail = rust_str_from_nsstring(comp).unwrap_or_default();
    let joined = std::path::Path::new(&base).join(tail).to_string_lossy().into_owned();
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(joined)) }
}

extern "C" fn url_appending_path_component_dir(self_: Id, sel_id: Sel, comp: Id, _is_dir: bool) -> Id {
    url_appending_path_component(self_, sel_id, comp)
}

extern "C" fn url_del_last_comp(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    let p = std::path::Path::new(&s)
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(p)) }
}

extern "C" fn url_append_ext(self_: Id, _cmd: Sel, ext: Id) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    let e = rust_str_from_nsstring(ext).unwrap_or_default();
    let out = if e.is_empty() { s } else { format!("{s}.{e}") };
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(out)) }
}

extern "C" fn url_del_ext(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    let p = std::path::Path::new(&s);
    let stem = match (p.parent(), p.file_stem()) {
        (Some(d), Some(st)) if !d.as_os_str().is_empty() => d.join(st).to_string_lossy().into_owned(),
        (_, Some(st)) => st.to_string_lossy().into_owned(),
        _ => s,
    };
    unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(stem)) }
}

extern "C" fn url_is_file(self_: Id, _cmd: Sel) -> bool {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    s.starts_with('/') || s.starts_with("file://")
}

extern "C" fn url_check_reachable(self_: Id, _cmd: Sel, _err: *mut Id) -> bool {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    let path = s.strip_prefix("file://").unwrap_or(&s);
    std::path::Path::new(path).exists()
}

extern "C" fn url_scheme(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    if let Some((sch, _)) = s.split_once("://") {
        nsstring_from_rust(sch)
    } else if s.starts_with('/') {
        nsstring_from_rust("file")
    } else {
        std::ptr::null_mut()
    }
}

extern "C" fn url_host(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    if let Some((_, rest)) = s.split_once("://") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        nsstring_from_rust(host)
    } else {
        std::ptr::null_mut()
    }
}

extern "C" fn url_query(self_: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(self_).unwrap_or_default();
    s.split_once('?')
        .map(|(_, q)| nsstring_from_rust(q.split('#').next().unwrap_or("")))
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn str_can_be_converted(this: Id, _cmd: Sel, _enc: usize) -> bool {
    rust_str_from_nsstring(this).is_some()
}

extern "C" fn str_get_cstring(this: Id, _cmd: Sel, buf: *mut c_char, max_len: usize, _enc: usize) -> bool {
    if buf.is_null() || max_len == 0 {
        return false;
    }
    let Some(s) = rust_str_from_nsstring(this) else { return false; };
    let bytes = s.as_bytes();
    if bytes.len() + 1 > max_len {
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
        *buf.add(bytes.len()) = 0;
    }
    true
}

extern "C" fn str_identity_path(this: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    nsstring_from_rust(&s)
}

extern "C" fn str_expand_tilde(this: Id, _cmd: Sel) -> Id {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    if let Some(rest) = s.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        nsstring_from_rust(&format!("{home}/{rest}"))
    } else if s == "~" {
        ns_home_directory()
    } else {
        nsstring_from_rust(&s)
    }
}

extern "C" fn str_init_cstring_enc(this: Id, sel_id: Sel, cstr: *const c_char, _enc: usize) -> Id {
    str_init_utf8(this, sel_id, cstr)
}

extern "C" fn str_init_data_enc(this: Id, _sel: Sel, data: Id, _enc: usize) -> Id {
    let bytes = if data.is_null() {
        Vec::new()
    } else {
        match store().lock().unwrap().get(&(data as usize)) {
            Some(Backing::Bytes(b)) => b.clone(),
            Some(Backing::Str(s)) => s.as_bytes().to_vec(),
            _ => Vec::new(),
        }
    };
    let s = String::from_utf8_lossy(&bytes).into_owned();
    let cls = unsafe { if this.is_null() { NSSTRING_CLASS.ptr() } else { *(this as *const *const ClassT) } };
    unsafe { alloc_with_backing(cls, Backing::Str(s)) }
}

extern "C" fn set_empty(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSSET_CLASS.ptr(), Backing::Arr(Vec::new())) }
}

extern "C" fn mset_empty(_self: Id, _cmd: Sel) -> Id {
    unsafe { alloc_with_backing(NSMUTABLESET_CLASS.ptr(), Backing::Arr(Vec::new())) }
}

extern "C" fn set_with_object(cls: Id, _cmd: Sel, obj: Id) -> Id {
    let mut items = Vec::new();
    if !obj.is_null() {
        items.push(retain(obj) as usize);
    }
    let target = if cls.is_null() { NSSET_CLASS.ptr() } else { cls as *const ClassT };
    unsafe { alloc_with_backing(target, Backing::Arr(items)) }
}

extern "C" fn set_any_object(this: Id, _cmd: Sel) -> Id {
    if this.is_null() {
        return std::ptr::null_mut();
    }
    match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Arr(v)) => v.first().copied().unwrap_or(0) as Id,
        _ => std::ptr::null_mut(),
    }
}

extern "C" fn set_all_objects(this: Id, _cmd: Sel) -> Id {
    let items: Vec<usize> = if this.is_null() {
        Vec::new()
    } else {
        match store().lock().unwrap().get(&(this as usize)) {
            Some(Backing::Arr(v)) => v.iter().map(|&p| retain(p as Id) as usize).collect(),
            _ => Vec::new(),
        }
    };
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(items)) }
}

/// Allocates an `NSSet` containing retained pointers to `items`.
pub fn ns_set_with_objects(items: &[Id]) -> Id {
    let retained: Vec<usize> = items
        .iter()
        .copied()
        .filter(|p| !p.is_null())
        .map(|p| retain(p) as usize)
        .collect();
    unsafe { alloc_with_backing(NSSET_CLASS.ptr(), Backing::Arr(retained)) }
}

static ENUM_CURSORS: Mutex<Option<HashMap<usize, usize>>> = Mutex::new(None);

fn make_enumerator(mut items: Vec<usize>, reverse: bool) -> Id {
    if reverse {
        items.reverse();
    }
    let retained: Vec<usize> = items.into_iter().map(|p| retain(p as Id) as usize).collect();
    let en = unsafe { alloc_with_backing(NSENUMERATOR_CLASS.ptr(), Backing::Arr(retained)) };
    ENUM_CURSORS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(en as usize, 0);
    en
}

extern "C" fn col_object_enumerator(this: Id, _cmd: Sel) -> Id {
    let items = if this.is_null() {
        Vec::new()
    } else {
        match store().lock().unwrap().get(&(this as usize)) {
            Some(Backing::Arr(v)) => v.clone(),
            _ => Vec::new(),
        }
    };
    make_enumerator(items, false)
}

extern "C" fn arr_reverse_object_enumerator(this: Id, _cmd: Sel) -> Id {
    let items = if this.is_null() {
        Vec::new()
    } else {
        match store().lock().unwrap().get(&(this as usize)) {
            Some(Backing::Arr(v)) => v.clone(),
            _ => Vec::new(),
        }
    };
    make_enumerator(items, true)
}

extern "C" fn dict_key_enumerator(this: Id, _cmd: Sel) -> Id {
    let items = if this.is_null() {
        Vec::new()
    } else {
        match store().lock().unwrap().get(&(this as usize)) {
            Some(Backing::Dict(m)) => m.keys().map(|k| nsstring_from_rust(k) as usize).collect(),
            _ => Vec::new(),
        }
    };
    let en = unsafe { alloc_with_backing(NSENUMERATOR_CLASS.ptr(), Backing::Arr(items)) };
    ENUM_CURSORS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(en as usize, 0);
    en
}

extern "C" fn dict_object_enumerator(this: Id, _cmd: Sel) -> Id {
    let items = if this.is_null() {
        Vec::new()
    } else {
        match store().lock().unwrap().get(&(this as usize)) {
            Some(Backing::Dict(m)) => m.values().copied().collect(),
            _ => Vec::new(),
        }
    };
    make_enumerator(items, false)
}

extern "C" fn enum_next_object(this: Id, _cmd: Sel) -> Id {
    if this.is_null() {
        return std::ptr::null_mut();
    }
    let idx = {
        let mut g = ENUM_CURSORS.lock().unwrap();
        let entry = g.get_or_insert_with(HashMap::new).entry(this as usize).or_insert(0);
        let cur = *entry;
        *entry = cur.saturating_add(1);
        cur
    };
    match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Arr(v)) if idx < v.len() => v[idx] as Id,
        _ => std::ptr::null_mut(),
    }
}

extern "C" fn enum_all_objects(this: Id, _cmd: Sel) -> Id {
    if this.is_null() {
        return unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(Vec::new())) };
    }
    let idx = {
        let mut g = ENUM_CURSORS.lock().unwrap();
        let entry = g.get_or_insert_with(HashMap::new).entry(this as usize).or_insert(0);
        let cur = *entry;
        cur
    };
    let rest: Vec<usize> = match store().lock().unwrap().get(&(this as usize)) {
        Some(Backing::Arr(v)) if idx < v.len() => {
            let slice = v[idx..].to_vec();
            if let Some(m) = ENUM_CURSORS.lock().unwrap().as_mut() {
                m.insert(this as usize, v.len());
            }
            slice.into_iter().map(|p| retain(p as Id) as usize).collect()
        }
        _ => Vec::new(),
    };
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(rest)) }
}

extern "C" fn enum_dealloc(this: Id, _cmd: Sel) {
    if let Some(m) = ENUM_CURSORS.lock().unwrap().as_mut() {
        m.remove(&(this as usize));
    }
    f_dealloc(this, _cmd);
}

extern "C" fn ud_standard(_self: Id, _cmd: Sel) -> Id {
    static S: OnceLock<usize> = OnceLock::new();
    *S.get_or_init(|| unsafe { alloc_with_backing(NSUSERDEFAULTS_CLASS.ptr(), Backing::Dict(HashMap::new())) } as usize) as Id
}

extern "C" fn ud_string_for_key(self_: Id, _cmd: Sel, key: Id) -> Id {
    let o = ud_obj_for_key(self_, _cmd, key);
    if o.is_null() { std::ptr::null_mut() } else { o }
}

extern "C" fn ud_bool_for_key(self_: Id, _cmd: Sel, key: Id) -> bool {
    let o = ud_obj_for_key(self_, _cmd, key);
    if o.is_null() { return false; }
    num_as_bool(o, _cmd)
}

extern "C" fn ud_int_for_key(self_: Id, _cmd: Sel, key: Id) -> isize {
    let o = ud_obj_for_key(self_, _cmd, key);
    if o.is_null() { return 0; }
    num_as_i64(o, _cmd) as isize
}

extern "C" fn ud_sync(_self: Id, _cmd: Sel) -> bool {
    true
}

extern "C" fn fm_file_exists(_self: Id, _cmd: Sel, path: Id) -> bool {
    rust_str_from_nsstring(path).is_some_and(|p| std::path::Path::new(&p).exists())
}

extern "C" fn fm_file_exists_is_dir(_self: Id, _cmd: Sel, path: Id, is_dir: *mut u8) -> bool {
    let Some(p) = rust_str_from_nsstring(path) else {
        if !is_dir.is_null() { unsafe { *is_dir = 0; } }
        return false;
    };
    let path = std::path::Path::new(&p);
    let exists = path.exists();
    if !is_dir.is_null() {
        unsafe { *is_dir = u8::from(path.is_dir()); }
    }
    exists
}

extern "C" fn fm_change_dir(_self: Id, _cmd: Sel, path: Id) -> bool {
    rust_str_from_nsstring(path).is_some_and(|p| std::env::set_current_dir(&p).is_ok())
}

extern "C" fn fm_current_dir(_self: Id, _cmd: Sel) -> Id {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".to_string());
    nsstring_from_rust(&cwd)
}

extern "C" fn fm_remove_item(_self: Id, _cmd: Sel, path: Id, _err: *mut Id) -> bool {
    let Some(p) = rust_str_from_nsstring(path) else { return false; };
    let path = std::path::Path::new(&p);
    if path.is_dir() {
        std::fs::remove_dir_all(path).is_ok()
    } else {
        std::fs::remove_file(path).is_ok()
    }
}

extern "C" fn fm_copy_item(_self: Id, _cmd: Sel, src: Id, dst: Id, _err: *mut Id) -> bool {
    let (Some(s), Some(d)) = (rust_str_from_nsstring(src), rust_str_from_nsstring(dst)) else {
        return false;
    };
    std::fs::copy(&s, &d).is_ok()
}

extern "C" fn fm_move_item(_self: Id, _cmd: Sel, src: Id, dst: Id, _err: *mut Id) -> bool {
    let (Some(s), Some(d)) = (rust_str_from_nsstring(src), rust_str_from_nsstring(dst)) else {
        return false;
    };
    std::fs::rename(&s, &d).is_ok()
}

extern "C" fn fm_contents_of_dir(_self: Id, _cmd: Sel, path: Id, _err: *mut Id) -> Id {
    let Some(p) = rust_str_from_nsstring(path) else { return std::ptr::null_mut(); };
    let Ok(rd) = std::fs::read_dir(&p) else { return std::ptr::null_mut(); };
    let mut items = Vec::new();
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let ns = nsstring_from_rust(&name);
        items.push(retain(ns) as usize);
    }
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(items)) }
}

extern "C" fn fm_attributes_of_item(_self: Id, _cmd: Sel, path: Id, _err: *mut Id) -> Id {
    let Some(p) = rust_str_from_nsstring(path) else { return std::ptr::null_mut(); };
    let Ok(meta) = std::fs::metadata(&p) else { return std::ptr::null_mut(); };
    let mut map = HashMap::new();
    let num = unsafe {
        alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumI64(meta.len() as i64))
    };
    map.insert("NSFileSize".to_string(), retain(num) as usize);
    unsafe { alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map)) }
}

extern "C" fn fm_urls_for_dir(_self: Id, _cmd: Sel, dir: usize, _mask: usize) -> Id {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let sub = match dir {
        9 => "Documents",
        13 => "Library/Caches",
        5 => "Library",
        14 => "Library/Application Support",
        _ => "",
    };
    let full = if sub.is_empty() {
        home
    } else {
        let p = format!("{home}/{sub}");
        let _ = std::fs::create_dir_all(&p);
        p
    };
    let u = unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(full)) };
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec![retain(u) as usize])) }
}

extern "C" fn fm_create_dir(_self: Id, _cmd: Sel, path: Id, inter: bool, _attr: Id, _err: *mut Id) -> bool {
    let Some(p) = rust_str_from_nsstring(path) else { return false; };
    if inter {
        std::fs::create_dir_all(&p).is_ok()
    } else {
        std::fs::create_dir(&p).is_ok()
    }
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

fn boxed_objc_slot(obj: usize) -> usize {
    Box::into_raw(Box::new(obj)) as usize
}

fn ns_localized_description_key_ptr() -> *const () {
    static K: OnceLock<usize> = OnceLock::new();
    let p = *K.get_or_init(|| {
        init_foundation();
        boxed_objc_slot(nsstring_from_rust("NSLocalizedDescription") as usize)
    });
    p as *const ()
}

fn ns_array0_ptr() -> usize {
    static ARR0: OnceLock<usize> = OnceLock::new();
    *ARR0.get_or_init(|| {
        init_foundation();
        let obj = Box::leak(Box::new([NSARRAY_CLASS.ptr() as usize, 0usize]));
        let ptr = obj.as_mut_ptr() as usize;
        store().lock().unwrap().insert(ptr, Backing::Arr(Vec::new()));
        boxed_objc_slot(ptr)
    })
}

pub fn symbol(name: &str) -> Option<usize> {
    init_foundation();
    let f = |p: *const ()| Some(p as usize);
    match name {
        "___CFConstantStringClassReference" => Some(NSSTRING_CLASS.ptr() as usize),
        "___NSArray0__" => Some(ns_array0_ptr()),
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
        "_OBJC_CLASS_$_NSEnumerator" => Some(NSENUMERATOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSUserDefaults" => Some(NSUSERDEFAULTS_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSJSONSerialization" => Some(NSJSONSERIALIZATION_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSCharacterSet" => Some(NSCHARACTERSET_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSLocale" => Some(NSLOCALE_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSURLRequest" => Some(NSURLREQUEST_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMutableURLRequest" => Some(NSMUTABLEURLREQUEST_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSUUID" => Some(NSUUID_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSProcessInfo" => Some(NSPROCESSINFO_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSTimer" => Some(NSTIMER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSAutoreleasePool" => Some(NSAUTORELEASEPOOL_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSFileCoordinator" => Some(NSFILECOORDINATOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSFileVersion" => Some(NSFILEVERSION_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSMetadataQuery" => Some(NSMETADATAQUERY_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSNumberFormatter" => Some(NSNUMBERFORMATTER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSPredicate" => Some(NSPREDICATE_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSThread" => Some(NSTHREAD_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSCalendar" => Some(NSCALENDAR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSDateComponents" => Some(NSDATECOMPONENTS_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSException" => Some(NSEXCEPTION_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSConstantArray" => Some(NSARRAY_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSConstantDictionary" => Some(NSDICTIONARY_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSURLSession" => Some(NSURLSESSION_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSURLSessionConfiguration" => Some(NSURLSESSIONCONFIGURATION_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSOperationQueue" => Some(NSOPERATIONQUEUE_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSKeyedUnarchiver" => Some(NSKEYEDUNARCHIVER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSAssertionHandler" => Some(NSASSERTIONHANDLER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_NSNull" => Some(NSNULL_CLASS.ptr() as usize),
        "_OBJC_EHTYPE_$_NSException" => Some(&raw const OBJC_EHTYPE_NSEXCEPTION as usize),
        "_NSRunLoopCommonModes" => Some(const_nsstr("kCFRunLoopCommonModes") as usize),
        "_CFGetTypeID" => f(cf_get_type_id as *const ()),
        "_NSClassFromString" => f(ns_class_from_string as *const ()),
        "_NSCalendarIdentifierISO8601" => Some(const_nsstr("iso8601") as usize),
        "_NSLocaleCurrencyCode" => Some(const_nsstr("NSLocaleCurrencyCode") as usize),
        "_NSLocaleCurrencySymbol" => Some(const_nsstr("NSLocaleCurrencySymbol") as usize),
        "_NSLocaleDecimalSeparator" => Some(const_nsstr("NSLocaleDecimalSeparator") as usize),
        "_NSLocaleGroupingSeparator" => Some(const_nsstr("NSLocaleGroupingSeparator") as usize),
        "_NSURLIsExcludedFromBackupKey" => Some(const_nsstr("NSURLIsExcludedFromBackupKey") as usize),
        "_NSURLVolumeAvailableCapacityForImportantUsageKey" => Some(const_nsstr("NSURLVolumeAvailableCapacityForImportantUsageKey") as usize),
        "_NSURLVolumeTotalCapacityKey" => Some(const_nsstr("NSURLVolumeTotalCapacityKey") as usize),
        "_CFArrayCreate" => f(cf_array_create as *const ()),
        "_CFBooleanGetValue" => f(cf_boolean_get_value as *const ()),
        "_CFDataCreateWithBytesNoCopy" => f(cf_data_create as *const ()),
        "_CFLocaleCopyCurrent" => f(cf_locale_copy_current as *const ()),
        "_CFLocaleCopyPreferredLanguages" => f(cf_locale_copy_preferred_langs as *const ()),
        "_CFLocaleGetValue" => f(cf_locale_get_value as *const ()),
        "_CFStringCreateWithBytes" => f(cf_string_create_with_bytes as *const ()),
        "_CFStringGetBytes" => f(cf_string_get_bytes as *const ()),
        "_CFUUIDCreate" => f(cf_uuid_create as *const ()),
        "_CFUUIDCreateFromUUIDBytes" => f(cf_uuid_create as *const ()),
        "_CFUUIDCreateString" => f(cf_uuid_create_string as *const ()),
        "_CFUUIDGetUUIDBytes" => f(cf_uuid_get_bytes as *const ()),
        "_kCFRunLoopCommonModes" => Some(const_nsstr("kCFRunLoopCommonModes") as usize),
        "_kCFStreamPropertySocketNativeHandle" => Some(const_nsstr("kCFStreamPropertySocketNativeHandle") as usize),
        "_NSMetadataItemFSNameKey" => Some(const_nsstr("NSMetadataItemFSNameKey") as usize),
        "_NSMetadataQueryDidFinishGatheringNotification" => Some(const_nsstr("NSMetadataQueryDidFinishGatheringNotification") as usize),
        "_NSMetadataQueryUbiquitousDataScope" => Some(const_nsstr("NSMetadataQueryUbiquitousDataScope") as usize),
        "_NSUbiquityIdentityDidChangeNotification" => Some(const_nsstr("NSUbiquityIdentityDidChangeNotification") as usize),
        "_kCFTypeDictionaryKeyCallBacks" => Some(&raw const CF_CALLBACK_STUB as usize),
        "_kCFTypeDictionaryValueCallBacks" => Some(&raw const CF_CALLBACK_STUB as usize),
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
        "_CFNumberIsFloatType" => f(cf_number_is_float_type as *const ()),
        "_CFBooleanGetTypeID" => f(cf_boolean_get_type_id as *const ()),
        "_CFBundleGetMainBundle" => f(cf_bundle_get_main as *const ()),
        "_CFBundleGetInfoDictionary" => f(cf_bundle_get_info_dict as *const ()),
        "_CFAbsoluteTimeGetCurrent" => f(cf_abs_time_current as *const ()),
        "_CFDictionaryGetValueIfPresent" => f(cf_dict_get_value_if_present as *const ()),
        "_CFRunLoopGetCurrent" => f(cf_runloop_get_main as *const ()),
        "_CFRunLoopPerformBlock" => f(cf_runloop_perform_block as *const ()),
        "_CFNotificationCenterGetLocalCenter" => f(cf_notif_center_local as *const ()),
        "_CFNotificationCenterAddObserver" => f(cf_notif_center_add_obs as *const ()),
        "_CFPreferencesCopyAppValue" => f(cf_prefs_copy_app_val as *const ()),
        "_CFPreferencesSetAppValue" => f(cf_prefs_set_app_val as *const ()),
        "_CFPreferencesAppSynchronize" => f(cf_prefs_app_sync as *const ()),
        "_CFPreferencesCopyKeyList" => f(cf_prefs_copy_key_list as *const ()),
        "_CFPreferencesSetMultiple" => f(cf_prefs_set_multiple as *const ()),
        "_CFStringCreateMutable" => f(cf_string_create_mutable as *const ()),
        "_CFStringCreateCopy" => f(cf_string_create_copy as *const ()),
        "_CFStringAppend" => f(cf_string_append as *const ()),
        "_CFStringAppendCharacters" => f(cf_string_append_chars as *const ()),
        "_CFStringAppendFormat" => f(cf_string_append_format as *const ()),
        "_CFStringCompare" => f(cf_string_compare as *const ()),
        "_CFStringGetCharacterAtIndex" => f(cf_string_get_char_at as *const ()),
        "_CFStringHasPrefix" => f(cf_string_has_prefix as *const ()),
        "_CFStringCreateWithSubstring" => f(cf_string_create_with_substr as *const ()),
        "_CFStringConvertEncodingToNSStringEncoding" => f(cf_string_conv_enc as *const ()),
        "_CFURLCreateStringByAddingPercentEscapes" => f(cf_url_percent_escape as *const ()),
        "_kCFAllocatorSystemDefault" => Some(&raw const K_CF_ALLOCATOR_DEFAULT as usize),
        "_kCFBundleVersionKey" => Some(const_nsstr("CFBundleVersion") as usize),
        "_kCFPreferencesAnyHost" => Some(const_nsstr("kCFPreferencesAnyHost") as usize),
        "_kCFPreferencesCurrentApplication" => Some(const_nsstr("kCFPreferencesCurrentApplication") as usize),
        "_kCFPreferencesCurrentHost" => Some(const_nsstr("kCFPreferencesCurrentHost") as usize),
        "_kCFPreferencesCurrentUser" => Some(const_nsstr("kCFPreferencesCurrentUser") as usize),
        "_kCFRunLoopDefaultMode" => Some(const_nsstr("kCFRunLoopDefaultMode") as usize),
        "_kCFTypeArrayCallBacks" => Some(&raw const CF_CALLBACK_STUB as usize),
        "_kCFAbsoluteTimeIntervalSince1970" => Some(&raw const K_CF_ABS_TIME_1970 as usize),
        _ => cf_ext::symbol(name),
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
extern "C" fn cs_char_is_member(this: Id, _cmd: Sel, ch: u16) -> bool {
    let s = rust_str_from_nsstring(this).unwrap_or_default();
    char::from_u32(ch as u32).is_some_and(|c| s.contains(c))
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
extern "C" fn locale_with_id(_self: Id, _cmd: Sel, ident: Id) -> Id {
    let s = rust_str_from_nsstring(ident).unwrap_or_else(|| "en_US".to_string());
    unsafe { alloc_with_backing(NSLOCALE_CLASS.ptr(), Backing::Str(s)) }
}
extern "C" fn locale_country_code(_self: Id, _cmd: Sel) -> Id {
    nsstring_from_rust("US")
}
extern "C" fn locale_language_code(_self: Id, _cmd: Sel) -> Id {
    nsstring_from_rust("en")
}
extern "C" fn locale_currency_code(_self: Id, _cmd: Sel) -> Id {
    nsstring_from_rust("USD")
}
extern "C" fn locale_currency_symbol(_self: Id, _cmd: Sel) -> Id {
    nsstring_from_rust("$")
}
extern "C" fn locale_decimal_sep(_self: Id, _cmd: Sel) -> Id {
    nsstring_from_rust(".")
}
extern "C" fn locale_grouping_sep(_self: Id, _cmd: Sel) -> Id {
    nsstring_from_rust(",")
}
extern "C" fn locale_country_codes(_self: Id, _cmd: Sel) -> Id {
    let us = nsstring_from_rust("US");
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec![retain(us) as usize])) }
}
extern "C" fn locale_obj_for_key(self_: Id, _cmd: Sel, key: Id) -> Id {
    let k = rust_str_from_nsstring(key).unwrap_or_default();
    let ident = rust_str_from_nsstring(self_).unwrap_or_else(|| "en_US".to_string());
    let (lang, country) = ident.split_once(['_', '-']).unwrap_or((ident.as_str(), "US"));
    match k.as_str() {
        "kCFLocaleIdentifierKey" | "NSLocaleIdentifier" => nsstring_from_rust(&ident),
        "kCFLocaleLanguageCodeKey" | "NSLocaleLanguageCode" => nsstring_from_rust(lang),
        "kCFLocaleCountryCodeKey" | "NSLocaleCountryCode" => nsstring_from_rust(country),
        "kCFLocaleCurrencyCodeKey" | "NSLocaleCurrencyCode" => nsstring_from_rust("USD"),
        "kCFLocaleCurrencySymbolKey" | "NSLocaleCurrencySymbol" => nsstring_from_rust("$"),
        "kCFLocaleDecimalSeparatorKey" | "NSLocaleDecimalSeparator" => nsstring_from_rust("."),
        "kCFLocaleGroupingSeparatorKey" | "NSLocaleGroupingSeparator" => nsstring_from_rust(","),
        _ => std::ptr::null_mut(),
    }
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

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSOperatingSystemVersion {
    major: isize,
    minor: isize,
    patch: isize,
}

extern "C" fn pi_env(_self: Id, _cmd: Sel) -> Id {
    let mut map = HashMap::new();
    for (k, v) in std::env::vars() {
        map.insert(k, nsstring_from_rust(&v) as usize);
    }
    unsafe { alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(map)) }
}

extern "C" fn pi_args(_self: Id, _cmd: Sel) -> Id {
    let args: Vec<usize> = std::env::args()
        .map(|a| nsstring_from_rust(&a) as usize)
        .collect();
    unsafe { alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(args)) }
}

extern "C" fn pi_name(_self: Id, _cmd: Sel) -> Id {
    let name = std::env::args()
        .next()
        .and_then(|p| std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "PearApp".to_string());
    nsstring_from_rust(&name)
}

extern "C" fn pi_os_version_struct(_self: Id, _cmd: Sel) -> NSOperatingSystemVersion {
    NSOperatingSystemVersion { major: 17, minor: 0, patch: 0 }
}

extern "C" fn pi_is_os_at_least(_self: Id, _cmd: Sel, ver: NSOperatingSystemVersion) -> bool {
    ver.major <= 17
}

extern "C" fn pi_uptime(_self: Id, _cmd: Sel) -> f64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() + 100.0
}

extern "C" fn pi_unique_str(_self: Id, _cmd: Sel) -> Id {
    uuid_string(uuid_new(_self, _cmd), _cmd)
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
    *g.entry(s).or_insert_with(|| boxed_objc_slot(nsstring_from_rust(s) as usize))
}

fn empty_dict_singleton() -> usize {
    static D: OnceLock<usize> = OnceLock::new();
    *D.get_or_init(|| {
        let obj = unsafe { alloc_with_backing(NSDICTIONARY_CLASS.ptr(), Backing::Dict(HashMap::new())) } as usize;
        boxed_objc_slot(obj)
    })
}

fn cf_bool_true_ptr() -> usize {
    static B: OnceLock<usize> = OnceLock::new();
    *B.get_or_init(|| {
        let obj = unsafe { alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(true)) } as usize;
        boxed_objc_slot(obj)
    })
}

fn cf_bool_false_ptr() -> usize {
    static B: OnceLock<usize> = OnceLock::new();
    *B.get_or_init(|| {
        let obj = unsafe { alloc_with_backing(NSNUMBER_CLASS.ptr(), Backing::NumBool(false)) } as usize;
        boxed_objc_slot(obj)
    })
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

static CF_CALLBACK_STUB: [usize; 6] = [0; 6];

pub extern "C" fn ns_class_from_string(name: Id) -> *const ClassT {
    let Some(s) = rust_str_from_nsstring(name) else {
        return std::ptr::null();
    };
    runtime::class_named(&s).unwrap_or(std::ptr::null())
}

pub unsafe extern "C" fn cf_array_create(
    _alloc: *const c_void,
    values: *const Id,
    num_values: isize,
    _cb: *const c_void,
) -> Id {
    let mut vec = Vec::new();
    if !values.is_null() && num_values > 0 {
        for idx in 0..num_values as usize {
            vec.push(retain(*values.add(idx)) as usize);
        }
    }
    alloc_with_backing(NSARRAY_CLASS.ptr(), Backing::Arr(vec))
}

pub extern "C" fn cf_boolean_get_value(b: Id) -> bool {
    if let Some(Backing::NumBool(v)) = store().lock().unwrap().get(&(b as usize)) {
        return *v;
    }
    false
}

pub extern "C" fn cf_locale_copy_current() -> Id {
    locale_current(NSLOCALE_CLASS.ptr() as Id, sel("currentLocale"))
}
pub extern "C" fn cf_locale_copy_preferred_langs() -> Id {
    locale_langs(NSLOCALE_CLASS.ptr() as Id, sel("preferredLanguages"))
}
pub extern "C" fn cf_locale_get_value(_loc: Id, _key: Id) -> Id {
    nsstring_from_rust("US")
}

pub unsafe extern "C" fn cf_string_create_with_bytes(
    _alloc: *const c_void,
    bytes: *const u8,
    num_bytes: isize,
    _enc: u32,
    _is_external: bool,
) -> Id {
    let s = if bytes.is_null() || num_bytes <= 0 {
        String::new()
    } else {
        String::from_utf8_lossy(std::slice::from_raw_parts(bytes, num_bytes as usize)).into_owned()
    };
    nsstring_from_rust(&s)
}

pub unsafe extern "C" fn cf_string_get_bytes(
    s: Id,
    loc: isize,
    len: isize,
    _enc: u32,
    _loss_byte: u8,
    _is_ext: bool,
    buf: *mut u8,
    max_buf_len: isize,
    used_buf_len: *mut isize,
) -> isize {
    let text = rust_str_from_nsstring(s).unwrap_or_default();
    let bytes = text.as_bytes();
    let start = (loc.max(0) as usize).min(bytes.len());
    let take = (len.max(0) as usize).min(bytes.len() - start);
    if !buf.is_null() && max_buf_len > 0 {
        let copy_n = take.min(max_buf_len as usize);
        std::ptr::copy_nonoverlapping(bytes[start..].as_ptr(), buf, copy_n);
        if !used_buf_len.is_null() {
            *used_buf_len = copy_n as isize;
        }
    } else if !used_buf_len.is_null() {
        *used_buf_len = take as isize;
    }
    take as isize
}

pub extern "C" fn cf_uuid_create(_alloc: *const c_void) -> Id {
    uuid_new(NSUUID_CLASS.ptr() as Id, sel("UUID"))
}
pub extern "C" fn cf_uuid_create_string(_alloc: *const c_void, u: Id) -> Id {
    uuid_string(u, sel("UUIDString"))
}
#[repr(C)]
pub struct CFUUIDBytes {
    pub bytes: [u8; 16],
}

pub extern "C" fn cf_uuid_get_bytes(_u: Id) -> CFUUIDBytes {
    CFUUIDBytes {
        bytes: [0x50, 0x65, 0x61, 0x72, 0x53, 0x69, 0x6c, 0x69, 0x80, 0, 0, 0, 0, 0, 0, 1],
    }
}

pub extern "C" fn cf_get_type_id(obj: Id) -> usize {
    if obj.is_null() {
        return 0;
    }
    match store().lock().unwrap().get(&(obj as usize)) {
        Some(Backing::Str(_)) => 7,
        Some(Backing::NumI64(_) | Backing::NumF64(_) | Backing::NumBool(_)) => 22,
        Some(Backing::Arr(_)) => 19,
        Some(Backing::Dict(_)) => 18,
        Some(Backing::Bytes(_)) => 20,
        _ => 1,
    }
}

static K_CF_ABS_TIME_1970: f64 = 978_307_200.0;

pub extern "C" fn cf_bundle_get_main() -> Id {
    bundle_main(NSBUNDLE_CLASS.ptr() as Id, sel("mainBundle"))
}
pub extern "C" fn cf_bundle_get_info_dict(bundle: Id) -> Id {
    let b = if bundle.is_null() { cf_bundle_get_main() } else { bundle };
    bundle_info(b, sel("infoDictionary"))
}
pub extern "C" fn cf_abs_time_current() -> f64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    secs - K_CF_ABS_TIME_1970
}
pub unsafe extern "C" fn cf_dict_get_value_if_present(
    dict: Id,
    key: Id,
    value: *mut *const c_void,
) -> bool {
    if dict.is_null() {
        return false;
    }
    let Some(k) = rust_str_from_nsstring(key) else {
        return false;
    };
    let found = store().lock().unwrap().get(&(dict as usize)).and_then(|b| match b {
        Backing::Dict(m) => m.get(&k).copied(),
        _ => None,
    });
    if let Some(v) = found {
        if !value.is_null() {
            *value = v as *const c_void;
        }
        true
    } else {
        false
    }
}
pub extern "C" fn cf_runloop_perform_block(_rl: Id, _mode: Id, _block: *const c_void) {}
pub extern "C" fn cf_notif_center_local() -> Id {
    unsafe { alloc_with_backing(NSNOTIFICATIONCENTER_CLASS.ptr(), Backing::Arr(Vec::new())) }
}
pub extern "C" fn cf_notif_center_add_obs(
    _center: Id,
    _obs: *const c_void,
    _cb: *const c_void,
    _name: Id,
    _obj: *const c_void,
    _behavior: usize,
) {}
pub extern "C" fn cf_prefs_copy_app_val(key: Id, _app_id: Id) -> Id {
    let d = ud_standard(NSUSERDEFAULTS_CLASS.ptr() as Id, sel("standardUserDefaults"));
    let v = ud_obj_for_key(d, sel("objectForKey:"), key);
    if v.is_null() { std::ptr::null_mut() } else { retain(v) }
}
pub extern "C" fn cf_prefs_set_app_val(key: Id, val: Id, _app_id: Id) {
    let d = ud_standard(NSUSERDEFAULTS_CLASS.ptr() as Id, sel("standardUserDefaults"));
    ud_set_obj_for_key(d, sel("setObject:forKey:"), val, key);
}
pub extern "C" fn cf_prefs_app_sync(_app_id: Id) -> bool { true }
pub extern "C" fn cf_prefs_copy_key_list(_app_id: Id, _user: Id, _host: Id) -> Id {
    let d = ud_standard(NSUSERDEFAULTS_CLASS.ptr() as Id, sel("standardUserDefaults"));
    dict_all_keys(d, sel("allKeys"))
}
pub extern "C" fn cf_prefs_set_multiple(_set: Id, _rem: Id, _app: Id, _user: Id, _host: Id) {}
pub extern "C" fn cf_string_create_mutable(_alloc: *const c_void, _max_len: isize) -> Id {
    unsafe { alloc_with_backing(NSMUTABLESTRING_CLASS.ptr(), Backing::Str(String::new())) }
}
pub extern "C" fn cf_string_create_copy(_alloc: *const c_void, s: Id) -> Id {
    let text = rust_str_from_nsstring(s).unwrap_or_default();
    nsstring_from_rust(&text)
}
pub extern "C" fn cf_string_append(s: Id, appended: Id) {
    str_append(s, sel("appendString:"), appended);
}
pub unsafe extern "C" fn cf_string_append_chars(s: Id, chars: *const u16, num: isize) {
    if s.is_null() || chars.is_null() || num <= 0 { return; }
    let slice = std::slice::from_raw_parts(chars, num as usize);
    let tail = String::from_utf16_lossy(slice);
    let add = nsstring_from_rust(&tail);
    str_append(s, sel("appendString:"), add);
}
pub extern "C" fn cf_string_append_format(s: Id, _opts: *const c_void, fmt: Id) {
    str_append(s, sel("appendString:"), fmt);
}
pub extern "C" fn cf_string_compare(a: Id, b: Id, opts: usize) -> isize {
    let sa = rust_str_from_nsstring(a).unwrap_or_default();
    let sb = rust_str_from_nsstring(b).unwrap_or_default();
    let cmp = if (opts & 1) != 0 {
        sa.to_ascii_lowercase().cmp(&sb.to_ascii_lowercase())
    } else {
        sa.cmp(&sb)
    };
    cmp as isize
}
pub extern "C" fn cf_string_get_char_at(s: Id, idx: isize) -> u16 {
    let text = rust_str_from_nsstring(s).unwrap_or_default();
    text.encode_utf16().nth(idx.max(0) as usize).unwrap_or(0)
}
pub extern "C" fn cf_string_has_prefix(s: Id, pfx: Id) -> bool {
    str_has_prefix(s, sel("hasPrefix:"), pfx)
}
pub extern "C" fn cf_string_create_with_substr(_alloc: *const c_void, s: Id, loc: isize, len: isize) -> Id {
    let text = rust_str_from_nsstring(s).unwrap_or_default();
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let start = (loc.max(0) as usize).min(utf16.len());
    let end = start.saturating_add(len.max(0) as usize).min(utf16.len());
    nsstring_from_rust(&String::from_utf16_lossy(&utf16[start..end]))
}
pub extern "C" fn cf_string_conv_enc(cf_enc: u32) -> usize {
    match cf_enc {
        0x0800_0100 => 4, // kCFStringEncodingUTF8 -> NSUTF8StringEncoding
        0x0600 => 1,      // kCFStringEncodingASCII -> NSASCIIStringEncoding
        0x0201 => 5,      // kCFStringEncodingISOLatin1 -> NSISOLatin1StringEncoding
        _ => 4,
    }
}
pub extern "C" fn cf_number_is_float_type(num: Id) -> bool {
    matches!(store().lock().unwrap().get(&(num as usize)), Some(Backing::NumF64(_)))
}
pub extern "C" fn cf_boolean_get_type_id() -> usize { 21 }
pub extern "C" fn cf_url_percent_escape(_alloc: *const c_void, orig: Id, _leave: Id, _legal: Id, _enc: u32) -> Id {
    let s = rust_str_from_nsstring(orig).unwrap_or_default();
    nsstring_from_rust(&s)
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
