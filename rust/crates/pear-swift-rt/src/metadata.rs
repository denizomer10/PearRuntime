//! Clean-room Swift 5 ABI metadata, ValueWitnessTable, protocol conformance,
//! dynamic casting, array value operations, enum payloads, and KeyPaths.

use crate::runtime::{
    self, swift_bridge_object_release, swift_bridge_object_retain, swift_release, swift_retain,
    SWIFT_OBJECT_CLASS,
};
use pear_libobjc::abi::{ClassT, Id};
use pear_libobjc::runtime as objc_rt;
use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::sync::{LazyLock, Mutex};

// Swift 5 MetadataKind constants
pub const KIND_CLASS: usize = 0;
pub const KIND_STRUCT: usize = 0x200;
pub const KIND_ENUM: usize = 0x201;
pub const KIND_OPTIONAL: usize = 0x202;
pub const KIND_FOREIGN_CLASS: usize = 0x203;
pub const KIND_OPAQUE: usize = 0x300;
pub const KIND_TUPLE: usize = 0x301;
pub const KIND_FUNCTION: usize = 0x302;
pub const KIND_EXISTENTIAL: usize = 0x303;
pub const KIND_METATYPE: usize = 0x304;
pub const KIND_OBJC_CLASS_WRAPPER: usize = 0x305;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ValueWitnessTable {
    pub initialize_buffer_with_copy_of_buffer: extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> *mut c_void,
    pub destroy: extern "C" fn(*mut c_void, *const c_void),
    pub initialize_with_copy: extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> *mut c_void,
    pub assign_with_copy: extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> *mut c_void,
    pub initialize_with_take: extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> *mut c_void,
    pub assign_with_take: extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> *mut c_void,
    pub get_enum_tag_single_payload: extern "C" fn(*const c_void, u32, *const c_void) -> u32,
    pub store_enum_tag_single_payload: extern "C" fn(*mut c_void, u32, u32, *const c_void),
    pub size: usize,
    pub stride: usize,
    pub flags: u32,
    pub extra_inhabitant_count: u32,
}
unsafe impl Sync for ValueWitnessTable {}

#[repr(C)]
pub struct FullMetadata {
    pub vwt: *const ValueWitnessTable,
    pub kind: usize,
    pub desc_or_arg0: usize,
    pub arg1: usize,
    pub arg2: usize,
    pub arg3: usize,
}
unsafe impl Sync for FullMetadata {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MetadataResponse {
    pub value: *const c_void,
    pub state: usize,
}

#[repr(C)]
pub struct TupleElement {
    pub ty: *const c_void,
    pub offset: usize,
}

#[repr(C)]
pub struct TupleTypeMetadata {
    pub vwt: *const ValueWitnessTable,
    pub kind: usize,
    pub num_elements: usize,
    pub labels: *const c_char,
    pub elements: [TupleElement; 4],
}

#[repr(C)]
pub struct FunctionTypeMetadata {
    pub vwt: *const ValueWitnessTable,
    pub kind: usize,
    pub flags: usize,
    pub result: *const c_void,
    pub params: [usize; 4],
}

#[repr(C)]
pub struct MetatypeMetadata {
    pub vwt: *const ValueWitnessTable,
    pub kind: usize,
    pub instance_type: *const c_void,
}

#[repr(C)]
pub struct ExistentialTypeMetadata {
    pub vwt: *const ValueWitnessTable,
    pub kind: usize,
    pub flags: usize,
    pub num_protocols: usize,
    pub protocols: [usize; 4],
}

#[repr(C)]
pub struct ObjCClassWrapperMetadata {
    pub vwt: *const ValueWitnessTable,
    pub kind: usize,
    pub cls: *const ClassT,
}

#[repr(C)]
pub struct GenericDescriptorStub {
    pub flags: u32,
    pub parent: i32,
    pub name_rel: i32,
    pub reserved: [u32; 5],
    pub label: [u8; 64],
}
unsafe impl Sync for GenericDescriptorStub {}

#[repr(C)]
pub struct ProtocolWitnessTableStub {
    pub conformance: *const c_void,
    pub methods: [usize; 31],
}
unsafe impl Sync for ProtocolWitnessTableStub {}

// ---- ValueWitnessTable function implementations -----------------------------

pub fn metadata_vwt(ty: *const c_void) -> *const ValueWitnessTable {
    if (ty as usize) < 0x10000 {
        return &raw const VWT_BI64;
    }
    let vwt = unsafe { *((ty as *const *const ValueWitnessTable).sub(1)) };
    if (vwt as usize) < 0x10000 {
        &raw const VWT_BI64
    } else {
        vwt
    }
}

pub fn type_size(ty: *const c_void) -> usize {
    if (ty as usize) < 0x10000 {
        return 8;
    }
    let vwt = metadata_vwt(ty);
    unsafe { (*vwt).size.max(8) }
}

extern "C" fn pod_init_buf_copy(dst_buf: *mut c_void, src_buf: *mut c_void, ty: *const c_void) -> *mut c_void {
    pod_init_copy(dst_buf, src_buf, ty)
}
extern "C" fn pod_destroy(_obj: *mut c_void, _ty: *const c_void) {}
extern "C" fn pod_init_copy(dst: *mut c_void, src: *mut c_void, ty: *const c_void) -> *mut c_void {
    let sz = unsafe { (*metadata_vwt(ty)).size };
    if !dst.is_null() && !src.is_null() && sz > 0 {
        unsafe { std::ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, sz) };
    }
    dst
}
extern "C" fn pod_assign_copy(dst: *mut c_void, src: *mut c_void, ty: *const c_void) -> *mut c_void {
    let sz = unsafe { (*metadata_vwt(ty)).size };
    if !dst.is_null() && !src.is_null() && dst != src && sz > 0 {
        unsafe { std::ptr::copy(src as *const u8, dst as *mut u8, sz) };
    }
    dst
}
extern "C" fn pod_get_enum_tag_sp(val: *const c_void, empty_cases: u32, ty: *const c_void) -> u32 {
    if val.is_null() || empty_cases == 0 {
        return 0;
    }
    let sz = unsafe { (*metadata_vwt(ty)).size };
    if sz >= 8 {
        let ptr_val = unsafe { *(val as *const usize) };
        if ptr_val == 0 {
            return 1;
        }
    }
    0
}
extern "C" fn pod_store_enum_tag_sp(val: *mut c_void, which_case: u32, empty_cases: u32, ty: *const c_void) {
    if val.is_null() || empty_cases == 0 || which_case == 0 {
        return;
    }
    let sz = unsafe { (*metadata_vwt(ty)).size };
    if sz > 0 {
        unsafe { std::ptr::write_bytes(val as *mut u8, 0, sz) };
    }
}

