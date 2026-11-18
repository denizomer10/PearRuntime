//! Opt-in public Metal API diagnostics (PEAR_DEBUG_METAL). No shader or
//! texture is modified: failed library/pipeline creations are logged.

use crate::rt;
use pear_objc::{msg0, nsstring, Id, Imp, Sel, NIL};
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

#[link(name = "Metal", kind = "framework")]
extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}

static PIPELINE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static LIB_DATA: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static LIB_SOURCE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe fn failure(operation: &str, error: Id) {
    static FAILURES: AtomicU32 = AtomicU32::new(0);
    if FAILURES.fetch_add(1, Ordering::Relaxed) >= 32 {
        return;
    }
    let (domain, code, reason) = if error.is_null() {
        ("unavailable".to_string(), 0isize, "unavailable".to_string())
    } else {
        let desc = rt::ns_string(msg0(error, "localizedDescription"));
        let first = desc.split('\n').next().unwrap_or("").to_string();
        let domain = rt::ns_string(msg0(error, "domain"));
        (
            if domain.is_empty() { "unavailable".into() } else { domain },
            msg0::<isize>(error, "code"),
            if first.is_empty() { "unavailable".into() } else { first },
        )
    };
    rt::log(&format!(
        "[PearRuntime][metal-diagnostic] {operation} failed domain={domain} code={code} reason={reason}"
    ));
}

unsafe extern "C" fn new_pipeline(s: Id, c: Sel, descriptor: Id, error: *mut Id) -> Id {
    let mut local: Id = NIL;
    let dest = if error.is_null() { &mut local as *mut Id } else { error };
    let f: unsafe extern "C" fn(Id, Sel, Id, *mut Id) -> Id = std::mem::transmute(PIPELINE.load(Ordering::Acquire));
    let result = f(s, c, descriptor, dest);
    if result.is_null() {
        failure("render pipeline", *dest);
    }
    result
}

unsafe extern "C" fn new_library_data(s: Id, c: Sel, bytes: Id, error: *mut Id) -> Id {
    let mut local: Id = NIL;
    let dest = if error.is_null() { &mut local as *mut Id } else { error };
    let f: unsafe extern "C" fn(Id, Sel, Id, *mut Id) -> Id = std::mem::transmute(LIB_DATA.load(Ordering::Acquire));
    let result = f(s, c, bytes, dest);
    if result.is_null() {
        failure("compiled library", *dest);
    }
    result
}

unsafe extern "C" fn new_library_source(s: Id, c: Sel, text: Id, options: Id, error: *mut Id) -> Id {
    let mut local: Id = NIL;
    let dest = if error.is_null() { &mut local as *mut Id } else { error };
    let f: unsafe extern "C" fn(Id, Sel, Id, Id, *mut Id) -> Id = std::mem::transmute(LIB_SOURCE.load(Ordering::Acquire));
    let result = f(s, c, text, options, dest);
    if result.is_null() {
        failure("source library", *dest);
    }
    result
}

unsafe fn hook(cls: pear_objc::Class, selector: &str, imp: Imp, slot: &AtomicPtr<std::ffi::c_void>) {
    let m = rt::class_getInstanceMethod(cls, pear_objc::sel(selector));
    if !m.is_null() {
        slot.store(rt::method_getImplementation(m) as *mut _, Ordering::Release);
        rt::method_setImplementation(m, imp);
    }
}

pub fn init() {
    if !rt::module_enabled("graphics-metal") || !rt::env_set("PEAR_DEBUG_METAL") {
        return;
    }
    unsafe {
        let device = MTLCreateSystemDefaultDevice();
        if device.is_null() {
            return;
        }
        let cls = rt::object_getClass(device);
        hook(cls, "newRenderPipelineStateWithDescriptor:error:", new_pipeline as Imp, &PIPELINE);
        hook(cls, "newLibraryWithData:error:", new_library_data as Imp, &LIB_DATA);
        hook(cls, "newLibraryWithSource:options:error:", new_library_source as Imp, &LIB_SOURCE);
        pear_objc::objc_release(device);
        let _ = nsstring;
        rt::log("[PearRuntime][metal-diagnostic] library and pipeline failures enabled (limit=32)");
    }
}
