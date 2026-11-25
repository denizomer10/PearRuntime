//! Clean-room: ABI layouts and semantics from Apple's open-source objc4
//! (APSL 2.0, reference only, nothing copied) and the public objc/runtime.h
//! documentation.
//!
//! Objective-C reflection, runtime class creation (`objc_allocateClassPair`),
//! dynamic method/ivar/protocol/property inspection, and type-encoding parser.

use crate::abi::*;
use crate::object::{alloc, dispose, EMPTY_CACHE, NSOBJECT_META};
use crate::runtime::{
    self, all_classes, class, class_named, flush_cache, image_names, lookup, lookup_method,
    object_class, own_method_entry, protocol_named, sel, superclass, IvarEntry, MethodEntry,
    PropertyEntry,
};
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::Ordering;

pub type MethodHandle = *const MethodEntry;
pub type IvarHandle = *const IvarEntry;
pub type PropertyHandle = *const PropertyEntry;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ObjcMethodDescription {
    pub name: Sel,
    pub types: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ObjcPropertyAttribute {
    pub name: *const c_char,
    pub value: *const c_char,
}

extern "C" {
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn malloc(size: usize) -> *mut c_void;
    fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
}

unsafe fn alloc_null_terminated<T: Copy>(items: &[T], out_count: *mut u32) -> *mut T {
    if !out_count.is_null() {
        *out_count = items.len() as u32;
    }
    if items.is_empty() {
        return std::ptr::null_mut();
    }
    let buf = calloc(items.len() + 1, std::mem::size_of::<T>()) as *mut T;
    if !buf.is_null() {
        for (i, &item) in items.iter().enumerate() {
            *buf.add(i) = item;
        }
    }
    buf
}

unsafe fn strdup_bytes(bytes: &[u8]) -> *mut c_char {
    let p = malloc(bytes.len() + 1) as *mut u8;
    if !p.is_null() {
        if !bytes.is_empty() {
            memcpy(p as *mut c_void, bytes.as_ptr() as *const c_void, bytes.len());
        }
        *p.add(bytes.len()) = 0;
    }
    p as *mut c_char
}

unsafe fn copy_str_to_buf(src: &[u8], dst: *mut c_char, dst_len: usize) {
    if dst.is_null() || dst_len == 0 {
        return;
    }
    let n = src.len().min(dst_len - 1);
    if n > 0 {
        memcpy(dst as *mut c_void, src.as_ptr() as *const c_void, n);
    }
    *(dst as *mut u8).add(n) = 0;
}

// ---- Type encoding parser --------------------------------------------------

fn skip_balanced(b: &[u8], mut i: usize, open: u8, close: u8) -> usize {
    let mut depth = 0usize;
    while i < b.len() {
        if b[i] == open {
            depth += 1;
        } else if b[i] == close {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
        i += 1;
    }
    i
}

fn parse_one_type(b: &[u8], mut i: usize) -> (usize, usize) {
    // Skip type qualifiers r, n, N, o, O, R, V
    while i < b.len() && matches!(b[i], b'r' | b'n' | b'N' | b'o' | b'O' | b'R' | b'V') {
        i += 1;
    }
    let start = i;
    if i >= b.len() {
        return (start, i);
    }
    match b[i] {
        b'{' => i = skip_balanced(b, i, b'{', b'}'),
        b'(' => i = skip_balanced(b, i, b'(', b')'),
        b'[' => i = skip_balanced(b, i, b'[', b']'),
        b'^' => {
            i += 1;
            let (_, end) = parse_one_type(b, i);
            i = end;
        }
        b'b' => {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        b'@' => {
            i += 1;
            if i < b.len() && b[i] == b'?' {
                i += 1;
            } else if i < b.len() && b[i] == b'"' {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                }
            }
        }
        _ => {
            i += 1;
        }
    }
    (start, i)
}

fn type_slots(types: *const c_char) -> Vec<&'static [u8]> {
    if types.is_null() {
        return Vec::new();
    }
    let b = unsafe { CStr::from_ptr(types).to_bytes() };
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let (start, end) = parse_one_type(b, i);
        if end <= start {
            break;
        }
        out.push(&b[start..end]);
        i = end;
        // Skip stack/register offset digits
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'+') {
            i += 1;
        }
    }
    out
}

// ---- Classes & methods -----------------------------------------------------