extern "C" fn rc_init_buf_copy(dst: *mut c_void, src: *mut c_void, ty: *const c_void) -> *mut c_void {
    rc_init_copy(dst, src, ty)
}
extern "C" fn rc_destroy(obj: *mut c_void, _ty: *const c_void) {
    if !obj.is_null() {
        let p = unsafe { *(obj as *mut Id) };
        swift_release(p);
    }
}
extern "C" fn rc_init_copy(dst: *mut c_void, src: *mut c_void, _ty: *const c_void) -> *mut c_void {
    if !dst.is_null() && !src.is_null() {
        let p = unsafe { *(src as *const Id) };
        swift_retain(p);
        unsafe { *(dst as *mut Id) = p };
    }
    dst
}
extern "C" fn rc_assign_copy(dst: *mut c_void, src: *mut c_void, _ty: *const c_void) -> *mut c_void {
    if !dst.is_null() && !src.is_null() {
        let old = unsafe { *(dst as *const Id) };
        let new = unsafe { *(src as *const Id) };
        swift_retain(new);
        unsafe { *(dst as *mut Id) = new };
        swift_release(old);
    }
    dst
}
extern "C" fn rc_init_take(dst: *mut c_void, src: *mut c_void, _ty: *const c_void) -> *mut c_void {
    if !dst.is_null() && !src.is_null() {
        unsafe { *(dst as *mut Id) = *(src as *const Id) };
    }
    dst
}
extern "C" fn rc_assign_take(dst: *mut c_void, src: *mut c_void, _ty: *const c_void) -> *mut c_void {
    if !dst.is_null() && !src.is_null() && dst != src {
        let old = unsafe { *(dst as *const Id) };
        unsafe { *(dst as *mut Id) = *(src as *const Id) };
        swift_release(old);
    }
    dst
}
extern "C" fn rc_get_enum_tag_sp(val: *const c_void, empty_cases: u32, _ty: *const c_void) -> u32 {
    if val.is_null() || empty_cases == 0 {
        return 0;
    }
    let p = unsafe { *(val as *const usize) };
    if p == 0 { 1 } else { 0 }
}
extern "C" fn rc_store_enum_tag_sp(val: *mut c_void, which: u32, empty_cases: u32, _ty: *const c_void) {
    if !val.is_null() && empty_cases > 0 && which > 0 {
        unsafe { *(val as *mut usize) = 0 };
    }
}

extern "C" fn bridge_destroy(obj: *mut c_void, _ty: *const c_void) {
    if !obj.is_null() {
        let p = unsafe { *(obj as *mut Id) };
        swift_bridge_object_release(p);
    }
}
extern "C" fn bridge_init_copy(dst: *mut c_void, src: *mut c_void, _ty: *const c_void) -> *mut c_void {
    if !dst.is_null() && !src.is_null() {
        let p = unsafe { *(src as *const Id) };
        swift_bridge_object_retain(p);
        unsafe { *(dst as *mut Id) = p };
    }
    dst
}
extern "C" fn bridge_assign_copy(dst: *mut c_void, src: *mut c_void, _ty: *const c_void) -> *mut c_void {
    if !dst.is_null() && !src.is_null() {
        let old = unsafe { *(dst as *const Id) };
        let new = unsafe { *(src as *const Id) };
        swift_bridge_object_retain(new);
        unsafe { *(dst as *mut Id) = new };
        swift_bridge_object_release(old);
    }
    dst
}
extern "C" fn bridge_assign_take(dst: *mut c_void, src: *mut c_void, _ty: *const c_void) -> *mut c_void {
    if !dst.is_null() && !src.is_null() && dst != src {
        let old = unsafe { *(dst as *const Id) };
        unsafe { *(dst as *mut Id) = *(src as *const Id) };
        swift_bridge_object_release(old);
    }
    dst
}

const fn make_pod_vwt(size: usize, align_mask: u32, extra_inhabitants: u32) -> ValueWitnessTable {
    ValueWitnessTable {
        initialize_buffer_with_copy_of_buffer: pod_init_buf_copy,
        destroy: pod_destroy,
        initialize_with_copy: pod_init_copy,
        assign_with_copy: pod_assign_copy,
        initialize_with_take: pod_init_copy,
        assign_with_take: pod_assign_copy,
        get_enum_tag_single_payload: pod_get_enum_tag_sp,
        store_enum_tag_single_payload: pod_store_enum_tag_sp,
        size,
        stride: size,
        flags: align_mask,
        extra_inhabitant_count: extra_inhabitants,
    }
}

