//! Clean-room: ABI layouts and semantics from Apple's open-source objc4
//! (APSL 2.0, reference only, nothing copied), the public objc headers and
//! documentation, and what clang emits for the analysed apps; libobjc
//! imports chosen from the apps' census (docs/census).
//!
//! PearRuntime's own Objective-C runtime: Apple's 64-bit ABI, in Rust, for
//! images loaded by pear-dyld, on any arm64 host.
//!
//! Using it: load an image with pear-dyld, resolving its libobjc imports
//! through [`symbol`]; then [`register_image`] (selectors, classes,
//! categories), [`call_load_methods`], and the image's initializers.
//!
//! Implemented: class/metaclass realization from `class_ro_t` (pointer and
//! relative method lists, non-fragile ivar sliding), categories, protocols
//! (conformance by name), `objc_msgSend`/`objc_msgSendSuper2` with
//! `+initialize`, `+load`, NSObject's core methods, retain/release in a
//! side table, `.cxx_destruct`, zeroing weak references, autorelease pools
//! and the ARC entry points clang emits for them. Not yet: a fast message
//! cache in assembly, forwarding (`forwardInvocation:`), exceptions,
//! associated objects, Swift classes, tagged pointers, blocks, properties
//! and the reflection API beyond the basics.
//!
//! None of this replaces the host's libobjc: the entry points are not
//! exported under their C names, they are handed to the loader.

pub mod abi;
pub mod message;
pub mod object;
pub mod runtime;

use abi::*;
use std::ffi::{c_char, c_void};

pub use runtime::{call_load_methods, register_image, ImageReport};

// ---- C entry points (libobjc's names, minus the leading underscore) -----

extern "C" fn objc_retain(obj: Id) -> Id {
    object::retain(obj)
}
extern "C" fn objc_release(obj: Id) {
    object::release(obj)
}
/// Targets of the objc_retain_xN / objc_release_xN register variants.
pub(crate) extern "C" fn objc_retain_entry(obj: Id) -> Id {
    object::retain(obj)
}
pub(crate) extern "C" fn objc_release_entry(obj: Id) {
    object::release(obj)
}
extern "C" fn objc_autorelease(obj: Id) -> Id {
    object::autorelease(obj)
}
extern "C" fn objc_retain_autorelease(obj: Id) -> Id {
    object::autorelease(object::retain(obj))
}
extern "C" fn objc_autorelease_return_value(obj: Id) -> Id {
    object::autorelease(obj)
}
extern "C" fn objc_retain_autorelease_return_value(obj: Id) -> Id {
    object::autorelease(object::retain(obj))
}
extern "C" fn objc_retain_autoreleased_return_value(obj: Id) -> Id {
    object::retain(obj)
}
extern "C" fn objc_unsafe_claim_autoreleased_return_value(obj: Id) -> Id {
    obj
}
extern "C" fn objc_claim_autoreleased_return_value(obj: Id) -> Id {
    object::retain(obj)
}
unsafe extern "C" fn objc_store_strong(location: *mut Id, obj: Id) {
    let old = *location;
    if old == obj {
        return;
    }
    object::retain(obj);
    *location = obj;
    object::release(old);
}
extern "C" fn objc_autorelease_pool_push() -> *mut c_void {
    object::pool_push()
}
extern "C" fn objc_autorelease_pool_pop(token: *mut c_void) {
    object::pool_pop(token)
}
unsafe extern "C" fn objc_init_weak(location: *mut Id, obj: Id) -> Id {
    *location = std::ptr::null_mut();
    object::store_weak(location, obj)
}
unsafe extern "C" fn objc_store_weak(location: *mut Id, obj: Id) -> Id {
    object::store_weak(location, obj)
}
unsafe extern "C" fn objc_destroy_weak(location: *mut Id) {
    object::store_weak(location, std::ptr::null_mut());
}
unsafe extern "C" fn objc_load_weak_retained(location: *mut Id) -> Id {
    object::load_weak_retained(location)
}
unsafe extern "C" fn objc_load_weak(location: *mut Id) -> Id {
    object::autorelease(object::load_weak_retained(location))
}
unsafe extern "C" fn objc_copy_weak(to: *mut Id, from: *mut Id) {
    let obj = object::load_weak_retained(from);
    objc_init_weak(to, obj);
    object::release(obj);
}
unsafe extern "C" fn objc_move_weak(to: *mut Id, from: *mut Id) {
    objc_copy_weak(to, from);
    objc_destroy_weak(from);
}

unsafe fn send(obj: Id, name: &str) -> Id {
    if obj.is_null() {
        return obj;
    }
    let s = runtime::sel(name);
    let d = message::lookup_for_send(obj, s);
    let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
    f(d.receiver, s)
}
unsafe extern "C" fn objc_alloc(cls: Id) -> Id {
    send(cls, "alloc")
}
unsafe extern "C" fn objc_alloc_init(cls: Id) -> Id {
    send(send(cls, "alloc"), "init")
}
unsafe extern "C" fn objc_opt_new(cls: Id) -> Id {
    send(cls, "new")
}
unsafe extern "C" fn objc_opt_class(obj: Id) -> Id {
    send(obj, "class")
}
unsafe extern "C" fn objc_opt_self(obj: Id) -> Id {
    obj
}
unsafe extern "C" fn objc_opt_is_kind_of_class(obj: Id, cls: *const ClassT) -> bool {
    if obj.is_null() {
        return false;
    }
    let s = runtime::sel("isKindOfClass:");
    let d = message::lookup_for_send(obj, s);
    let f: extern "C" fn(Id, Sel, *const ClassT) -> bool = std::mem::transmute(d.imp);
    f(d.receiver, s, cls)
}
unsafe extern "C" fn objc_opt_responds_to_selector(obj: Id, sel: Sel) -> bool {
    if obj.is_null() {
        return false;
    }
    let s = runtime::sel("respondsToSelector:");
    let d = message::lookup_for_send(obj, s);
    let f: extern "C" fn(Id, Sel, Sel) -> bool = std::mem::transmute(d.imp);
    f(d.receiver, s, sel)
}