pub unsafe extern "C" fn class_get_name(cls: *const ClassT) -> *const c_char {
    if cls.is_null() {
        return c"nil".as_ptr();
    }
    if let Some(c) = class(cls) {
        let leaked = Box::leak(CString::new(c.name.as_str()).unwrap_or_default().into_boxed_c_str());
        return leaked.as_ptr();
    }
    let r = runtime::ro(cls);
    if !r.is_null() && !(*r).name.is_null() {
        return (*r).name;
    }
    c"".as_ptr()
}

pub unsafe extern "C" fn class_get_instance_method(cls: *const ClassT, s: Sel) -> MethodHandle {
    if cls.is_null() || s.is_null() {
        return std::ptr::null();
    }
    lookup_method(cls, s).map_or(std::ptr::null(), |m| m as MethodHandle)
}

pub unsafe extern "C" fn class_get_class_method(cls: *const ClassT, s: Sel) -> MethodHandle {
    if cls.is_null() || s.is_null() {
        return std::ptr::null();
    }
    class_get_instance_method((*cls).isa, s)
}

pub unsafe extern "C" fn class_copy_method_list(cls: *const ClassT, out_count: *mut u32) -> *mut MethodHandle {
    let Some(c) = class(cls) else {
        return alloc_null_terminated::<MethodHandle>(&[], out_count);
    };
    let list = c.method_list.read().unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut dedup = Vec::new();
    for &m in list.iter().rev() {
        if seen.insert(m.name as usize) {
            dedup.push(m as MethodHandle);
        }
    }
    dedup.reverse();
    alloc_null_terminated(&dedup, out_count)
}

pub unsafe extern "C" fn class_get_method_implementation(cls: *const ClassT, s: Sel) -> Imp {
    if cls.is_null() || s.is_null() {
        return std::ptr::null();
    }
    lookup(cls, s).unwrap_or(crate::message::objc_msg_forward as *const () as Imp)
}

pub unsafe extern "C" fn class_replace_method(
    cls: *const ClassT,
    s: Sel,
    imp: Imp,
    types: *const c_char,
) -> Imp {
    if cls.is_null() || s.is_null() {
        return std::ptr::null();
    }
    if let Some(m) = own_method_entry(cls, s) {
        let old = m.imp.swap(imp as usize, Ordering::AcqRel) as Imp;
        flush_cache();
        return old;
    }
    let prev = lookup(cls, s).unwrap_or(std::ptr::null());
    runtime::add_methods(cls, &[MethodDesc { name: s, types, imp }]);
    prev
}

pub unsafe extern "C" fn method_get_name(m: MethodHandle) -> Sel {
    if m.is_null() { std::ptr::null() } else { (*m).name }
}

pub unsafe extern "C" fn method_get_implementation(m: MethodHandle) -> Imp {
    if m.is_null() { std::ptr::null() } else { (*m).imp() }
}

pub unsafe extern "C" fn method_set_implementation(m: MethodHandle, imp: Imp) -> Imp {
    if m.is_null() {
        return std::ptr::null();
    }
    let old = (*m).imp.swap(imp as usize, Ordering::AcqRel) as Imp;
    flush_cache();
    old
}

pub unsafe extern "C" fn method_get_type_encoding(m: MethodHandle) -> *const c_char {
    if m.is_null() { std::ptr::null() } else { (*m).types }
}

pub unsafe extern "C" fn method_get_number_of_arguments(m: MethodHandle) -> u32 {
    if m.is_null() {
        return 0;
    }
    type_slots((*m).types).len().saturating_sub(1) as u32
}

pub unsafe extern "C" fn method_copy_return_type(m: MethodHandle) -> *mut c_char {
    if m.is_null() {
        return std::ptr::null_mut();
    }
    let slots = type_slots((*m).types);
    strdup_bytes(slots.first().copied().unwrap_or(b"v"))
}

pub unsafe extern "C" fn method_copy_argument_type(m: MethodHandle, index: u32) -> *mut c_char {
    if m.is_null() {
        return std::ptr::null_mut();
    }
    let slots = type_slots((*m).types);
    let Some(&slot) = slots.get(index as usize + 1) else {
        return std::ptr::null_mut();
    };
    strdup_bytes(slot)
}

pub unsafe extern "C" fn method_get_return_type(m: MethodHandle, dst: *mut c_char, dst_len: usize) {
    if m.is_null() {
        copy_str_to_buf(b"", dst, dst_len);
        return;
    }
    let slots = type_slots((*m).types);
    copy_str_to_buf(slots.first().copied().unwrap_or(b"v"), dst, dst_len);
}