pub static VWT_BO_NATIVE: ValueWitnessTable = ValueWitnessTable {
    initialize_buffer_with_copy_of_buffer: rc_init_buf_copy,
    destroy: rc_destroy,
    initialize_with_copy: rc_init_copy,
    assign_with_copy: rc_assign_copy,
    initialize_with_take: rc_init_take,
    assign_with_take: rc_assign_take,
    get_enum_tag_single_payload: rc_get_enum_tag_sp,
    store_enum_tag_single_payload: rc_store_enum_tag_sp,
    size: 8,
    stride: 8,
    flags: 7,
    extra_inhabitant_count: 1,
};

pub static VWT_BO_UNKNOWN: ValueWitnessTable = VWT_BO_NATIVE;

pub static VWT_BB_BRIDGE: ValueWitnessTable = ValueWitnessTable {
    initialize_buffer_with_copy_of_buffer: bridge_init_copy,
    destroy: bridge_destroy,
    initialize_with_copy: bridge_init_copy,
    assign_with_copy: bridge_assign_copy,
    initialize_with_take: rc_init_take,
    assign_with_take: bridge_assign_take,
    get_enum_tag_single_payload: rc_get_enum_tag_sp,
    store_enum_tag_single_payload: rc_store_enum_tag_sp,
    size: 8,
    stride: 8,
    flags: 7,
    extra_inhabitant_count: 1,
};

pub static VWT_BP_RAWPTR: ValueWitnessTable = make_pod_vwt(8, 7, 1);
pub static VWT_BI8: ValueWitnessTable = make_pod_vwt(1, 0, 0);
pub static VWT_BI16: ValueWitnessTable = make_pod_vwt(2, 1, 0);
pub static VWT_BI32: ValueWitnessTable = make_pod_vwt(4, 3, 0);
pub static VWT_BI64: ValueWitnessTable = make_pod_vwt(8, 7, 0);
pub static VWT_BI128: ValueWitnessTable = make_pod_vwt(16, 15, 0);
pub static VWT_YT_VOID: ValueWitnessTable = make_pod_vwt(0, 0, 0);

static ANYOBJECT_META: FullMetadata = FullMetadata {
    vwt: &raw const VWT_BO_UNKNOWN,
    kind: KIND_EXISTENTIAL,
    desc_or_arg0: 0,
    arg1: 0,
    arg2: 0,
    arg3: 0,
};

pub fn anyobject_metadata() -> *const c_void {
    unsafe { (&raw const ANYOBJECT_META as *const usize).add(1) as *const c_void }
}

pub fn make_static_metadata(kind: usize, vwt: &'static ValueWitnessTable, arg0: usize) -> *const c_void {
    let leaked = Box::leak(Box::new(FullMetadata {
        vwt,
        kind,
        desc_or_arg0: arg0,
        arg1: 0,
        arg2: 0,
        arg3: 0,
    }));
    (&raw const leaked.kind) as *const c_void
}

// ---- Class & Metadata runtime functions ------------------------------------

static OBJC_WRAPPERS: LazyLock<Mutex<HashMap<usize, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static METATYPE_CACHE: LazyLock<Mutex<HashMap<usize, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static FUNCTION_CACHE: LazyLock<Mutex<HashMap<(usize, usize, usize), usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static TUPLE_CACHE: LazyLock<Mutex<HashMap<(usize, usize, usize), usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static WITNESS_CACHE: LazyLock<Mutex<HashMap<(usize, usize), usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub extern "C" fn swift_get_objc_class_metadata(cls: *const ClassT) -> *const c_void {
    if cls.is_null() {
        return std::ptr::null();
    }
    let mut map = OBJC_WRAPPERS.lock().unwrap();
    if let Some(&existing) = map.get(&(cls as usize)) {
        return existing as *const c_void;
    }
    let boxed = Box::leak(Box::new(ObjCClassWrapperMetadata {
        vwt: &raw const VWT_BO_UNKNOWN,
        kind: KIND_OBJC_CLASS_WRAPPER,
        cls,
    }));
    let meta_ptr = (&raw const boxed.kind) as usize;
    map.insert(cls as usize, meta_ptr);
    meta_ptr as *const c_void
}

pub extern "C" fn swift_get_objc_class_from_metadata(meta: *const c_void) -> *const ClassT {
    if (meta as usize) < 0x10000 {
        return std::ptr::null();
    }
    let kind = unsafe { *(meta as *const usize) };
    if kind == KIND_OBJC_CLASS_WRAPPER {
        unsafe { *((meta as *const usize).add(1) as *const *const ClassT) }
    } else {
        meta as *const ClassT
    }
}

pub extern "C" fn swift_get_initialized_objc_class(cls: *const ClassT) -> *const ClassT {
    if cls.is_null() {
        return std::ptr::null();
    }
    unsafe {
        if objc_rt::class(cls).is_none() {
            objc_rt::realize(cls);
        }
        objc_rt::ensure_initialized(cls);
    }
    cls
}

pub extern "C" fn swift_get_object_type(obj: Id) -> *const c_void {
    if obj.is_null() {
        return std::ptr::null();
    }
    unsafe { objc_rt::object_class(obj) as *const c_void }
}

pub extern "C" fn swift_get_dynamic_type(val: *const c_void, static_ty: *const c_void, is_boxed: bool) -> *const c_void {
    if val.is_null() {
        return static_ty;
    }
    if is_boxed || unsafe { *(static_ty as *const usize) } == KIND_EXISTENTIAL {
        return static_ty;
    }
    let obj = unsafe { *(val as *const Id) };
    if obj.is_null() || runtime::is_tagged_or_null(obj as usize) {
        static_ty
    } else {
        unsafe { objc_rt::object_class(obj) as *const c_void }
    }
}

pub extern "C" fn swift_is_class_type(ty: *const c_void) -> bool {
    if (ty as usize) < 0x10000 {
        return false;
    }
    let kind = unsafe { *(ty as *const usize) };
    kind == KIND_CLASS || kind == KIND_OBJC_CLASS_WRAPPER || kind == KIND_FOREIGN_CLASS
}

