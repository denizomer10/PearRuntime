//! macOS Metal compatibility hooks for iOS precompiled Metal shader libraries (`MTLB`).

use pear_objc::{cls, sel, Class, Id, Imp, Sel, NIL};
use std::ffi::{c_char, c_void, CString};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

extern "C" {
    fn class_getInstanceMethod(cls: Class, name: Sel) -> *mut c_void;
    fn method_setImplementation(m: *mut c_void, imp: Imp) -> Imp;
    fn dispatch_data_create_map(
        data: *mut c_void,
        buffer_ptr: *mut *const c_void,
        size_ptr: *mut usize,
    ) -> *mut c_void;
    fn dispatch_data_create(
        buffer: *const c_void,
        size: usize,
        queue: *const c_void,
        destructor: *const c_void,
    ) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
    fn PearHostLogMessage(area: *const c_char, message: *const c_char);
}

type NewLibWithDataFn = unsafe extern "C" fn(Id, Sel, *mut c_void, *mut Id) -> Id;

static ORIG_NEW_LIB: AtomicUsize = AtomicUsize::new(0);
static LOGGED_OK: AtomicBool = AtomicBool::new(false);
static LOGGED_FAIL: AtomicBool = AtomicBool::new(false);

fn log_metal(msg: &str) {
    if let Ok(c) = CString::new(msg.replace('\0', "")) {
        unsafe { PearHostLogMessage(c"metal".as_ptr(), c.as_ptr()) };
    }
}

fn hex_slice(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn tags_preview(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        if (0x20..0x7f).contains(&b) {
            s.push(b as char);
        } else {
            let _ = write!(s, "\\x{b:02x}");
        }
    }
    s
}

fn find_air64_preview(bytes: &[u8]) -> String {
    if let Some(pos) = bytes.windows(5).position(|w| w == b"air64") {
        let end = (pos + 40).min(bytes.len());
        tags_preview(&bytes[pos..end])
    } else {
        "none".to_string()
    }
}

unsafe fn try_patched_mtlb(
    orig: NewLibWithDataFn,
    self_obj: Id,
    cmd: Sel,
    bytes: &[u8],
    error_out: *mut Id,
    patch: fn(&mut [u8]),
) -> Id {
    let mut buf = bytes.to_vec();
    patch(&mut buf);
    let dd = dispatch_data_create(
        buf.as_ptr() as *const c_void,
        buf.len(),
        std::ptr::null(),
        std::ptr::null(),
    );
    if dd.is_null() {
        return NIL;
    }
    if !error_out.is_null() {
        *error_out = NIL;
    }
    let lib = orig(self_obj, cmd, dd, error_out);
    dispatch_release(dd);
    lib
}

unsafe extern "C" fn hook_new_library_with_data(
    self_obj: Id,
    cmd: Sel,
    data: *mut c_void,
    error_out: *mut Id,
) -> Id {
    let raw = ORIG_NEW_LIB.load(Ordering::Acquire);
    if raw == 0 {
        return NIL;
    }
    let orig: NewLibWithDataFn = std::mem::transmute(raw);
    let lib = orig(self_obj, cmd, data, error_out);
    if !lib.is_null() || data.is_null() {
        return lib;
    }

    let mut ptr: *const c_void = std::ptr::null();
    let mut len: usize = 0;
    let map = dispatch_data_create_map(data, &mut ptr, &mut len);
    if map.is_null() {
        return NIL;
    }
    if ptr.is_null() || len < 88 {
        dispatch_release(map);
        return NIL;
    }

    let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
    if &bytes[0..4] != b"MTLB" {
        dispatch_release(map);
        return NIL;
    }

    let candidates: [(&str, fn(&mut [u8])); 3] = [
        ("macos_bit", |b| {
            b[5] |= 0x80;
        }),
        ("macos_bit+os81", |b| {
            b[5] |= 0x80;
            b[11] = 0x81;
        }),
        ("v1.2.6+os81", |b| {
            b[5] |= 0x80;
            if b[8] < 6 {
                b[8] = 6;
            }
            b[11] = 0x81;
            b[12] = 14;
            b[13] = 0;
        }),
    ];

    for (name, patch) in candidates {
        let patched_lib = try_patched_mtlb(orig, self_obj, cmd, bytes, error_out, patch);
        if !patched_lib.is_null() {
            if !error_out.is_null() {
                *error_out = NIL;
            }
            if !LOGGED_OK.swap(true, Ordering::AcqRel) {
                log_metal(&format!("patched iOS MTLB ({len} bytes) succeeded via {name}"));
            }
            dispatch_release(map);
            return patched_lib;
        }
    }

    if !LOGGED_FAIL.swap(true, Ordering::AcqRel) {
        let h0 = hex_slice(&bytes[0..32]);
        let h1 = hex_slice(&bytes[32..88]);
        let tags = tags_preview(&bytes[88..len.min(240)]);
        let air = find_air64_preview(bytes);
        log_metal(&format!(
            "MTLB fallback failed len={len} hdr0..32={h0} hdr32..88={h1} tags=\"{tags}\" air64=\"{air}\""
        ));
    }

    if !error_out.is_null() {
        *error_out = NIL;
    }
    let _ = orig(self_obj, cmd, data, error_out);
    dispatch_release(map);
    NIL
}

pub unsafe fn install() {
    if ORIG_NEW_LIB.load(Ordering::Acquire) != 0 {
        return;
    }
    let dev_cls = cls("_MTLDevice");
    if dev_cls.is_null() {
        return;
    }
    let m = class_getInstanceMethod(dev_cls, sel("newLibraryWithData:error:"));
    if m.is_null() {
        return;
    }
    let prev = method_setImplementation(m, hook_new_library_with_data as Imp);
    ORIG_NEW_LIB.store(prev as usize, Ordering::Release);
}