pub unsafe extern "C" fn method_get_argument_type(
    m: MethodHandle,
    index: u32,
    dst: *mut c_char,
    dst_len: usize,
) {
    if m.is_null() {
        copy_str_to_buf(b"", dst, dst_len);
        return;
    }
    let slots = type_slots((*m).types);
    copy_str_to_buf(slots.get(index as usize + 1).copied().unwrap_or(b""), dst, dst_len);
}

pub unsafe extern "C" fn method_get_description(m: MethodHandle) -> *const ObjcMethodDescription {
    if m.is_null() {
        return std::ptr::null();
    }
    // MethodEntry starts with `name: Sel, types: *const c_char` — identical layout.
    m as *const ObjcMethodDescription
}

pub unsafe extern "C" fn method_exchange_implementations(m1: MethodHandle, m2: MethodHandle) {
    if m1.is_null() || m2.is_null() || m1 == m2 {
        return;
    }
    let i1 = (*m1).imp.load(Ordering::Acquire);
    let i2 = (*m2).imp.swap(i1, Ordering::AcqRel);
    (*m1).imp.store(i2, Ordering::Release);
    flush_cache();
}

// ---- Ivars -----------------------------------------------------------------

pub unsafe extern "C" fn class_get_instance_variable(
    cls: *const ClassT,
    name: *const c_char,
) -> IvarHandle {
    if cls.is_null() || name.is_null() {
        return std::ptr::null();
    }
    let target = CStr::from_ptr(name).to_bytes();
    let mut c = cls;
    while !c.is_null() {
        if let Some(ci) = class(c) {
            for &iv in ci.ivars.read().unwrap().iter() {
                if !iv.name.is_null() && CStr::from_ptr(iv.name).to_bytes() == target {
                    return iv as IvarHandle;
                }
            }
        }
        c = superclass(c);
    }
    std::ptr::null()
}

pub unsafe extern "C" fn ivar_get_name(iv: IvarHandle) -> *const c_char {
    if iv.is_null() { std::ptr::null() } else { (*iv).name }
}

pub unsafe extern "C" fn ivar_get_offset(iv: IvarHandle) -> isize {
    if iv.is_null() { 0 } else { (*iv).offset() }
}

pub unsafe extern "C" fn ivar_get_type_encoding(iv: IvarHandle) -> *const c_char {
    if iv.is_null() { std::ptr::null() } else { (*iv).types }
}

pub unsafe extern "C" fn class_copy_ivar_list(
    cls: *const ClassT,
    out_count: *mut u32,
) -> *mut IvarHandle {
    let Some(c) = class(cls) else {
        return alloc_null_terminated::<IvarHandle>(&[], out_count);
    };
    let ivars: Vec<IvarHandle> = c.ivars.read().unwrap().iter().map(|&iv| iv as IvarHandle).collect();
    alloc_null_terminated(&ivars, out_count)
}

pub unsafe extern "C" fn class_get_instance_size(cls: *const ClassT) -> usize {
    if cls.is_null() { 0 } else { class(cls).map_or(0, |c| c.size()) }
}

pub unsafe extern "C" fn class_is_meta_class(cls: *const ClassT) -> bool {
    !cls.is_null() && class(cls).is_some_and(|c| c.meta)
}

// ---- Protocols & Properties ------------------------------------------------

pub unsafe extern "C" fn class_conforms_to_protocol(
    cls: *const ClassT,
    proto: *const ProtocolT,
) -> bool {
    if cls.is_null() || proto.is_null() || (*proto).mangled_name.is_null() {
        return false;
    }
    runtime::conforms(cls, CStr::from_ptr((*proto).mangled_name))
}

pub unsafe extern "C" fn class_add_protocol(cls: *const ClassT, proto: *const ProtocolT) -> bool {
    if cls.is_null() || proto.is_null() {
        return false;
    }
    let Some(c) = class(cls) else {
        return false;
    };
    runtime::register_protocol(proto);
    let mut list = c.protocols.write().unwrap();
    if list.contains(&proto) {
        return false;
    }
    list.push(proto);
    true
}