pub extern "C" fn swift_lookup_class_method(cls: *const c_void, method: *const usize) -> *const c_void {
    if cls.is_null() || method.is_null() {
        return fallback_method_stub as *const c_void;
    }
    let offset = unsafe { *method };
    if offset > 0 && offset < 4096 {
        let slot = unsafe { (cls as *const usize).add(offset / std::mem::size_of::<usize>()) };
        let fn_ptr = unsafe { *slot };
        if fn_ptr > 0x10000 {
            return fn_ptr as *const c_void;
        }
    }
    fallback_method_stub as *const c_void
}

extern "C" fn fallback_method_stub() -> usize {
    0
}

pub extern "C" fn swift_init_class_metadata2(
    _self_meta: *mut c_void,
    _flags: usize,
    _num_fields: usize,
    _field_types: *const *const c_void,
    _field_offsets: *mut usize,
) {
}

pub extern "C" fn swift_update_class_metadata2(
    _self_meta: *mut c_void,
    _flags: usize,
    _num_fields: usize,
    _field_types: *const *const c_void,
    _field_offsets: *mut usize,
) {
}

pub extern "C" fn swift_allocate_generic_class_metadata(
    desc: *const GenericDescriptorStub,
    _arguments: *const *const c_void,
    _pattern: *const c_void,
) -> *const c_void {
    make_static_metadata(KIND_CLASS, &VWT_BO_NATIVE, desc as usize)
}

pub extern "C" fn swift_allocate_generic_value_metadata(
    desc: *const GenericDescriptorStub,
    _arguments: *const *const c_void,
    _pattern: *const c_void,
    extra_bytes: usize,
) -> *const c_void {
    let total = 64 + extra_bytes.min(4096);
    let buf = Box::leak(vec![0u8; total].into_boxed_slice());
    let ptr = buf.as_mut_ptr() as *mut usize;
    unsafe {
        *ptr = &raw const VWT_BI64 as usize;
        *ptr.add(1) = KIND_STRUCT;
        *ptr.add(2) = desc as usize;
        ptr.add(1) as *const c_void
    }
}

pub extern "C" fn swift_init_struct_metadata(
    meta: *mut c_void,
    _flags: usize,
    num_fields: usize,
    field_types: *const *const c_void,
    field_offsets: *mut u32,
) {
    if meta.is_null() {
        return;
    }
    let mut offset = 0u32;
    for i in 0..num_fields {
        let fty = if field_types.is_null() {
            std::ptr::null()
        } else {
            unsafe { *field_types.add(i) }
        };
        let sz = type_size(fty) as u32;
        if !field_offsets.is_null() {
            unsafe { *field_offsets.add(i) = offset };
        }
        offset = (offset + sz.max(1) + 7) & !7;
    }
}

pub extern "C" fn swift_init_enum_metadata_single_payload(
    _meta: *mut c_void,
    _flags: usize,
    _payload: *const c_void,
    _empty_cases: u32,
) {
}

pub extern "C" fn swift_init_enum_metadata_multi_payload(
    _meta: *mut c_void,
    _flags: usize,
    _num_payloads: usize,
    _payloads: *const *const c_void,
) {
}

pub extern "C" fn swift_get_generic_metadata(
    _req: usize,
    _args: *const *const c_void,
    desc: *const GenericDescriptorStub,
) -> MetadataResponse {
    let value = make_static_metadata(KIND_STRUCT, &VWT_BI64, desc as usize);
    MetadataResponse { value, state: 0 }
}

pub extern "C" fn swift_get_singleton_metadata(
    _req: usize,
    desc: *const GenericDescriptorStub,
) -> MetadataResponse {
    let value = make_static_metadata(KIND_STRUCT, &VWT_BI64, desc as usize);
    MetadataResponse { value, state: 0 }
}

pub extern "C" fn swift_get_foreign_type_metadata(
    _req: usize,
    candidate: *const c_void,
) -> MetadataResponse {
    let value = if candidate.is_null() {
        make_static_metadata(KIND_FOREIGN_CLASS, &VWT_BI64, 0)
    } else {
        candidate
    };
    MetadataResponse { value, state: 0 }
}

pub extern "C" fn swift_get_metatype_metadata(instance_type: *const c_void) -> *const c_void {
    let key = instance_type as usize;
    let mut map = METATYPE_CACHE.lock().unwrap();
    if let Some(&cached) = map.get(&key) {
        return cached as *const c_void;
    }
    let m = Box::leak(Box::new(MetatypeMetadata {
        vwt: &raw const VWT_BP_RAWPTR,
        kind: KIND_METATYPE,
        instance_type,
    }));
    let ptr = (&raw const m.kind) as usize;
    map.insert(key, ptr);
    ptr as *const c_void
}

pub extern "C" fn swift_get_existential_type_metadata(
    class_constraint: usize,
    _superclass: *const c_void,
    num_protocols: usize,
    protocols: *const usize,
) -> *const c_void {
    let m = Box::leak(Box::new(ExistentialTypeMetadata {
        vwt: &raw const VWT_BO_UNKNOWN,
        kind: KIND_EXISTENTIAL,
        flags: class_constraint,
        num_protocols,
        protocols: [
            if num_protocols > 0 && !protocols.is_null() { unsafe { *protocols } } else { 0 },
            if num_protocols > 1 && !protocols.is_null() { unsafe { *protocols.add(1) } } else { 0 },
            if num_protocols > 2 && !protocols.is_null() { unsafe { *protocols.add(2) } } else { 0 },
            if num_protocols > 3 && !protocols.is_null() { unsafe { *protocols.add(3) } } else { 0 },
        ],
    }));
    (&raw const m.kind) as *const c_void
}

