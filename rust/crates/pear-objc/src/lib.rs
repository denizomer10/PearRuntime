//! Minimal zero-dependency Objective-C & CoreFoundation/CoreGraphics FFI
//! helpers for PearRuntime.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};

pub type Id = *mut c_void;
pub type Class = *mut c_void;
pub type Sel = *mut c_void;
pub type Imp = *const c_void;

pub const NIL: Id = std::ptr::null_mut();
pub const OBJC_ASSOCIATION_RETAIN: usize = 1;
pub const OBJC_ASSOCIATION_RETAIN_NONATOMIC: usize = 1;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct UIEdgeInsets {
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct NSRange {
    pub location: usize,
    pub length: usize,
}

#[link(name = "objc", kind = "dylib")]
extern "C" {
    pub fn objc_getClass(name: *const c_char) -> Class;
    pub fn objc_getProtocol(name: *const c_char) -> *mut c_void;
    pub fn objc_lookUpClass(name: *const c_char) -> Class;
    pub fn object_getClass(obj: Id) -> Class;
    pub fn object_isClass(obj: Id) -> bool;
    pub fn class_getName(cls: Class) -> *const c_char;
    pub fn class_getSuperclass(cls: Class) -> Class;
    pub fn class_addMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> bool;
    pub fn class_replaceMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> Imp;
    pub fn class_respondsToSelector(cls: Class, sel: Sel) -> bool;
    pub fn sel_registerName(str: *const c_char) -> Sel;
    pub fn sel_getName(sel: Sel) -> *const c_char;
    pub fn objc_retain(obj: Id) -> Id;
    pub fn objc_release(obj: Id);
    pub fn objc_autorelease(obj: Id) -> Id;
    pub fn objc_autoreleasePoolPush() -> *mut c_void;
    pub fn objc_autoreleasePoolPop(pool: *mut c_void);
    pub fn objc_setAssociatedObject(obj: Id, key: *const c_void, value: Id, policy: usize);
    pub fn objc_getAssociatedObject(obj: Id, key: *const c_void) -> Id;
    pub fn objc_msgSend();
    pub fn objc_msgSendSuper();
}

#[inline]
pub fn sel(name: &str) -> Sel {
    let c = CString::new(name).expect("selector contains interior NUL");
    unsafe { sel_registerName(c.as_ptr()) }
}

#[inline]
pub fn cls(name: &str) -> Class {
    let c = CString::new(name).expect("class name contains interior NUL");
    unsafe { objc_getClass(c.as_ptr()) }
}

#[inline]
pub unsafe fn add_method(cls: Class, selector: &str, imp: Imp, types: &str) -> bool {
    let s = sel(selector);
    let t = CString::new(types).expect("types contains interior NUL");
    class_addMethod(cls, s, imp, t.as_ptr())
}

#[inline]
pub unsafe fn replace_method(cls: Class, selector: &str, imp: Imp, types: &str) -> Imp {
    let s = sel(selector);
    let t = CString::new(types).expect("types contains interior NUL");
    class_replaceMethod(cls, s, imp, t.as_ptr())
}

#[inline]
pub unsafe fn msg0<R>(obj: Id, selector: &str) -> R {
    let f: unsafe extern "C" fn(Id, Sel) -> R = std::mem::transmute(objc_msgSend as *const c_void);
    f(obj, sel(selector))
}

#[inline]
pub unsafe fn msg1<A1, R>(obj: Id, selector: &str, a1: A1) -> R {
    let f: unsafe extern "C" fn(Id, Sel, A1) -> R = std::mem::transmute(objc_msgSend as *const c_void);
    f(obj, sel(selector), a1)
}

#[inline]
pub unsafe fn msg2<A1, A2, R>(obj: Id, selector: &str, a1: A1, a2: A2) -> R {
    let f: unsafe extern "C" fn(Id, Sel, A1, A2) -> R = std::mem::transmute(objc_msgSend as *const c_void);
    f(obj, sel(selector), a1, a2)
}

#[inline]
pub unsafe fn msg3<A1, A2, A3, R>(obj: Id, selector: &str, a1: A1, a2: A2, a3: A3) -> R {
    let f: unsafe extern "C" fn(Id, Sel, A1, A2, A3) -> R = std::mem::transmute(objc_msgSend as *const c_void);
    f(obj, sel(selector), a1, a2, a3)
}

#[inline]
pub unsafe fn msg4<A1, A2, A3, A4, R>(obj: Id, selector: &str, a1: A1, a2: A2, a3: A3, a4: A4) -> R {
    let f: unsafe extern "C" fn(Id, Sel, A1, A2, A3, A4) -> R = std::mem::transmute(objc_msgSend as *const c_void);
    f(obj, sel(selector), a1, a2, a3, a4)
}

pub unsafe fn nsstring(s: &str) -> Id {
    let c = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
    let ns_str_cls = cls("NSString");
    msg1::<*const c_char, Id>(ns_str_cls, "stringWithUTF8String:", c.as_ptr())
}

pub unsafe fn nsstring_to_string(obj: Id) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    let utf8: *const c_char = msg0(obj, "UTF8String");
    if utf8.is_null() {
        return None;
    }
    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

pub unsafe fn autorelease_pool<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let pool = objc_autoreleasePoolPush();
    let res = f();
    objc_autoreleasePoolPop(pool);
    res
}