pub unsafe extern "C" fn class_copy_protocol_list(
    cls: *const ClassT,
    out_count: *mut u32,
) -> *mut *const ProtocolT {
    let Some(c) = class(cls) else {
        return alloc_null_terminated::<*const ProtocolT>(&[], out_count);
    };
    let protos = c.protocols.read().unwrap();
    alloc_null_terminated(&protos, out_count)
}

pub unsafe extern "C" fn objc_get_protocol(name: *const c_char) -> *const ProtocolT {
    if name.is_null() {
        return std::ptr::null();
    }
    let n = CStr::from_ptr(name).to_string_lossy();
    protocol_named(&n).unwrap_or(std::ptr::null())
}

pub unsafe extern "C" fn protocol_get_name(p: *const ProtocolT) -> *const c_char {
    if p.is_null() { c"".as_ptr() } else { (*p).mangled_name }
}

pub unsafe extern "C" fn protocol_is_equal(p: *const ProtocolT, other: *const ProtocolT) -> bool {
    if p == other {
        return true;
    }
    if p.is_null() || other.is_null() || (*p).mangled_name.is_null() || (*other).mangled_name.is_null() {
        return false;
    }
    CStr::from_ptr((*p).mangled_name) == CStr::from_ptr((*other).mangled_name)
}

pub unsafe extern "C" fn protocol_copy_protocol_list(
    p: *const ProtocolT,
    out_count: *mut u32,
) -> *mut *const ProtocolT {
    if p.is_null() {
        return alloc_null_terminated::<*const ProtocolT>(&[], out_count);
    }
    let list = protocols((*p).protocols);
    alloc_null_terminated(&list, out_count)
}

pub unsafe extern "C" fn protocol_conforms_to_protocol(
    p: *const ProtocolT,
    other: *const ProtocolT,
) -> bool {
    if p.is_null() || other.is_null() {
        return false;
    }
    if p == other {
        return true;
    }
    if !(*p).mangled_name.is_null() && !(*other).mangled_name.is_null() {
        if CStr::from_ptr((*p).mangled_name) == CStr::from_ptr((*other).mangled_name) {
            return true;
        }
    }
    for sub in protocols((*p).protocols) {
        if protocol_conforms_to_protocol(sub, other) {
            return true;
        }
    }
    false
}

unsafe fn proto_method_list(
    p: *const ProtocolT,
    is_required: bool,
    is_instance: bool,
) -> *const MethodList {
    match (is_required, is_instance) {
        (true, true) => (*p).instance_methods,
        (true, false) => (*p).class_methods,
        (false, true) => (*p).optional_instance_methods,
        (false, false) => (*p).optional_class_methods,
    }
}

pub unsafe extern "C" fn protocol_get_method_description(
    p: *const ProtocolT,
    s: Sel,
    is_required: bool,
    is_instance: bool,
) -> ObjcMethodDescription {
    if p.is_null() || s.is_null() {
        return ObjcMethodDescription { name: std::ptr::null(), types: std::ptr::null() };
    }
    for m in methods(proto_method_list(p, is_required, is_instance)) {
        if m.name == s {
            return ObjcMethodDescription { name: m.name, types: m.types };
        }
    }
    for sub in protocols((*p).protocols) {
        let d = protocol_get_method_description(sub, s, is_required, is_instance);
        if !d.name.is_null() {
            return d;
        }
    }
    ObjcMethodDescription { name: std::ptr::null(), types: std::ptr::null() }
}

pub unsafe extern "C" fn protocol_copy_method_description_list(
    p: *const ProtocolT,
    is_required: bool,
    is_instance: bool,
    out_count: *mut u32,
) -> *mut ObjcMethodDescription {
    if p.is_null() {
        return alloc_null_terminated::<ObjcMethodDescription>(&[], out_count);
    }
    let list: Vec<ObjcMethodDescription> = methods(proto_method_list(p, is_required, is_instance))
        .into_iter()
        .map(|m| ObjcMethodDescription { name: m.name, types: m.types })
        .collect();
    if !out_count.is_null() {
        *out_count = list.len() as u32;
    }
    if list.is_empty() {
        return std::ptr::null_mut();
    }
    let buf = calloc(list.len(), std::mem::size_of::<ObjcMethodDescription>()) as *mut ObjcMethodDescription;
    for (i, d) in list.into_iter().enumerate() {
        *buf.add(i) = d;
    }
    buf
}