pub extern "C" fn swift_get_function_type_metadata0(flags: usize, result: *const c_void) -> *const c_void {
    let key = (flags, result as usize, 0);
    let mut map = FUNCTION_CACHE.lock().unwrap();
    if let Some(&cached) = map.get(&key) {
        return cached as *const c_void;
    }
    let m = Box::leak(Box::new(FunctionTypeMetadata {
        vwt: &raw const VWT_BP_RAWPTR,
        kind: KIND_FUNCTION,
        flags,
        result,
        params: [0; 4],
    }));
    let ptr = (&raw const m.kind) as usize;
    map.insert(key, ptr);
    ptr as *const c_void
}

pub extern "C" fn swift_get_function_type_metadata1(
    flags: usize,
    param0: *const c_void,
    result: *const c_void,
) -> *const c_void {
    let key = (flags, result as usize, param0 as usize);
    let mut map = FUNCTION_CACHE.lock().unwrap();
    if let Some(&cached) = map.get(&key) {
        return cached as *const c_void;
    }
    let m = Box::leak(Box::new(FunctionTypeMetadata {
        vwt: &raw const VWT_BP_RAWPTR,
        kind: KIND_FUNCTION,
        flags,
        result,
        params: [param0 as usize, 0, 0, 0],
    }));
    let ptr = (&raw const m.kind) as usize;
    map.insert(key, ptr);
    ptr as *const c_void
}

pub extern "C" fn swift_get_tuple_type_metadata(
    _req: usize,
    flags: usize,
    elements: *const *const c_void,
    labels: *const c_char,
    proposed_witness: *const ValueWitnessTable,
) -> MetadataResponse {
    let n = (flags & 0xFFFF).min(4);
    let vwt = if proposed_witness.is_null() {
        &raw const VWT_BI64
    } else {
        proposed_witness
    };
    let mut tuple_elems = [
        TupleElement { ty: std::ptr::null(), offset: 0 },
        TupleElement { ty: std::ptr::null(), offset: 0 },
        TupleElement { ty: std::ptr::null(), offset: 0 },
        TupleElement { ty: std::ptr::null(), offset: 0 },
    ];
    let mut off = 0usize;
    for i in 0..n {
        let t = if elements.is_null() { std::ptr::null() } else { unsafe { *elements.add(i) } };
        tuple_elems[i] = TupleElement { ty: t, offset: off };
        off += type_size(t);
    }
    let m = Box::leak(Box::new(TupleTypeMetadata {
        vwt,
        kind: KIND_TUPLE,
        num_elements: n,
        labels,
        elements: tuple_elems,
    }));
    MetadataResponse {
        value: (&raw const m.kind) as *const c_void,
        state: 0,
    }
}

pub extern "C" fn swift_get_tuple_type_metadata2(
    _req: usize,
    e0: *const c_void,
    e1: *const c_void,
    labels: *const c_char,
    proposed_witness: *const ValueWitnessTable,
) -> MetadataResponse {
    let key = (e0 as usize, e1 as usize, labels as usize);
    let mut map = TUPLE_CACHE.lock().unwrap();
    if let Some(&cached) = map.get(&key) {
        return MetadataResponse {
            value: cached as *const c_void,
            state: 0,
        };
    }
    let s0 = type_size(e0);
    let vwt = if proposed_witness.is_null() { &raw const VWT_BI128 } else { proposed_witness };
    let m = Box::leak(Box::new(TupleTypeMetadata {
        vwt,
        kind: KIND_TUPLE,
        num_elements: 2,
        labels,
        elements: [
            TupleElement { ty: e0, offset: 0 },
            TupleElement { ty: e1, offset: s0 },
            TupleElement { ty: std::ptr::null(), offset: 0 },
            TupleElement { ty: std::ptr::null(), offset: 0 },
        ],
    }));
    let ptr = (&raw const m.kind) as usize;
    map.insert(key, ptr);
    MetadataResponse {
        value: ptr as *const c_void,
        state: 0,
    }
}

pub extern "C" fn swift_get_tuple_type_metadata3(
    _req: usize,
    e0: *const c_void,
    e1: *const c_void,
    e2: *const c_void,
    labels: *const c_char,
    proposed_witness: *const ValueWitnessTable,
) -> MetadataResponse {
    let elems = [e0, e1, e2];
    swift_get_tuple_type_metadata(0, 3, elems.as_ptr(), labels, proposed_witness)
}

#[repr(C)]
pub struct TupleTypeLayout2 {
    pub size: usize,
    pub stride: usize,
    pub flags: usize,
    pub element1_offset: usize,
}

pub extern "C" fn swift_get_tuple_type_layout2(
    _layout0: *const c_void,
    _layout1: *const c_void,
) -> TupleTypeLayout2 {
    TupleTypeLayout2 {
        size: 16,
        stride: 16,
        flags: 7,
        element1_offset: 8,
    }
}

pub extern "C" fn swift_get_type_by_mangled_name_in_context(
    name: *const c_char,
    name_len: usize,
    _ctx: *const c_void,
    _generic_args: *const *const c_void,
) -> *const c_void {
    if !name.is_null() && name_len > 0 {
        let slice = unsafe { std::slice::from_raw_parts(name as *const u8, name_len.min(256)) };
        let s = String::from_utf8_lossy(slice);
        if let Some(meta) = crate::overlays::lookup_builtin_type_by_name(&s) {
            return meta;
        }
        if let Some(cls) = objc_rt::class_named(&s) {
            return swift_get_objc_class_metadata(cls);
        }
    }
    anyobject_metadata()
}

pub extern "C" fn swift_check_metadata_state(_req: usize, ty: *const c_void) -> MetadataResponse {
    MetadataResponse {
        value: ty,
        state: 0,
    }
}

