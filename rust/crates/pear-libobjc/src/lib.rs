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
pub mod blocks;
pub mod exceptions;
pub mod message;
pub mod object;
pub mod props;
pub mod reflection;
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
unsafe extern "C" fn objc_read_class_pair(cls: *const ClassT, _info: *const c_void) -> *const ClassT {
    if !cls.is_null() && runtime::class(cls).is_none() {
        runtime::realize(cls);
    }
    cls
}
unsafe extern "C" fn objc_set_hook_get_class(new_hook: *const c_void, old_hook_out: *mut *const c_void) {
    static HOOK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let prev = HOOK.swap(new_hook as usize, std::sync::atomic::Ordering::AcqRel);
    if !old_hook_out.is_null() {
        *old_hook_out = prev as *const c_void;
    }
}

/// The address of a libobjc symbol (Mach-O name, with its `_`), for a
/// pear-dyld resolver (and libclosure symbols imported from libSystem).
pub fn symbol(name: &str) -> Option<usize> {
    object::init_runtime();
    blocks::init_blocks();
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_objc_msgSend" => f(message::objc_msg_send as *const ()),
        "_objc_msgSendSuper2" => f(message::objc_msg_send_super2 as *const ()),
        "_objc_msgSendSuper" => f(message::objc_msg_send_super as *const ()),
        "__objc_msgForward" => f(message::objc_msg_forward as *const ()),
        "_OBJC_CLASS_$_NSObject" => Some(object::NSOBJECT_CLASS.ptr() as usize),
        "_OBJC_METACLASS_$_NSObject" => Some(object::NSOBJECT_META.ptr() as usize),
        "__objc_empty_cache" => Some(&object::EMPTY_CACHE as *const _ as usize),
        "__objc_empty_vtable" => Some(&exceptions::OBJC_EMPTY_VTABLE as *const _ as usize),
        "_objc_ehtype_vtable" | "__objc_ehtype_vtable" => Some(&exceptions::OBJC_EHTYPE_VTABLE as *const _ as usize),
        "_OBJC_EHTYPE_id" => Some(&exceptions::OBJC_EHTYPE_ID as *const _ as usize),
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
        "_objc_allocWithZone" => f(reflection::objc_alloc_with_zone as *const ()),
        "_objc_opt_new" => f(objc_opt_new as *const ()),
        "_objc_opt_class" => f(objc_opt_class as *const ()),
        "_objc_opt_self" => f(objc_opt_self as *const ()),
        "_objc_opt_isKindOfClass" => f(objc_opt_is_kind_of_class as *const ()),
        "_objc_opt_respondsToSelector" => f(objc_opt_responds_to_selector as *const ()),
        "__objc_rootAutorelease" => f(reflection::objc_root_autorelease as *const ()),
        "_sel_registerName" | "_sel_getUid" => f(sel_register_name as *const ()),
        "_sel_getName" => f(sel_get_name as *const ()),
        "_sel_isEqual" => f(reflection::sel_is_equal as *const ()),
        "_objc_getClass" | "_objc_lookUpClass" => f(objc_get_class as *const ()),
        "_objc_getMetaClass" => f(reflection::objc_get_meta_class as *const ()),
        "_objc_getRequiredClass" => f(reflection::objc_get_required_class as *const ()),
        "_objc_copyClassList" => f(reflection::objc_copy_class_list as *const ()),
        "_objc_getClassList" => f(reflection::objc_get_class_list as *const ()),
        "_objc_copyImageNames" => f(reflection::objc_copy_image_names as *const ()),
        "_objc_copyClassNamesForImage" => f(reflection::objc_copy_class_names_for_image as *const ()),
        "_objc_allocateClassPair" => f(reflection::objc_allocate_class_pair as *const ()),
        "_objc_initializeClassPair" => f(reflection::objc_initialize_class_pair as *const ()),
        "_objc_registerClassPair" => f(reflection::objc_register_class_pair as *const ()),
        "_objc_readClassPair" => f(objc_read_class_pair as *const ()),
        "_objc_setHook_getClass" => f(objc_set_hook_get_class as *const ()),
        "_object_getClass" => f(object_get_class as *const ()),
        "_object_setClass" => f(reflection::object_set_class as *const ()),
        "_object_getClassName" => f(reflection::object_get_class_name as *const ()),
        "_object_isClass" => f(reflection::object_is_class as *const ()),
        "_object_getIvar" => f(reflection::object_get_ivar as *const ()),
        "_object_setIvar" => f(reflection::object_set_ivar as *const ()),
        "_object_getInstanceVariable" => f(reflection::object_get_instance_variable as *const ()),
        "_object_getIndexedIvars" => f(reflection::object_get_indexed_ivars as *const ()),
        "_object_dispose" => f(reflection::object_dispose_fn as *const ()),
        "_objc_constructInstance" => f(reflection::objc_construct_instance as *const ()),
        "_objc_destructInstance" => f(reflection::objc_destruct_instance as *const ()),
        "_class_getName" => f(reflection::class_get_name as *const ()),
        "_class_getSuperclass" => f(class_get_superclass as *const ()),
        "_class_setSuperclass" => f(reflection::class_set_superclass as *const ()),
        "_class_isMetaClass" => f(reflection::class_is_meta_class as *const ()),
        "_class_getInstanceSize" => f(reflection::class_get_instance_size as *const ()),
        "_class_getIvarLayout" => f(reflection::class_get_ivar_layout as *const ()),
        "_class_getImageName" => f(reflection::class_get_image_name as *const ()),
        "_class_respondsToSelector" => f(class_respond_to_selector as *const ()),
        "_class_addMethod" => f(class_add_method as *const ()),
        "_class_replaceMethod" => f(reflection::class_replace_method as *const ()),
        "_class_getInstanceMethod" => f(reflection::class_get_instance_method as *const ()),
        "_class_getClassMethod" => f(reflection::class_get_class_method as *const ()),
        "_class_copyMethodList" => f(reflection::class_copy_method_list as *const ()),
        "_class_getMethodImplementation" => f(reflection::class_get_method_implementation as *const ()),
        "_class_getInstanceVariable" => f(reflection::class_get_instance_variable as *const ()),
        "_class_copyIvarList" => f(reflection::class_copy_ivar_list as *const ()),
        "_class_addIvar" => f(reflection::class_add_ivar as *const ()),
        "_class_conformsToProtocol" => f(reflection::class_conforms_to_protocol as *const ()),
        "_class_addProtocol" => f(reflection::class_add_protocol as *const ()),
        "_class_copyProtocolList" => f(reflection::class_copy_protocol_list as *const ()),
        "_class_copyPropertyList" => f(reflection::class_copy_property_list as *const ()),
        "_class_getProperty" => f(reflection::class_get_property as *const ()),
        "_class_addProperty" => f(reflection::class_add_property as *const ()),
        "_class_createInstance" => f(reflection::class_create_instance as *const ()),
        "_method_getName" => f(reflection::method_get_name as *const ()),
        "_method_getImplementation" => f(reflection::method_get_implementation as *const ()),
        "_method_setImplementation" => f(reflection::method_set_implementation as *const ()),
        "_method_getTypeEncoding" => f(reflection::method_get_type_encoding as *const ()),
        "_method_getNumberOfArguments" => f(reflection::method_get_number_of_arguments as *const ()),
        "_method_copyReturnType" => f(reflection::method_copy_return_type as *const ()),
        "_method_copyArgumentType" => f(reflection::method_copy_argument_type as *const ()),
        "_method_getReturnType" => f(reflection::method_get_return_type as *const ()),
        "_method_getArgumentType" => f(reflection::method_get_argument_type as *const ()),
        "_method_getDescription" => f(reflection::method_get_description as *const ()),
        "_method_exchangeImplementations" => f(reflection::method_exchange_implementations as *const ()),
        "_ivar_getName" => f(reflection::ivar_get_name as *const ()),
        "_ivar_getOffset" => f(reflection::ivar_get_offset as *const ()),
        "_ivar_getTypeEncoding" => f(reflection::ivar_get_type_encoding as *const ()),
        "_objc_getProtocol" => f(reflection::objc_get_protocol as *const ()),
        "_protocol_getName" => f(reflection::protocol_get_name as *const ()),
        "_protocol_isEqual" => f(reflection::protocol_is_equal as *const ()),
        "_protocol_copyProtocolList" => f(reflection::protocol_copy_protocol_list as *const ()),
        "_protocol_getMethodDescription" => f(reflection::protocol_get_method_description as *const ()),
        "_protocol_copyMethodDescriptionList" => f(reflection::protocol_copy_method_description_list as *const ()),
        "_protocol_copyPropertyList" => f(reflection::protocol_copy_property_list as *const ()),
        "_protocol_getProperty" => f(reflection::protocol_get_property as *const ()),
        "_protocol_conformsToProtocol" => f(reflection::protocol_conforms_to_protocol as *const ()),
        "_property_getName" => f(reflection::property_get_name as *const ()),
        "_property_getAttributes" => f(reflection::property_get_attributes as *const ()),
        "_property_copyAttributeList" => f(reflection::property_copy_attribute_list as *const ()),
        "_property_copyAttributeValue" => f(reflection::property_copy_attribute_value as *const ()),
        // Properties / sync / associated objects
        "_objc_getProperty" => f(props::objc_get_property as *const ()),
        "_objc_setProperty" => f(props::objc_set_property as *const ()),
        "_objc_setProperty_atomic" => f(props::objc_set_property_atomic as *const ()),
        "_objc_setProperty_nonatomic" => f(props::objc_set_property_nonatomic as *const ()),
        "_objc_setProperty_atomic_copy" => f(props::objc_set_property_atomic_copy as *const ()),
        "_objc_setProperty_nonatomic_copy" => f(props::objc_set_property_nonatomic_copy as *const ()),
        "_objc_copyStruct" => f(props::objc_copy_struct as *const ()),
        "_objc_copyCppObjectAtomic" => f(props::objc_copy_cpp_object_atomic as *const ()),
        "_objc_sync_enter" => f(props::objc_sync_enter as *const ()),
        "_objc_sync_exit" => f(props::objc_sync_exit as *const ()),
        "_objc_enumerationMutation" => f(props::objc_enumeration_mutation as *const ()),
        "_objc_setAssociatedObject" => f(props::objc_set_associated_object as *const ()),
        "_objc_getAssociatedObject" => f(props::objc_get_associated_object as *const ()),
        "_objc_removeAssociatedObjects" => f(props::objc_remove_associated_objects as *const ()),
        // Blocks (libclosure + libobjc block helpers)
        "__Block_copy" => f(blocks::block_copy as *const ()),
        "__Block_release" => f(blocks::block_release as *const ()),
        "__Block_object_assign" => f(blocks::block_object_assign as *const ()),
        "__Block_object_dispose" => f(blocks::block_object_dispose as *const ()),
        "__NSConcreteStackBlock" => Some(blocks::NSSTACKBLOCK_CLASS.ptr() as usize),
        "__NSConcreteMallocBlock" => Some(blocks::NSMALLOCBLOCK_CLASS.ptr() as usize),
        "__NSConcreteGlobalBlock" => Some(blocks::NSGLOBALBLOCK_CLASS.ptr() as usize),
        "_objc_retainBlock" => f(blocks::objc_retain_block as *const ()),
        "_imp_implementationWithBlock" => f(blocks::imp_implementation_with_block as *const ()),
        "_imp_getBlock" => f(blocks::imp_get_block as *const ()),
        "_imp_removeBlock" => f(blocks::imp_remove_block as *const ()),
        // Exceptions
        "___objc_personality_v0" => f(exceptions::objc_personality_v0 as *const ()),
        "_objc_exception_throw" => f(exceptions::objc_exception_throw as *const ()),
        "_objc_exception_rethrow" => f(exceptions::objc_exception_rethrow as *const ()),
        "_objc_begin_catch" => f(exceptions::objc_begin_catch as *const ()),
        "_objc_end_catch" => f(exceptions::objc_end_catch as *const ()),
        "_objc_terminate" => f(exceptions::objc_terminate as *const ()),
        "_objc_setExceptionPreprocessor" => f(exceptions::objc_set_exception_preprocessor as *const ()),
        _ => message::register_variant(name),
    }
}