pub unsafe extern "C" fn protocol_copy_property_list(
    p: *const ProtocolT,
    out_count: *mut u32,
) -> *mut PropertyHandle {
    if p.is_null() {
        return alloc_null_terminated::<PropertyHandle>(&[], out_count);
    }
    let props: Vec<PropertyHandle> = properties((*p).instance_properties)
        .into_iter()
        .map(|(name, attributes)| Box::leak(Box::new(PropertyEntry { name, attributes })) as PropertyHandle)
        .collect();
    alloc_null_terminated(&props, out_count)
}

pub unsafe extern "C" fn protocol_get_property(
    p: *const ProtocolT,
    name: *const c_char,
    _is_required_property: bool,
    is_instance_property: bool,
) -> PropertyHandle {
    if p.is_null() || name.is_null() || !is_instance_property {
        return std::ptr::null();
    }
    let target = CStr::from_ptr(name).to_bytes();
    for (pname, attributes) in properties((*p).instance_properties) {
        if !pname.is_null() && CStr::from_ptr(pname).to_bytes() == target {
            return Box::leak(Box::new(PropertyEntry { name: pname, attributes })) as PropertyHandle;
        }
    }
    for sub in protocols((*p).protocols) {
        let found = protocol_get_property(sub, name, _is_required_property, is_instance_property);
        if !found.is_null() {
            return found;
        }
    }
    std::ptr::null()
}

pub unsafe extern "C" fn class_copy_property_list(
    cls: *const ClassT,
    out_count: *mut u32,
) -> *mut PropertyHandle {
    let Some(c) = class(cls) else {
        return alloc_null_terminated::<PropertyHandle>(&[], out_count);
    };
    let props: Vec<PropertyHandle> = c.properties.read().unwrap().iter().map(|&p| p as PropertyHandle).collect();
    alloc_null_terminated(&props, out_count)
}

pub unsafe extern "C" fn class_get_property(
    cls: *const ClassT,
    name: *const c_char,
) -> PropertyHandle {
    if cls.is_null() || name.is_null() {
        return std::ptr::null();
    }
    let target = CStr::from_ptr(name).to_bytes();
    let mut c = cls;
    while !c.is_null() {
        if let Some(ci) = class(c) {
            for &p in ci.properties.read().unwrap().iter() {
                if !p.name.is_null() && CStr::from_ptr(p.name).to_bytes() == target {
                    return p as PropertyHandle;
                }
            }
        }
        c = superclass(c);
    }
    std::ptr::null()
}

pub unsafe extern "C" fn class_add_property(
    cls: *const ClassT,
    name: *const c_char,
    attributes: *const ObjcPropertyAttribute,
    attribute_count: u32,
) -> bool {
    if cls.is_null() || name.is_null() {
        return false;
    }
    let Some(c) = class(cls) else {
        return false;
    };
    let mut attr_str = String::new();
    if !attributes.is_null() {
        for i in 0..attribute_count as usize {
            let a = *attributes.add(i);
            if !a.name.is_null() {
                if !attr_str.is_empty() { attr_str.push(','); }
                attr_str.push_str(&CStr::from_ptr(a.name).to_string_lossy());
                if !a.value.is_null() {
                    attr_str.push_str(&CStr::from_ptr(a.value).to_string_lossy());
                }
            }
        }
    }
    let entry = Box::leak(Box::new(PropertyEntry {
        name: strdup_bytes(CStr::from_ptr(name).to_bytes()),
        attributes: strdup_bytes(attr_str.as_bytes()),
    }));
    c.properties.write().unwrap().push(entry);
    true
}

pub unsafe extern "C" fn property_get_name(p: PropertyHandle) -> *const c_char {
    if p.is_null() { std::ptr::null() } else { (*p).name }
}

pub unsafe extern "C" fn property_get_attributes(p: PropertyHandle) -> *const c_char {
    if p.is_null() { std::ptr::null() } else { (*p).attributes }
}