pub unsafe fn responds_to(obj: Id, selector: &str) -> bool {
    if obj.is_null() {
        return false;
    }
    let ok: c_int = msg1::<Sel, c_int>(obj, "respondsToSelector:", sel(selector));
    ok != 0
}

pub unsafe fn class_name(obj: Id) -> &'static str {
    if obj.is_null() {
        return "nil";
    }
    let c = object_getClass(obj);
    let ptr = class_getName(c);
    if ptr.is_null() {
        return "?";
    }
    CStr::from_ptr(ptr).to_str().unwrap_or("?")
}

pub unsafe fn nsdata_from_bytes(bytes: &[u8]) -> Id {
    let data_cls = cls("NSData");
    msg2::<*const c_void, usize, Id>(
        data_cls,
        "dataWithBytes:length:",
        bytes.as_ptr() as *const c_void,
        bytes.len(),
    )
}

pub unsafe fn nsdata_to_vec(obj: Id) -> Option<Vec<u8>> {
    if obj.is_null() {
        return None;
    }
    let len: usize = msg0(obj, "length");
    if len == 0 {
        return Some(Vec::new());
    }
    let ptr: *const u8 = msg0(obj, "bytes");
    if ptr.is_null() {
        return Some(Vec::new());
    }
    Some(std::slice::from_raw_parts(ptr, len).to_vec())
}

pub unsafe fn nsarray_count(arr: Id) -> usize {
    if arr.is_null() {
        0
    } else {
        msg0(arr, "count")
    }
}

pub unsafe fn nsarray_object_at(arr: Id, idx: usize) -> Id {
    msg1::<usize, Id>(arr, "objectAtIndex:", idx)
}

pub unsafe fn nsarray_from_ids(items: &[Id]) -> Id {
    let arr_cls = cls("NSMutableArray");
    let arr: Id = msg1::<usize, Id>(arr_cls, "arrayWithCapacity:", items.len());
    for &item in items {
        if !item.is_null() {
            msg1::<Id, ()>(arr, "addObject:", item);
        }
    }
    arr
}

pub unsafe fn nsnumber_bool(v: bool) -> Id {
    msg1::<c_int, Id>(cls("NSNumber"), "numberWithBool:", if v { 1 } else { 0 })
}

pub unsafe fn nsnumber_i64(v: i64) -> Id {
    msg1::<i64, Id>(cls("NSNumber"), "numberWithLongLong:", v)
}

pub unsafe fn nsnumber_f64(v: f64) -> Id {
    msg1::<f64, Id>(cls("NSNumber"), "numberWithDouble:", v)
}

pub unsafe fn is_kind_of(obj: Id, class_name_str: &str) -> bool {
    if obj.is_null() {
        return false;
    }
    let target = cls(class_name_str);
    if target.is_null() {
        return false;
    }
    let r: c_uint = msg1::<Class, c_uint>(obj, "isKindOfClass:", target);
    r != 0
}
