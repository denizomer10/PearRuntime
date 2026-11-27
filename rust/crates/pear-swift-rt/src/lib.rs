//! Clean-room Swift 5 ABI runtime & ObjC interop layer for PearRuntime.
//! Zero crates.io dependencies; registers `_TtCs12_SwiftObject` onto `pear-libobjc`.

pub mod metadata;
pub mod overlays;
pub mod runtime;

use pear_libobjc::abi::ClassT;

/// Initializes Swift runtime support (`_TtCs12_SwiftObject` root class).
pub fn init() {
    runtime::register_swift_object_class();
    runtime::touch_runtime_init();
}

pub fn swift_object_class() -> *const ClassT {
    runtime::swift_object_class()
}

pub fn swift_object_metaclass() -> *const ClassT {
    runtime::swift_object_metaclass()
}

/// Resolves Swift runtime (`_swift_*`, `__swift*`), `_TtCs12_SwiftObject`,
/// value witness tables, or stdlib/overlay mangled (`_$s*`) symbols.
pub fn symbol(library: Option<&str>, name: &str) -> Option<usize> {
    init();
    if let Some(addr) = runtime::runtime_symbol(name) {
        return Some(addr);
    }
    if let Some(addr) = metadata::metadata_symbol(name) {
        return Some(addr);
    }
    if let Some(lib) = library {
        if overlays::is_swift_library_path(lib) {
            if let Some(addr) = overlays::resolve_mangled_symbol(Some(lib), name) {
                return Some(addr);
            }
        }
    } else if let Some(addr) = overlays::resolve_mangled_symbol(None, name) {
        return Some(addr);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pear_libobjc::abi::{ClassRo, ClassT, Id, Imp, Sel};
    use pear_libobjc::object::EMPTY_CACHE;
    use pear_libobjc::runtime as objc_rt;
    use std::ffi::{c_void, CStr};
    use std::sync::atomic::{AtomicUsize, Ordering};

    unsafe fn send0(obj: Id, sel_name: &str) -> Id {
        let s = objc_rt::sel(sel_name);
        let d = pear_libobjc::message::lookup_for_send(obj, s);
        let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s)
    }

    unsafe fn send1_ptr(obj: Id, sel_name: &str, arg: *const c_void) -> bool {
        let s = objc_rt::sel(sel_name);
        let d = pear_libobjc::message::lookup_for_send(obj, s);
        let f: extern "C" fn(Id, Sel, *const c_void) -> bool = std::mem::transmute(d.imp);
        f(d.receiver, s, arg)
    }

    #[test]
    fn swift_object_root_class_and_subclass_realize() {
        init();
        let cls = swift_object_class();
        let meta = swift_object_metaclass();
        assert!(!cls.is_null() && !meta.is_null());
        assert_eq!(
            symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_OBJC_CLASS_$__TtCs12_SwiftObject"),
            Some(cls as usize)
        );
        assert_eq!(
            symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_OBJC_METACLASS_$__TtCs12_SwiftObject"),
            Some(meta as usize)
        );

        unsafe {
            let inst = send0(cls as Id, "new");
            assert!(!inst.is_null());
            assert!(send1_ptr(inst, "isKindOfClass:", cls as *const c_void));
            assert_eq!(send0(inst, "class"), cls as Id);

            // Realize a synthetic ObjC subclass whose superclass is _TtCs12_SwiftObject
            let sub_name = c"_TtC4Test10MySwiftBox";
            let mut sub_ro = ClassRo {
                flags: 0,
                instance_start: 16,
                instance_size: 24,
                reserved: 0,
                ivar_layout: std::ptr::null(),
                name: sub_name.as_ptr(),
                base_methods: std::ptr::null(),
                base_protocols: std::ptr::null(),
                ivars: std::ptr::null(),
                weak_ivar_layout: std::ptr::null(),
                base_properties: std::ptr::null(),
            };
            let mut sub_meta_ro = ClassRo {
                flags: 1,
                instance_start: 40,
                instance_size: 40,
                reserved: 0,
                ivar_layout: std::ptr::null(),
                name: sub_name.as_ptr(),
                base_methods: std::ptr::null(),
                base_protocols: std::ptr::null(),
                ivars: std::ptr::null(),
                weak_ivar_layout: std::ptr::null(),
                base_properties: std::ptr::null(),
            };
            let sub_meta = Box::leak(Box::new(ClassT {
                isa: meta,
                superclass: meta,
                cache: &raw const EMPTY_CACHE as *const c_void,
                vtable: std::ptr::null(),
                bits: (&raw mut sub_meta_ro as usize) | pear_libobjc::abi::FAST_IS_SWIFT_STABLE,
            }));
            let sub_cls = Box::leak(Box::new(ClassT {
                isa: sub_meta,
                superclass: cls,
                cache: &raw const EMPTY_CACHE as *const c_void,
                vtable: std::ptr::null(),
                bits: (&raw mut sub_ro as usize) | pear_libobjc::abi::FAST_IS_SWIFT_STABLE,
            }));
            assert!(objc_rt::realize(sub_cls));
            assert!(objc_rt::is_swift_class(sub_cls));
            assert_eq!(
                CStr::from_ptr((*objc_rt::ro(sub_cls)).name).to_str().unwrap(),
                "_TtC4Test10MySwiftBox"
            );
            let sub_inst = send0(sub_cls as *const ClassT as Id, "alloc");
            assert!(send1_ptr(sub_inst, "isKindOfClass:", cls as *const c_void));
            let s_rel = objc_rt::sel("release");
            let d_rel = pear_libobjc::message::lookup_for_send(sub_inst, s_rel);
            let f_rel: extern "C" fn(Id, Sel) = std::mem::transmute(d_rel.imp);
            f_rel(d_rel.receiver, s_rel);
            let d_rel2 = pear_libobjc::message::lookup_for_send(inst, s_rel);
            let f_rel2: extern "C" fn(Id, Sel) = std::mem::transmute(d_rel2.imp);
            f_rel2(d_rel2.receiver, s_rel);
        }
    }

    #[test]
    fn swift_arc_weak_unowned_box_and_once_work() {
        init();
        let obj = runtime::swift_alloc_object(swift_object_class(), 32, 7);
        assert!(!obj.is_null());
        assert!(runtime::swift_is_uniquely_referenced_native(obj));
        assert!(runtime::swift_is_uniquely_referenced_bridge(obj));

        runtime::swift_retain_n(obj, 2);
        assert!(!runtime::swift_is_uniquely_referenced_native(obj));

        let mut weak_slot: Id = std::ptr::null_mut();
        runtime::swift_weak_init(&mut weak_slot, obj);
        let loaded = runtime::swift_weak_load_strong(&mut weak_slot);
        assert_eq!(loaded, obj);
        runtime::swift_release(loaded);

        let mut unowned_slot: Id = std::ptr::null_mut();
        runtime::swift_unknown_object_unowned_init(&mut unowned_slot, obj);
        let u_strong = runtime::swift_unknown_object_unowned_load_strong(&mut unowned_slot);
        assert_eq!(u_strong, obj);
        runtime::swift_release(u_strong);
        runtime::swift_unknown_object_unowned_destroy(&mut unowned_slot);

        runtime::swift_release_n(obj, 2);
        assert!(runtime::swift_is_uniquely_referenced_native(obj));
        runtime::swift_release(obj);
        // Weak reference must now return null cleanly after dealloc
        assert!(runtime::swift_weak_load_strong(&mut weak_slot).is_null());
        runtime::swift_weak_destroy(&mut weak_slot);

        // Box allocation and projection
        let bp = runtime::swift_alloc_box(metadata::anyobject_metadata());
        assert!(!bp.heap_object.is_null() && !bp.buffer.is_null());
        assert_eq!(runtime::swift_project_box(bp.heap_object), bp.buffer);
        runtime::swift_release(bp.heap_object);

        // swift_once
        static HITS: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn bump(_ctx: *mut c_void) {
            HITS.fetch_add(1, Ordering::SeqCst);
        }
        let mut pred: usize = 0;
        runtime::swift_once(&mut pred, bump, std::ptr::null_mut());
        runtime::swift_once(&mut pred, bump, std::ptr::null_mut());
        assert_eq!(pred, usize::MAX);
        assert_eq!(HITS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn metadata_vwt_casting_and_arrays_work() {
        init();
        let mut a: u64 = 42;
        let mut b: u64 = 0;
        let i64_ty = overlays::lookup_builtin_type_by_name("Si").unwrap();
        let src = [&raw mut a as *mut c_void; 1];
        let _ = src;
        metadata::swift_array_init_with_copy(
            &raw mut b as *mut c_void,
            &raw mut a as *mut c_void,
            1,
            i64_ty,
        );
        assert_eq!(b, 42);

        let wrapped = metadata::swift_get_objc_class_metadata(swift_object_class());
        assert_eq!(metadata::swift_get_objc_class_from_metadata(wrapped), swift_object_class());

        let meta_of_int = metadata::swift_get_metatype_metadata(i64_ty);
        assert!(!meta_of_int.is_null());
        let fn0 = metadata::swift_get_function_type_metadata0(0, i64_ty);
        assert!(!fn0.is_null());
        let tup = metadata::swift_get_tuple_type_metadata2(0, i64_ty, i64_ty, std::ptr::null(), std::ptr::null());
        assert_eq!(tup.state, 0);
        assert!(!tup.value.is_null());

        let inst = runtime::swift_alloc_object(swift_object_class(), 16, 7);
        assert_eq!(metadata::swift_dynamic_cast_class(inst, swift_object_class()), inst);
        runtime::swift_release(inst);
    }

    #[test]
    fn resolves_all_genshin_non_mangled_swift_symbols_and_overlays() {
        init();
        const ALL_116: &[&str] = &[
            "__swiftEmptyArrayStorage",
            "__swiftEmptyDictionarySingleton",
            "__swiftEmptySetSingleton",
            "__swift_stdlib_operatingSystemVersion",
            "__swift_stdlib_reportUnimplementedInitializer",
            "__swift_stdlib_strtod_clocale",
            "__swift_stdlib_strtof_clocale",
            "_swift_allocBox",
            "_swift_allocError",
            "_swift_allocObject",
            "_swift_allocateGenericClassMetadata",
            "_swift_allocateGenericValueMetadata",
            "_swift_arrayAssignWithTake",
            "_swift_arrayDestroy",
            "_swift_arrayInitWithCopy",
            "_swift_arrayInitWithTakeBackToFront",
            "_swift_arrayInitWithTakeFrontToBack",
            "_swift_beginAccess",
            "_swift_bridgeObjectRelease",
            "_swift_bridgeObjectRelease_n",
            "_swift_bridgeObjectRetain",
            "_swift_bridgeObjectRetain_n",
            "_swift_checkMetadataState",
            "_swift_conformsToProtocol",
            "_swift_deallocClassInstance",
            "_swift_deallocObject",
            "_swift_deallocPartialClassInstance",
            "_swift_deallocUninitializedObject",
            "_swift_deletedMethodError",
            "_swift_dynamicCast",
            "_swift_dynamicCastClass",
            "_swift_dynamicCastMetatype",
            "_swift_dynamicCastObjCClass",
            "_swift_dynamicCastObjCProtocolConditional",
            "_swift_dynamicCastTypeToObjCProtocolConditional",
            "_swift_dynamicCastUnknownClassUnconditional",
            "_swift_endAccess",
            "_swift_errorRelease",
            "_swift_errorRetain",
            "_swift_getAssociatedConformanceWitness",
            "_swift_getAssociatedTypeWitness",
            "_swift_getAtKeyPath",
            "_swift_getDynamicType",
            "_swift_getEnumCaseMultiPayload",
            "_swift_getEnumTagSinglePayloadGeneric",
            "_swift_getErrorValue",
            "_swift_getExistentialTypeMetadata",
            "_swift_getForeignTypeMetadata",
            "_swift_getFunctionTypeMetadata0",
            "_swift_getFunctionTypeMetadata1",
            "_swift_getGenericMetadata",
            "_swift_getInitializedObjCClass",
            "_swift_getKeyPath",
            "_swift_getMetatypeMetadata",
            "_swift_getObjCClassFromMetadata",
            "_swift_getObjCClassMetadata",
            "_swift_getObjectType",
            "_swift_getSingletonMetadata",
            "_swift_getTupleTypeLayout2",
            "_swift_getTupleTypeMetadata",
            "_swift_getTupleTypeMetadata2",
            "_swift_getTupleTypeMetadata3",
            "_swift_getTypeByMangledNameInContext",
            "_swift_getWitnessTable",
            "_swift_initClassMetadata2",
            "_swift_initEnumMetadataMultiPayload",
            "_swift_initEnumMetadataSinglePayload",
            "_swift_initStackObject",
            "_swift_initStaticObject",
            "_swift_initStructMetadata",
            "_swift_isClassType",
            "_swift_isEscapingClosureAtFileLocation",
            "_swift_isUniquelyReferencedNonObjC_nonNull_bridgeObject",
            "_swift_isUniquelyReferenced_nonNull_native",
            "_swift_isaMask",
            "_swift_lookUpClassMethod",
            "_swift_makeBoxUnique",
            "_swift_once",
            "_swift_projectBox",
            "_swift_readAtKeyPath",
            "_swift_release",
            "_swift_release_n",
            "_swift_retain",
            "_swift_retain_n",
            "_swift_setAtReferenceWritableKeyPath",
            "_swift_setDeallocating",
            "_swift_slowAlloc",
            "_swift_slowDealloc",
            "_swift_stdlib_random",
            "_swift_storeEnumTagMultiPayload",
            "_swift_storeEnumTagSinglePayloadGeneric",
            "_swift_unexpectedError",
            "_swift_unknownObjectRelease",
            "_swift_unknownObjectRelease_n",
            "_swift_unknownObjectRetain",
            "_swift_unknownObjectRetain_n",
            "_swift_unknownObjectUnownedDestroy",
            "_swift_unknownObjectUnownedInit",
            "_swift_unknownObjectUnownedLoadStrong",
            "_swift_unknownObjectWeakAssign",
            "_swift_unknownObjectWeakCopyAssign",
            "_swift_unknownObjectWeakCopyInit",
            "_swift_unknownObjectWeakDestroy",
            "_swift_unknownObjectWeakInit",
            "_swift_unknownObjectWeakLoadStrong",
            "_swift_unknownObjectWeakTakeAssign",
            "_swift_unknownObjectWeakTakeInit",
            "_swift_unownedRelease",
            "_swift_unownedRetain",
            "_swift_unownedRetainStrong",
            "_swift_updateClassMetadata2",
            "_swift_weakAssign",
            "_swift_weakDestroy",
            "_swift_weakInit",
            "_swift_weakLoadStrong",
            "_swift_willThrow",
        ];
        assert_eq!(ALL_116.len(), 116);
        for &sym in ALL_116 {
            let res = symbol(Some("/usr/lib/swift/libswiftCore.dylib"), sym);
            assert!(res.is_some() && res.unwrap() != 0, "missing core symbol: {sym}");
        }

        for (lib, sym) in [
            ("/usr/lib/swift/libswiftCore.dylib", "_$sBOWV"),
            ("/usr/lib/swift/libswiftCore.dylib", "_$sSSN"),
            ("/usr/lib/swift/libswiftCore.dylib", "_$sSD4KeysVMn"),
            ("/usr/lib/swift/libswiftFoundation.dylib", "_$s10Foundation10URLRequestVMa"),
            ("/usr/lib/swift/libswiftDispatch.dylib", "_$s8Dispatch0A4TimeV3nowACyFZ"),
            ("/usr/lib/swift/libswiftObjectiveC.dylib", "_$s10ObjectiveC8SelectorVyACSScfC"),
            ("/usr/lib/swift/libswiftCoreGraphics.dylib", "_$s12CoreGraphics7CGFloatVN"),
            ("/usr/lib/swift/libswiftCoreFoundation.dylib", "_$s15CoreFoundation9_CFObjectMp"),
            ("/usr/lib/swift/libswiftDarwin.dylib", "_$s6Darwin5errnos5Int32Vvg"),
            ("/usr/lib/swift/libswiftos.dylib", "_$s2os9OSLogTypeV5debugACvgZ"),
            ("/System/Library/Frameworks/SwiftUI.framework/SwiftUI", "_$s7SwiftUI4TextVMn"),
        ] {
            let res = symbol(Some(lib), sym);
            assert!(res.is_some() && res.unwrap() != 0, "missing overlay symbol: {lib} :: {sym}");
        }
        let _imp_check: Imp = std::ptr::null();
        let _ = _imp_check;
    }

    #[test]
    fn phase5_20_swift_untracked_retain_release_no_recursion() {
        runtime::register_swift_object_class();
        unsafe {
            // 1. Immortal static Swift singleton not in SWIFT_OBJECTS
            let empty_arr = &raw const runtime::EMPTY_ARRAY_STORAGE as Id;
            assert_eq!(pear_libobjc::object::retain(empty_arr), empty_arr);
            pear_libobjc::object::release(empty_arr);
            assert_eq!(runtime::swift_retain(empty_arr), empty_arr);
            runtime::swift_release(empty_arr);

            // 2. Stack / static initialized Swift object
            let mut static_buf = [0usize; 4];
            let s_obj = runtime::swift_init_static_object(
                runtime::swift_object_class(),
                static_buf.as_mut_ptr() as Id,
            );
            assert_eq!(pear_libobjc::object::retain(s_obj), s_obj);
            pear_libobjc::object::release(s_obj);
            assert_eq!(runtime::swift_retain(s_obj), s_obj);
            runtime::swift_release(s_obj);

            // 3. ObjC-allocated _TtCs12_SwiftObject instance
            let objc_alloc = pear_libobjc::object::alloc(runtime::swift_object_class());
            assert!(!objc_alloc.is_null());
            assert_eq!(pear_libobjc::object::retain(objc_alloc), objc_alloc);
            assert_eq!(runtime::swift_retain(objc_alloc), objc_alloc);
            runtime::swift_release(objc_alloc);
            pear_libobjc::object::release(objc_alloc);
        }
    }
}