pub unsafe extern "C" fn property_copy_attribute_list(
    p: PropertyHandle,
    out_count: *mut u32,
) -> *mut ObjcPropertyAttribute {
    if p.is_null() || (*p).attributes.is_null() {
        if !out_count.is_null() { *out_count = 0; }
        return std::ptr::null_mut();
    }
    let attrs = CStr::from_ptr((*p).attributes).to_string_lossy();
    let parts: Vec<&str> = attrs.split(',').filter(|s| !s.is_empty()).collect();
    if !out_count.is_null() {
        *out_count = parts.len() as u32;
    }
    if parts.is_empty() {
        return std::ptr::null_mut();
    }
    let mut str_bytes = 0usize;
    for part in &parts {
        str_bytes += 2 + part.len();
    }
    let header_bytes = parts.len() * std::mem::size_of::<ObjcPropertyAttribute>();
    let raw = calloc(1, header_bytes + str_bytes) as *mut u8;
    let list = raw as *mut ObjcPropertyAttribute;
    let mut cursor = raw.add(header_bytes);
    for (i, part) in parts.iter().enumerate() {
        let (k, v) = part.split_at(1);
        *cursor = k.as_bytes()[0];
        *cursor.add(1) = 0;
        let name_ptr = cursor as *const c_char;
        cursor = cursor.add(2);
        let val_ptr = cursor as *const c_char;
        if !v.is_empty() {
            memcpy(cursor as *mut c_void, v.as_ptr() as *const c_void, v.len());
        }
        *cursor.add(v.len()) = 0;
        cursor = cursor.add(v.len() + 1);
        *list.add(i) = ObjcPropertyAttribute { name: name_ptr, value: val_ptr };
    }
    list
}

pub unsafe extern "C" fn property_copy_attribute_value(
    p: PropertyHandle,
    attr_name: *const c_char,
) -> *mut c_char {
    if p.is_null() || (*p).attributes.is_null() || attr_name.is_null() {
        return std::ptr::null_mut();
    }
    let target = CStr::from_ptr(attr_name).to_string_lossy();
    let attrs = CStr::from_ptr((*p).attributes).to_string_lossy();
    for part in attrs.split(',') {
        if let Some(rest) = part.strip_prefix(target.as_ref()) {
            return strdup_bytes(rest.as_bytes());
        }
    }
    std::ptr::null_mut()
}

// ---- Images & Global Class Queries -----------------------------------------

pub unsafe extern "C" fn class_get_image_name(cls: *const ClassT) -> *const c_char {
    let Some(c) = class(cls) else {
        return std::ptr::null();
    };
    let Some(ref img) = c.image else {
        return std::ptr::null();
    };
    // `Arc<Class>` lives in the global registry for the life of the process.
    let ptr = img.as_ptr();
    std::mem::forget(c);
    ptr
}

pub unsafe extern "C" fn objc_copy_image_names(out_count: *mut u32) -> *mut *const c_char {
    let guard = image_names().lock().unwrap();
    let ptrs: Vec<*const c_char> = guard.iter().map(|s| s.as_ptr()).collect();
    alloc_null_terminated(&ptrs, out_count)
}

pub unsafe extern "C" fn objc_copy_class_names_for_image(
    image: *const c_char,
    out_count: *mut u32,
) -> *mut *const c_char {
    if image.is_null() {
        return alloc_null_terminated::<*const c_char>(&[], out_count);
    }
    let target = CStr::from_ptr(image).to_bytes();
    let mut ptrs = Vec::new();
    for c in all_classes() {
        if let Some(ref img) = c.image {
            if img.to_bytes() == target {
                let cname = Box::leak(CString::new(c.name.as_str()).unwrap_or_default().into_boxed_c_str());
                ptrs.push(cname.as_ptr());
            }
        }
    }
    alloc_null_terminated(&ptrs, out_count)
}

pub unsafe extern "C" fn objc_copy_class_list(out_count: *mut u32) -> *mut *const ClassT {
    let list: Vec<*const ClassT> = all_classes().into_iter().map(|c| c.ptr).collect();
    alloc_null_terminated(&list, out_count)
}

pub unsafe extern "C" fn objc_get_class_list(
    buffer: *mut *const ClassT,
    buffer_count: i32,
) -> i32 {
    let list = all_classes();
    if !buffer.is_null() && buffer_count > 0 {
        for (i, c) in list.iter().take(buffer_count as usize).enumerate() {
            *buffer.add(i) = c.ptr;
        }
    }
    list.len() as i32
}

pub unsafe extern "C" fn objc_get_meta_class(name: *const c_char) -> *const ClassT {
    if name.is_null() {
        return std::ptr::null();
    }
    let n = CStr::from_ptr(name).to_string_lossy();
    class_named(&n).map_or(std::ptr::null(), |c| (*c).isa)
}