// ---- Protocol conformance & dynamic casting --------------------------------

pub extern "C" fn swift_get_witness_table(
    conformance: *const c_void,
    ty: *const c_void,
    _instantiation_args: *const *const c_void,
) -> *const c_void {
    let key = (conformance as usize, ty as usize);
    let mut map = WITNESS_CACHE.lock().unwrap();
    if let Some(&cached) = map.get(&key) {
        return cached as *const c_void;
    }
    let wt = Box::leak(Box::new(ProtocolWitnessTableStub {
        conformance,
        methods: [fallback_method_stub as *const () as usize; 31],
    }));
    let ptr = wt as *const ProtocolWitnessTableStub as usize;
    map.insert(key, ptr);
    ptr as *const c_void
}

pub extern "C" fn swift_conforms_to_protocol(ty: *const c_void, protocol: *const c_void) -> *const c_void {
    if ty.is_null() || protocol.is_null() {
        return std::ptr::null();
    }
    swift_get_witness_table(protocol, ty, std::ptr::null())
}

pub extern "C" fn swift_get_associated_type_witness(
    _req: usize,
    _wtable: *const c_void,
    _conforming_ty: *const c_void,
    _req_base: *const c_void,
    _assoc_type: *const c_void,
) -> MetadataResponse {
    MetadataResponse {
        value: anyobject_metadata(),
        state: 0,
    }
}

pub extern "C" fn swift_get_associated_conformance_witness(
    wtable: *const c_void,
    conforming_ty: *const c_void,
    _assoc_ty: *const c_void,
    _req_base: *const c_void,
    assoc_conformance: *const c_void,
) -> *const c_void {
    swift_get_witness_table(assoc_conformance, conforming_ty, &wtable)
}

pub extern "C" fn swift_dynamic_cast_class(obj: Id, target_cls: *const ClassT) -> Id {
    if obj.is_null() || target_cls.is_null() || runtime::is_tagged_or_null(obj as usize) {
        return std::ptr::null_mut();
    }
    let mut c = unsafe { objc_rt::object_class(obj) };
    let target = swift_get_objc_class_from_metadata(target_cls as *const c_void);
    while !c.is_null() {
        if c == target_cls || c == target {
            return obj;
        }
        c = objc_rt::superclass(c);
    }
    std::ptr::null_mut()
}

pub extern "C" fn swift_dynamic_cast_objc_class(obj: Id, target_cls: *const ClassT) -> Id {
    swift_dynamic_cast_class(obj, target_cls)
}

pub extern "C" fn swift_dynamic_cast_unknown_class_unconditional(
    obj: Id,
    target_cls: *const ClassT,
    _file: *const c_char,
    _line: u32,
    _col: u32,
) -> Id {
    let r = swift_dynamic_cast_class(obj, target_cls);
    if r.is_null() { obj } else { r }
}

pub extern "C" fn swift_dynamic_cast_metatype(src: *const c_void, target: *const c_void) -> *const c_void {
    if src == target {
        return src;
    }
    if !swift_dynamic_cast_class(src as Id, target as *const ClassT).is_null() {
        src
    } else {
        std::ptr::null()
    }
}

pub extern "C" fn swift_dynamic_cast_objc_protocol_conditional(
    obj: Id,
    _num_protocols: usize,
    _protocols: *const *const pear_libobjc::abi::ProtocolT,
) -> Id {
    obj
}

pub extern "C" fn swift_dynamic_cast_type_to_objc_protocol_conditional(
    ty: *const c_void,
    _num_protocols: usize,
    _protocols: *const *const pear_libobjc::abi::ProtocolT,
) -> *const c_void {
    ty
}

pub extern "C" fn swift_dynamic_cast(
    dest: *mut c_void,
    src: *mut c_void,
    src_ty: *const c_void,
    target_ty: *const c_void,
    flags: usize,
) -> bool {
    const TAKE_ON_SUCCESS: usize = 1;
    const DESTROY_ON_FAILURE: usize = 4;
    if dest.is_null() || src.is_null() {
        return false;
    }
    let matched = if src_ty == target_ty || target_ty.is_null() || target_ty == anyobject_metadata() {
        let sz = type_size(target_ty.max(src_ty));
        if (flags & TAKE_ON_SUCCESS) != 0 {
            unsafe { std::ptr::copy(src as *const u8, dest as *mut u8, sz) };
        } else {
            let vwt = unsafe { &*metadata_vwt(src_ty) };
            (vwt.initialize_with_copy)(dest, src, src_ty);
        }
        true
    } else if swift_is_class_type(src_ty) && swift_is_class_type(target_ty) {
        let obj = unsafe { *(src as *const Id) };
        let cast = swift_dynamic_cast_class(obj, target_ty as *const ClassT);
        if !cast.is_null() {
            if (flags & TAKE_ON_SUCCESS) == 0 {
                swift_retain(cast);
            }
            unsafe { *(dest as *mut Id) = cast };
            true
        } else {
            false
        }
    } else {
        false
    };
    if !matched && (flags & DESTROY_ON_FAILURE) != 0 {
        let vwt = unsafe { &*metadata_vwt(src_ty) };
        (vwt.destroy)(src, src_ty);
    }
    matched
}

// ---- Array / Enum / KeyPath helpers ----------------------------------------

pub extern "C" fn swift_array_init_with_copy(
    dest: *mut c_void,
    src: *mut c_void,
    count: usize,
    self_ty: *const c_void,
) {
    if dest.is_null() || src.is_null() || count == 0 {
        return;
    }
    let vwt = unsafe { &*metadata_vwt(self_ty) };
    let stride = vwt.stride.max(1);
    for i in 0..count {
        unsafe {
            let d = (dest as *mut u8).add(i * stride) as *mut c_void;
            let s = (src as *mut u8).add(i * stride) as *mut c_void;
            (vwt.initialize_with_copy)(d, s, self_ty);
        }
    }
}

