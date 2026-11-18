//! Shared runtime helpers for the Mac Catalyst modules: logging, module
//! gating, method replacement, block ABI and libdispatch.

use pear_objc::{cls, msg0, nsstring, sel, Class, Id, Imp, Sel};
use std::ffi::{c_char, c_int, c_void, CStr, CString};

pub type Method = *mut c_void;

extern "C" {
    pub fn class_getInstanceMethod(cls: Class, name: Sel) -> Method;
    pub fn class_getClassMethod(cls: Class, name: Sel) -> Method;
    pub fn class_copyMethodList(cls: Class, count: *mut u32) -> *mut Method;
    pub fn method_getName(m: Method) -> Sel;
    pub fn method_getImplementation(m: Method) -> Imp;
    pub fn method_setImplementation(m: Method, imp: Imp) -> Imp;
    pub fn method_getNumberOfArguments(m: Method) -> u32;
    pub fn class_addMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> bool;
    pub fn class_getSuperclass(cls: Class) -> Class;
    pub fn class_getName(cls: Class) -> *const c_char;
    pub fn object_getClass(obj: Id) -> Class;
    pub fn sel_getName(s: Sel) -> *const c_char;
    pub fn free(p: *mut c_void);
    pub fn getenv(name: *const c_char) -> *const c_char;
    pub fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    pub fn strcasestr(haystack: *const c_char, needle: *const c_char) -> *const c_char;
    fn _Block_copy(block: *const c_void) -> *mut c_void;
    fn _Block_release(block: *const c_void);
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));

    // core: PearRuntime.mm (C++ linkage) until core is ported.
    #[link_name = "_Z7PearLogP8NSStringz"]
    fn PearLog(format: Id, ...);
}

pub const RTLD_LAZY: c_int = 0x1;
pub const RTLD_GLOBAL: c_int = 0x8;

/// `[PearRuntime]…` line through PearLog (NSLog + PEAR_LOG_FILE).
pub fn log(message: &str) {
    let c = CString::new(message.replace('\0', "")).unwrap_or_default();
    unsafe { PearLog(nsstring("%s"), c.as_ptr()) };
}

/// The environment variable is set (any value, even empty), as `getenv() != NULL`.
pub fn env_set(name: &str) -> bool {
    let c = CString::new(name).unwrap_or_default();
    unsafe { !getenv(c.as_ptr()).is_null() }
}

/// `PEAR_MODULE_GUARD()`: false when the module is switched off for this launch.
pub fn module_enabled(module: &str) -> bool {
    crate::core::registry::enabled(module)
}

pub unsafe fn cstr<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        ""
    } else {
        CStr::from_ptr(p).to_str().unwrap_or("")
    }
}

pub unsafe fn class_name(c: Class) -> &'static str {
    cstr(class_getName(c))
}

pub unsafe fn dlopen_path(path: &str, mode: c_int) -> *mut c_void {
    let c = CString::new(path).unwrap_or_default();
    dlopen(c.as_ptr(), mode)
}

/// `NSClassFromString`.
pub unsafe fn class_named(name: &str) -> Class {
    cls(name)
}

/// Replaces an existing method's implementation, or adds it with `types`.
/// Returns the previous implementation (null when the method was added).
pub unsafe fn replace_instance(c: Class, selector: &str, imp: Imp, types: &str) -> Imp {
    if c.is_null() {
        return std::ptr::null();
    }
    let s = sel(selector);
    let m = class_getInstanceMethod(c, s);
    if !m.is_null() {
        method_setImplementation(m, imp)
    } else {
        let t = CString::new(types).unwrap_or_default();
        class_addMethod(c, s, imp, t.as_ptr());
        std::ptr::null()
    }
}

/// Class-method counterpart of [`replace_instance`].
pub unsafe fn replace_class(c: Class, selector: &str, imp: Imp, types: &str) -> Imp {
    if c.is_null() {
        return std::ptr::null();
    }
    let s = sel(selector);
    let m = class_getClassMethod(c, s);
    if !m.is_null() {
        method_setImplementation(m, imp)
    } else {
        let t = CString::new(types).unwrap_or_default();
        class_addMethod(object_getClass(c as Id), s, imp, t.as_ptr());
        std::ptr::null()
    }
}

#[repr(C)]
struct BlockLayout {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
}

/// The block's invoke function, cast to the caller's signature (first
/// argument is always the block itself).
pub unsafe fn block_invoke<F: Copy>(block: Id) -> Option<F> {
    if block.is_null() {
        return None;
    }
    let invoke = (*(block as *const BlockLayout)).invoke;
    if invoke.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy(&invoke))
    }
}

struct MainCall {
    block: *mut c_void,
    call: unsafe fn(Id, usize),
    arg: usize,
}

extern "C" fn main_call_trampoline(ctx: *mut c_void) {
    unsafe {
        let job = Box::from_raw(ctx as *mut MainCall);
        (job.call)(job.block as Id, job.arg);
        _Block_release(job.block);
    }
}

/// `dispatch_async(main, ^{ call(block, arg); })`, keeping the block alive.
pub unsafe fn async_main_with_block(block: Id, arg: usize, call: unsafe fn(Id, usize)) {
    if block.is_null() {
        return;
    }
    let job = Box::new(MainCall { block: _Block_copy(block), call, arg });
    dispatch_async_f(&_dispatch_main_q, Box::into_raw(job) as *mut c_void, main_call_trampoline);
}

/// Entry of the `__DATA,__interpose` table dyld applies at load.
#[repr(C)]
pub struct Interpose {
    pub replacement: *const c_void,
    pub replacee: *const c_void,
}
unsafe impl Sync for Interpose {}

/// `msg0` returning an autoreleased object from a +1 one.
pub unsafe fn autoreleased(obj: Id) -> Id {
    pear_objc::autoreleased(obj)
}

pub unsafe fn ns_string(obj: Id) -> String {
    pear_objc::nsstring_to_string(obj).unwrap_or_default()
}

pub unsafe fn description(obj: Id) -> String {
    if obj.is_null() {
        return String::new();
    }
    ns_string(msg0(obj, "description"))
}