pub unsafe extern "C" fn objc_get_required_class(name: *const c_char) -> *const ClassT {
    let c = if name.is_null() {
        std::ptr::null()
    } else {
        class_named(&CStr::from_ptr(name).to_string_lossy()).unwrap_or(std::ptr::null())
    };
    if c.is_null() {
        let n = if name.is_null() { "(null)".into() } else { CStr::from_ptr(name).to_string_lossy() };
        eprintln!("[pear-libobjc] objc_getRequiredClass({n}) failed");
        std::process::abort();
    }
    c
}

// ---- Selectors & Objects ---------------------------------------------------

pub extern "C" fn sel_is_equal(lhs: Sel, rhs: Sel) -> bool {
    lhs == rhs
}

pub unsafe extern "C" fn object_get_class_name(obj: Id) -> *const c_char {
    if obj.is_null() {
        return c"nil".as_ptr();
    }
    class_get_name(object_class(obj))
}

pub unsafe extern "C" fn object_is_class(obj: Id) -> bool {
    !obj.is_null() && class(obj as *const ClassT).is_some()
}

pub unsafe extern "C" fn object_get_ivar(obj: Id, iv: IvarHandle) -> Id {
    if obj.is_null() || iv.is_null() {
        return std::ptr::null_mut();
    }
    *((obj as *const u8).offset((*iv).offset()) as *const Id)
}

pub unsafe extern "C" fn object_set_ivar(obj: Id, iv: IvarHandle, val: Id) {
    if obj.is_null() || iv.is_null() {
        return;
    }
    *((obj as *mut u8).offset((*iv).offset()) as *mut Id) = val;
}

pub unsafe extern "C" fn object_get_instance_variable(
    obj: Id,
    name: *const c_char,
    out_value: *mut *mut c_void,
) -> IvarHandle {
    if obj.is_null() || name.is_null() {
        if !out_value.is_null() {
            *out_value = std::ptr::null_mut();
        }
        return std::ptr::null();
    }
    let iv = class_get_instance_variable(object_class(obj), name);
    if !iv.is_null() && !out_value.is_null() {
        *out_value = object_get_ivar(obj, iv) as *mut c_void;
    } else if !out_value.is_null() {
        *out_value = std::ptr::null_mut();
    }
    iv
}

pub unsafe extern "C" fn object_get_indexed_ivars(obj: Id) -> *mut c_void {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let sz = class_get_instance_size(object_class(obj));
    (obj as *mut u8).add(sz) as *mut c_void
}

pub unsafe extern "C" fn object_set_class(obj: Id, cls: *const ClassT) -> *const ClassT {
    if obj.is_null() || cls.is_null() {
        return std::ptr::null();
    }
    let old = *(obj as *const *const ClassT);
    *(obj as *mut *const ClassT) = cls;
    old
}

pub unsafe extern "C" fn class_create_instance(cls: *const ClassT, extra_bytes: usize) -> Id {
    if cls.is_null() {
        return std::ptr::null_mut();
    }
    runtime::ensure_initialized(cls);
    let sz = class_get_instance_size(cls).max(8) + extra_bytes;
    let obj = calloc(1, sz);
    if !obj.is_null() {
        *(obj as *mut *const ClassT) = cls;
        crate::object::track_allocated(obj);
    }
    obj
}

pub unsafe extern "C" fn objc_construct_instance(cls: *const ClassT, bytes: *mut c_void) -> Id {
    if cls.is_null() || bytes.is_null() {
        return std::ptr::null_mut();
    }
    runtime::ensure_initialized(cls);
    let sz = class_get_instance_size(cls).max(8);
    std::ptr::write_bytes(bytes as *mut u8, 0, sz);
    *(bytes as *mut *const ClassT) = cls;
    bytes
}

pub unsafe extern "C" fn objc_destruct_instance(obj: Id) -> *mut c_void {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let destruct = sel(".cxx_destruct");
    let mut c = object_class(obj);
    while !c.is_null() {
        if let Some(imp) = runtime::own_method(c, destruct) {
            let f: extern "C" fn(Id, Sel) = std::mem::transmute(imp);
            f(obj, destruct);
        }
        c = superclass(c);
    }
    crate::props::objc_remove_associated_objects(obj);
    obj
}

pub unsafe extern "C" fn objc_alloc_with_zone(cls: *const ClassT) -> Id {
    alloc(cls)
}

pub extern "C" fn objc_root_autorelease(obj: Id) -> Id {
    crate::object::autorelease_default(obj)
}