pub extern "C" fn swift_array_init_with_take_front_to_back(
    dest: *mut c_void,
    src: *mut c_void,
    count: usize,
    self_ty: *const c_void,
) {
    if dest.is_null() || src.is_null() || count == 0 {
        return;
    }
    let vwt = unsafe { &*metadata_vwt(self_ty) };
    let stride = vwt.stride.max(1);
    for i in 0..count {
        unsafe {
            let d = (dest as *mut u8).add(i * stride) as *mut c_void;
            let s = (src as *mut u8).add(i * stride) as *mut c_void;
            (vwt.initialize_with_take)(d, s, self_ty);
        }
    }
}

pub extern "C" fn swift_array_init_with_take_back_to_front(
    dest: *mut c_void,
    src: *mut c_void,
    count: usize,
    self_ty: *const c_void,
) {
    if dest.is_null() || src.is_null() || count == 0 {
        return;
    }
    let vwt = unsafe { &*metadata_vwt(self_ty) };
    let stride = vwt.stride.max(1);
    for i in (0..count).rev() {
        unsafe {
            let d = (dest as *mut u8).add(i * stride) as *mut c_void;
            let s = (src as *mut u8).add(i * stride) as *mut c_void;
            (vwt.initialize_with_take)(d, s, self_ty);
        }
    }
}

pub extern "C" fn swift_array_assign_with_take(
    dest: *mut c_void,
    src: *mut c_void,
    count: usize,
    self_ty: *const c_void,
) {
    if dest.is_null() || src.is_null() || count == 0 {
        return;
    }
    let vwt = unsafe { &*metadata_vwt(self_ty) };
    let stride = vwt.stride.max(1);
    for i in 0..count {
        unsafe {
            let d = (dest as *mut u8).add(i * stride) as *mut c_void;
            let s = (src as *mut u8).add(i * stride) as *mut c_void;
            (vwt.assign_with_take)(d, s, self_ty);
        }
    }
}

pub extern "C" fn swift_array_destroy(begin: *mut c_void, count: usize, self_ty: *const c_void) {
    if begin.is_null() || count == 0 {
        return;
    }
    let vwt = unsafe { &*metadata_vwt(self_ty) };
    let stride = vwt.stride.max(1);
    for i in 0..count {
        unsafe {
            let e = (begin as *mut u8).add(i * stride) as *mut c_void;
            (vwt.destroy)(e, self_ty);
        }
    }
}

pub extern "C" fn swift_get_enum_tag_single_payload_generic(
    val: *const c_void,
    empty_cases: u32,
    payload_type: *const c_void,
    _get_extra_inhabitant_tag: *const c_void,
) -> u32 {
    if val.is_null() || empty_cases == 0 {
        return 0;
    }
    let sz = type_size(payload_type);
    let extra_byte = unsafe { *(val as *const u8).add(sz) };
    if extra_byte != 0 {
        extra_byte as u32
    } else {
        0
    }
}

pub extern "C" fn swift_store_enum_tag_single_payload_generic(
    val: *mut c_void,
    which_case: u32,
    empty_cases: u32,
    payload_type: *const c_void,
    _store_extra_inhabitant_tag: *const c_void,
) {
    if val.is_null() || empty_cases == 0 {
        return;
    }
    let sz = type_size(payload_type);
    unsafe { *(val as *mut u8).add(sz) = which_case as u8 };
}

pub extern "C" fn swift_get_enum_case_multi_payload(val: *const c_void, _enum_ty: *const c_void) -> u32 {
    if val.is_null() {
        return 0;
    }
    unsafe { *(val as *const u8) as u32 }
}

pub extern "C" fn swift_store_enum_tag_multi_payload(val: *mut c_void, _enum_ty: *const c_void, which_case: u32) {
    if !val.is_null() {
        unsafe { *(val as *mut u8) = which_case as u8 };
    }
}

pub extern "C" fn swift_get_keypath(pattern: *const c_void, _arguments: *const c_void) -> Id {
    let obj = runtime::swift_alloc_object(SWIFT_OBJECT_CLASS.ptr(), 32, 7);
    if !obj.is_null() {
        unsafe { *((obj as *mut usize).add(2)) = pattern as usize };
    }
    obj
}

pub extern "C" fn swift_get_at_keypath(out: *mut c_void, _root: *const c_void, _keypath: Id) {
    if !out.is_null() {
        unsafe { *(out as *mut usize) = 0 };
    }
}

pub extern "C" fn swift_read_at_keypath(
    out: *mut c_void,
    _scratch: *mut c_void,
    _root: *const c_void,
    _keypath: Id,
) {
    if !out.is_null() {
        unsafe { *(out as *mut usize) = 0 };
    }
}

pub extern "C" fn swift_set_at_reference_writable_keypath(_root: Id, _keypath: Id, _val: *const c_void) {}