// The reflection basics.
unsafe extern "C" fn sel_register_name(name: *const c_char) -> Sel {
    runtime::intern_c(name)
}
extern "C" fn sel_get_name(s: Sel) -> *const c_char {
    s
}
unsafe extern "C" fn objc_get_class(name: *const c_char) -> *const ClassT {
    let name = std::ffi::CStr::from_ptr(name).to_string_lossy();
    runtime::class_named(&name).unwrap_or(std::ptr::null())
}
unsafe extern "C" fn object_get_class(obj: Id) -> *const ClassT {
    runtime::object_class(obj)
}
extern "C" fn class_get_superclass(cls: *const ClassT) -> *const ClassT {
    runtime::superclass(cls)
}
unsafe extern "C" fn class_respond_to_selector(cls: *const ClassT, s: Sel) -> bool {
    !cls.is_null() && runtime::lookup(cls, s).is_some()
}
unsafe extern "C" fn class_add_method(cls: *const ClassT, s: Sel, imp: Imp, _types: *const c_char) -> bool {
    if cls.is_null() || runtime::own_method(cls, s).is_some() {
        return false;
    }
    runtime::add_methods(cls, &[MethodDesc { name: s, types: _types, imp }]);
    true
}

/// While a C++ or Objective-C exception unwinds through image code: no
/// handlers yet, so cleanups are skipped (`_URC_CONTINUE_UNWIND`).
extern "C" fn objc_personality_v0() -> i32 {
    8
}

/// The address of a libobjc symbol (Mach-O name, with its `_`), for a
/// pear-dyld resolver.
pub fn symbol(name: &str) -> Option<usize> {
    object::init_runtime();
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_objc_msgSend" => f(message::objc_msg_send as *const ()),
        "_objc_msgSendSuper2" => f(message::objc_msg_send_super2 as *const ()),
        "_objc_msgSendSuper" => f(message::objc_msg_send_super as *const ()),
        "__objc_msgForward" => f(message::objc_msg_forward as *const ()),
        "_OBJC_CLASS_$_NSObject" => Some(object::NSOBJECT_CLASS.ptr() as usize),
        "_OBJC_METACLASS_$_NSObject" => Some(object::NSOBJECT_META.ptr() as usize),
        "__objc_empty_cache" => Some(&object::EMPTY_CACHE as *const _ as usize),
        "_objc_retain" => f(objc_retain as *const ()),
        "_objc_release" => f(objc_release as *const ()),
        "_objc_autorelease" => f(objc_autorelease as *const ()),
        "_objc_retainAutorelease" => f(objc_retain_autorelease as *const ()),
        "_objc_autoreleaseReturnValue" => f(objc_autorelease_return_value as *const ()),
        "_objc_retainAutoreleaseReturnValue" => f(objc_retain_autorelease_return_value as *const ()),
        "_objc_retainAutoreleasedReturnValue" => f(objc_retain_autoreleased_return_value as *const ()),
        "_objc_unsafeClaimAutoreleasedReturnValue" => f(objc_unsafe_claim_autoreleased_return_value as *const ()),
        "_objc_claimAutoreleasedReturnValue" => f(objc_claim_autoreleased_return_value as *const ()),
        "_objc_storeStrong" => f(objc_store_strong as *const ()),
        "_objc_autoreleasePoolPush" => f(objc_autorelease_pool_push as *const ()),
        "_objc_autoreleasePoolPop" => f(objc_autorelease_pool_pop as *const ()),
        "_objc_initWeak" => f(objc_init_weak as *const ()),
        "_objc_storeWeak" => f(objc_store_weak as *const ()),
        "_objc_destroyWeak" => f(objc_destroy_weak as *const ()),
        "_objc_loadWeakRetained" => f(objc_load_weak_retained as *const ()),
        "_objc_loadWeak" => f(objc_load_weak as *const ()),
        "_objc_copyWeak" => f(objc_copy_weak as *const ()),
        "_objc_moveWeak" => f(objc_move_weak as *const ()),
        "_objc_alloc" => f(objc_alloc as *const ()),
        "_objc_alloc_init" => f(objc_alloc_init as *const ()),
        "_objc_opt_new" => f(objc_opt_new as *const ()),
        "_objc_opt_class" => f(objc_opt_class as *const ()),
        "_objc_opt_self" => f(objc_opt_self as *const ()),
        "_objc_opt_isKindOfClass" => f(objc_opt_is_kind_of_class as *const ()),
        "_objc_opt_respondsToSelector" => f(objc_opt_responds_to_selector as *const ()),
        "_sel_registerName" | "_sel_getUid" => f(sel_register_name as *const ()),
        "_sel_getName" => f(sel_get_name as *const ()),
        "_objc_getClass" | "_objc_lookUpClass" => f(objc_get_class as *const ()),
        "_object_getClass" => f(object_get_class as *const ()),
        "_class_getSuperclass" => f(class_get_superclass as *const ()),
        "_class_respondsToSelector" => f(class_respond_to_selector as *const ()),
        "_class_addMethod" => f(class_add_method as *const ()),
        "___objc_personality_v0" => f(objc_personality_v0 as *const ()),
        _ => message::register_variant(name),
    }
}