pub unsafe extern "C" fn object_dispose_fn(obj: Id) -> Id {
    if !obj.is_null() {
        dispose(obj);
    }
    std::ptr::null_mut()
}

// ---- Dynamic classes -------------------------------------------------------

pub unsafe extern "C" fn objc_allocate_class_pair(
    sup: *const ClassT,
    name: *const c_char,
    extra_bytes: usize,
) -> *const ClassT {
    if name.is_null() {
        return std::ptr::null();
    }
    let n = CStr::from_ptr(name).to_string_lossy();
    if class_named(&n).is_some() {
        return std::ptr::null();
    }
    let sup_meta = if sup.is_null() { NSOBJECT_META.ptr() } else { (*sup).isa };
    let sup_size = if sup.is_null() { 8 } else { class_get_instance_size(sup).max(8) };
    let meta = Box::leak(Box::new(ClassT {
        isa: NSOBJECT_META.ptr(),
        superclass: sup_meta,
        cache: &raw const EMPTY_CACHE as *const c_void,
        vtable: std::ptr::null(),
        bits: 0,
    })) as *const ClassT;
    let cls = Box::leak(Box::new(ClassT {
        isa: meta,
        superclass: sup,
        cache: &raw const EMPTY_CACHE as *const c_void,
        vtable: std::ptr::null(),
        bits: 0,
    })) as *const ClassT;
    runtime::register_dynamic(cls, meta, &n, sup_size + extra_bytes);
    cls
}

pub unsafe extern "C" fn class_add_ivar(
    cls: *const ClassT,
    name: *const c_char,
    size: usize,
    alignment: u8,
    types: *const c_char,
) -> bool {
    if cls.is_null() || name.is_null() {
        return false;
    }
    let Some(c) = class(cls) else {
        return false;
    };
    if !c.building.load(Ordering::Acquire) {
        return false;
    }
    let align = 1usize << (alignment as usize).min(12);
    let cur = c.instance_size.load(Ordering::Acquire);
    let offset = (cur + align - 1) & !(align - 1);
    c.instance_size.store(offset + size, Ordering::Release);
    let entry = Box::leak(Box::new(IvarEntry {
        name: strdup_bytes(CStr::from_ptr(name).to_bytes()),
        types: if types.is_null() { std::ptr::null() } else { strdup_bytes(CStr::from_ptr(types).to_bytes()) },
        offset_var: std::ptr::null_mut(),
        offset: offset as i32,
        size: size as u32,
    }));
    c.ivars.write().unwrap().push(entry);
    true
}

pub unsafe extern "C" fn objc_register_class_pair(cls: *const ClassT) {
    if !cls.is_null() {
        runtime::publish(cls);
        flush_cache();
    }
}

pub unsafe extern "C" fn objc_initialize_class_pair(
    sup: *const ClassT,
    name: *const c_char,
    cls: *mut ClassT,
    meta: *mut ClassT,
) -> *const ClassT {
    if name.is_null() || cls.is_null() || meta.is_null() {
        return std::ptr::null();
    }
    let n = CStr::from_ptr(name).to_string_lossy();
    if class_named(&n).is_some() {
        return std::ptr::null();
    }
    let sup_meta = if sup.is_null() { NSOBJECT_META.ptr() } else { (*sup).isa };
    let sup_size = if sup.is_null() { 8 } else { class_get_instance_size(sup).max(8) };
    (*meta).isa = NSOBJECT_META.ptr();
    (*meta).superclass = sup_meta;
    (*meta).cache = &raw const EMPTY_CACHE as *const c_void;
    (*cls).isa = meta;
    (*cls).superclass = sup;
    (*cls).cache = &raw const EMPTY_CACHE as *const c_void;
    runtime::register_dynamic(cls, meta, &n, sup_size);
    cls
}

pub unsafe extern "C" fn class_set_superclass(
    cls: *mut ClassT,
    new_super: *const ClassT,
) -> *const ClassT {
    if cls.is_null() {
        return std::ptr::null();
    }
    let old = (*cls).superclass;
    (*cls).superclass = new_super;
    flush_cache();
    old
}

pub unsafe extern "C" fn class_get_ivar_layout(cls: *const ClassT) -> *const u8 {
    let ro_ptr = runtime::ro(cls);
    if ro_ptr.is_null() {
        std::ptr::null()
    } else {
        (*ro_ptr).ivar_layout
    }
}