/// Resolves Swift 5 ABI metadata, VWT, casting, array, enum, and KeyPath symbols.
pub fn metadata_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        // Built-in Value Witness Tables
        "_$sBoWV" => Some(&raw const VWT_BO_NATIVE as usize),
        "_$sBOWV" => Some(&raw const VWT_BO_UNKNOWN as usize),
        "_$sBbWV" => Some(&raw const VWT_BB_BRIDGE as usize),
        "_$sBpWV" => Some(&raw const VWT_BP_RAWPTR as usize),
        "_$sBi8_WV" => Some(&raw const VWT_BI8 as usize),
        "_$sBi16_WV" => Some(&raw const VWT_BI16 as usize),
        "_$sBi32_WV" => Some(&raw const VWT_BI32 as usize),
        "_$sBi64_WV" => Some(&raw const VWT_BI64 as usize),
        "_$sBi128_WV" => Some(&raw const VWT_BI128 as usize),
        "_$sytWV" => Some(&raw const VWT_YT_VOID as usize),
        // Metadata & ObjC reflection
        "_swift_getObjCClassMetadata" => f(swift_get_objc_class_metadata as *const ()),
        "_swift_getObjCClassFromMetadata" => f(swift_get_objc_class_from_metadata as *const ()),
        "_swift_getInitializedObjCClass" => f(swift_get_initialized_objc_class as *const ()),
        "_swift_getObjectType" => f(swift_get_object_type as *const ()),
        "_swift_getDynamicType" => f(swift_get_dynamic_type as *const ()),
        "_swift_isClassType" => f(swift_is_class_type as *const ()),
        "_swift_lookUpClassMethod" => f(swift_lookup_class_method as *const ()),
        "_swift_initClassMetadata2" => f(swift_init_class_metadata2 as *const ()),
        "_swift_updateClassMetadata2" => f(swift_update_class_metadata2 as *const ()),
        "_swift_allocateGenericClassMetadata" => f(swift_allocate_generic_class_metadata as *const ()),
        "_swift_allocateGenericValueMetadata" => f(swift_allocate_generic_value_metadata as *const ()),
        "_swift_initStructMetadata" => f(swift_init_struct_metadata as *const ()),
        "_swift_initEnumMetadataSinglePayload" => f(swift_init_enum_metadata_single_payload as *const ()),
        "_swift_initEnumMetadataMultiPayload" => f(swift_init_enum_metadata_multi_payload as *const ()),
        "_swift_getGenericMetadata" => f(swift_get_generic_metadata as *const ()),
        "_swift_getSingletonMetadata" => f(swift_get_singleton_metadata as *const ()),
        "_swift_getForeignTypeMetadata" => f(swift_get_foreign_type_metadata as *const ()),
        "_swift_getMetatypeMetadata" => f(swift_get_metatype_metadata as *const ()),
        "_swift_getExistentialTypeMetadata" => f(swift_get_existential_type_metadata as *const ()),
        "_swift_getFunctionTypeMetadata0" => f(swift_get_function_type_metadata0 as *const ()),
        "_swift_getFunctionTypeMetadata1" => f(swift_get_function_type_metadata1 as *const ()),
        "_swift_getTupleTypeMetadata" => f(swift_get_tuple_type_metadata as *const ()),
        "_swift_getTupleTypeMetadata2" => f(swift_get_tuple_type_metadata2 as *const ()),
        "_swift_getTupleTypeMetadata3" => f(swift_get_tuple_type_metadata3 as *const ()),
        "_swift_getTupleTypeLayout2" => f(swift_get_tuple_type_layout2 as *const ()),
        "_swift_getTypeByMangledNameInContext" => f(swift_get_type_by_mangled_name_in_context as *const ()),
        "_swift_checkMetadataState" => f(swift_check_metadata_state as *const ()),
        // Conformance & casts
        "_swift_getWitnessTable" => f(swift_get_witness_table as *const ()),
        "_swift_conformsToProtocol" => f(swift_conforms_to_protocol as *const ()),
        "_swift_getAssociatedTypeWitness" => f(swift_get_associated_type_witness as *const ()),
        "_swift_getAssociatedConformanceWitness" => f(swift_get_associated_conformance_witness as *const ()),
        "_swift_dynamicCast" => f(swift_dynamic_cast as *const ()),
        "_swift_dynamicCastClass" => f(swift_dynamic_cast_class as *const ()),
        "_swift_dynamicCastMetatype" => f(swift_dynamic_cast_metatype as *const ()),
        "_swift_dynamicCastObjCClass" => f(swift_dynamic_cast_objc_class as *const ()),
        "_swift_dynamicCastObjCProtocolConditional" => f(swift_dynamic_cast_objc_protocol_conditional as *const ()),
        "_swift_dynamicCastTypeToObjCProtocolConditional" => f(swift_dynamic_cast_type_to_objc_protocol_conditional as *const ()),
        "_swift_dynamicCastUnknownClassUnconditional" => f(swift_dynamic_cast_unknown_class_unconditional as *const ()),
        // Arrays / Enums / KeyPaths
        "_swift_arrayInitWithCopy" => f(swift_array_init_with_copy as *const ()),
        "_swift_arrayInitWithTakeFrontToBack" => f(swift_array_init_with_take_front_to_back as *const ()),
        "_swift_arrayInitWithTakeBackToFront" => f(swift_array_init_with_take_back_to_front as *const ()),
        "_swift_arrayAssignWithTake" => f(swift_array_assign_with_take as *const ()),
        "_swift_arrayDestroy" => f(swift_array_destroy as *const ()),
        "_swift_getEnumTagSinglePayloadGeneric" => f(swift_get_enum_tag_single_payload_generic as *const ()),
        "_swift_storeEnumTagSinglePayloadGeneric" => f(swift_store_enum_tag_single_payload_generic as *const ()),
        "_swift_getEnumCaseMultiPayload" => f(swift_get_enum_case_multi_payload as *const ()),
        "_swift_storeEnumTagMultiPayload" => f(swift_store_enum_tag_multi_payload as *const ()),
        "_swift_getKeyPath" => f(swift_get_keypath as *const ()),
        "_swift_getAtKeyPath" => f(swift_get_at_keypath as *const ()),
        "_swift_readAtKeyPath" => f(swift_read_at_keypath as *const ()),
        "_swift_setAtReferenceWritableKeyPath" => f(swift_set_at_reference_writable_keypath as *const ()),
        _ => None,
    }
}
