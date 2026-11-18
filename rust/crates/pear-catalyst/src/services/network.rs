//! Makes an app's network traffic look like it came from an iPhone.
//!
//! A Mac Catalyst process sends a "Macintosh" User-Agent and may negotiate
//! HTTP/3; some backends reject that. Outgoing requests get an iOS-style
//! product token ("MyApp/1.0 (iPhone; iOS 16.0; Scale/3.00)") plus
//! X-Device-Model / X-OS-Version, and assumesHTTP3Capable reads NO. Covered at
//! the NSMutableURLRequest header setters, NSURLSessionConfiguration's
//! additional headers and the NSURLSession task factories.
//! PEAR_NO_NETWORK_SHIM=1 opts out; PEAR_NETWORK_KEEP_UA=1 keeps an
//! app-supplied non-Mac User-Agent. It defeats no protection mechanism.

use crate::device::profile::{self, Status};
use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsstring, objc_retain, sel, Class, Id, Imp, Sel, NIL};
use std::collections::HashSet;
use std::ffi::{c_void, CStr};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

static KEEP_UA: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);

fn debug() -> bool {
    rt::env_set("PEAR_DEBUG_NETWORK")
}

fn profile_ready() -> bool {
    profile::get().status == Status::Ready
}

fn cstring(p: *const std::ffi::c_char) -> Option<String> {
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() })
}

/// Logs the request line and headers (never the body).
unsafe fn log_request(phase: &str, request: Id) {
    if !debug() || request.is_null() {
        return;
    }
    let method: Id = msg0(request, "HTTPMethod");
    let url: Id = msg0(msg0::<Id>(request, "URL"), "absoluteString");
    let headers: Id = msg0(request, "allHTTPHeaderFields");
    rt::log(&format!(
        "[PearRuntime][network] {phase} {} {} headers={}",
        if method.is_null() { "GET".into() } else { rt::ns_string(method) },
        if url.is_null() { "(null)".into() } else { rt::ns_string(url) },
        if headers.is_null() { "{\n}".into() } else { rt::description(headers) }
    ));
}

/// iOS-style User-Agent (retained NSString), or None for an invalid managed profile.
fn user_agent() -> Option<Id> {
    static UA: OnceLock<usize> = OnceLock::new();
    let ua = *UA.get_or_init(|| unsafe {
        let p = profile::get();
        let Some(version) = cstring(p.system_version) else { return 0 };
        let device = if cstring(p.idiom).as_deref() == Some("pad") { "iPad" } else { "iPhone" };
        let info: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "infoDictionary");
        let get = |k: &str| -> Option<String> {
            if info.is_null() {
                return None;
            }
            let v: Id = msg1(info, "objectForKey:", nsstring(k));
            (!v.is_null()).then(|| rt::ns_string(v))
        };
        let name = get("CFBundleExecutable").or_else(|| get("CFBundleName")).unwrap_or_else(|| "App".into()).replace(' ', "");
        let short = get("CFBundleShortVersionString").unwrap_or_else(|| "1.0".into());
        objc_retain(nsstring(&format!("{name}/{short} ({device}; iOS {version}; Scale/3.00)"))) as usize
    });
    (ua != 0).then_some(ua as Id)
}

fn looks_like_mac(agent: &str) -> bool {
    let l = agent.to_lowercase();
    l.is_empty() || l.contains("macintosh") || l.contains("mac os x") || l.contains("darwin")
}

/// The iOS headers merged into `headers`, keeping everything the caller set
/// except a macOS User-Agent.
unsafe fn merged_headers(headers: Id) -> Id {
    let merged: Id = if !headers.is_null() && pear_objc::is_kind_of(headers, "NSDictionary") {
        rt::autoreleased(msg0(headers, "mutableCopy"))
    } else {
        msg0(cls("NSMutableDictionary"), "dictionary")
    };
    if !profile_ready() {
        return merged;
    }
    let existing: Id = msg1(merged, "objectForKey:", nsstring("User-Agent"));
    let existing = if existing.is_null() { String::new() } else { rt::ns_string(existing) };
    if !KEEP_UA.load(Ordering::Relaxed) || looks_like_mac(&existing) {
        if let Some(ua) = user_agent() {
            msg2::<Id, Id, ()>(merged, "setObject:forKey:", ua, nsstring("User-Agent"));
        }
    }
    let p = profile::get();
    for (key, value) in [("X-Device-Model", p.model), ("X-OS-Version", p.system_version)] {
        if let Some(v) = cstring(value) {
            if msg1::<Id, Id>(merged, "objectForKey:", nsstring(key)).is_null() {
                msg2::<Id, Id, ()>(merged, "setObject:forKey:", nsstring(&v), nsstring(key));
            }
        }
    }
    merged
}

unsafe fn apply_headers(request: Id) {
    if request.is_null() {
        return;
    }
    let headers = merged_headers(msg0(request, "allHTTPHeaderFields"));
    let keys: Id = msg0(headers, "allKeys");
    for i in 0..pear_objc::nsarray_count(keys) {
        let field = pear_objc::nsarray_object_at(keys, i);
        msg2::<Id, Id, ()>(request, "setValue:forHTTPHeaderField:", msg1::<Id, Id>(headers, "objectForKey:", field), field);
    }
}

/// Each Method is patched once even if several classes share it.
fn first_patch(m: rt::Method) -> bool {
    static PATCHED: Mutex<Option<HashSet<usize>>> = Mutex::new(None);
    !m.is_null() && PATCHED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(m as usize)
}

unsafe fn swap(c: Class, selector: &str, imp: Imp, slot: &AtomicPtr<c_void>) {
    if c.is_null() {
        return;
    }
    let m = rt::class_getInstanceMethod(c, sel(selector));
    if !first_patch(m) {
        return;
    }
    slot.store(rt::method_getImplementation(m) as *mut c_void, Ordering::Release);
    rt::method_setImplementation(m, imp);
}

static SET_VALUE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SET_ALL: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static HEADERS_GET: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static HEADERS_SET: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static DATA_TASK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static DATA_TASK_H: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static DOWNLOAD_TASK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static DOWNLOAD_TASK_H: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static UPLOAD_DATA: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static UPLOAD_FILE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "C" fn set_value(s: Id, c: Sel, value: Id, field: Id) {
    let mut value = value;
    if !field.is_null()
        && msg1::<Id, isize>(field, "caseInsensitiveCompare:", nsstring("User-Agent")) == 0
        && !KEEP_UA.load(Ordering::Relaxed)
    {
        if let Some(ua) = user_agent() {
            value = ua;
        }
    }
    let p = SET_VALUE.load(Ordering::Acquire);
    if !p.is_null() {
        let f: unsafe extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(p);
        f(s, c, value, field);
    }
}

unsafe extern "C" fn set_all_headers(s: Id, c: Sel, headers: Id) {
    let p = SET_ALL.load(Ordering::Acquire);
    if !p.is_null() {
        let f: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(p);
        f(s, c, merged_headers(headers));
    }
}

unsafe extern "C" fn headers_get(s: Id, c: Sel) -> Id {
    let p = HEADERS_GET.load(Ordering::Acquire);
    let original = if p.is_null() {
        NIL
    } else {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(p);
        f(s, c)
    };
    merged_headers(original)
}

unsafe extern "C" fn headers_set(s: Id, c: Sel, headers: Id) {
    let p = HEADERS_SET.load(Ordering::Acquire);
    if !p.is_null() {
        let f: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(p);
        f(s, c, merged_headers(headers));
    }
}

unsafe extern "C" fn http3_capable(_s: Id, _c: Sel) -> bool {
    false
}
unsafe extern "C" fn set_http3_capable(_s: Id, _c: Sel, _v: bool) {}

unsafe fn rewritten(request: Id) -> Id {
    let mut r: Id = if request.is_null() { NIL } else { rt::autoreleased(msg0(request, "mutableCopy")) };
    if r.is_null() {
        let url: Id = if request.is_null() { NIL } else { msg0(request, "URL") };
        r = msg1(cls("NSMutableURLRequest"), "requestWithURL:", url);
    }
    apply_headers(r);
    log_request("outgoing", r);
    r
}

unsafe fn task1(slot: &AtomicPtr<c_void>, s: Id, c: Sel, request: Id) -> Id {
    let p = slot.load(Ordering::Acquire);
    if p.is_null() {
        return NIL;
    }
    let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(p);
    f(s, c, rewritten(request))
}

unsafe fn task2(slot: &AtomicPtr<c_void>, s: Id, c: Sel, request: Id, extra: Id) -> Id {
    let p = slot.load(Ordering::Acquire);
    if p.is_null() {
        return NIL;
    }
    let f: unsafe extern "C" fn(Id, Sel, Id, Id) -> Id = std::mem::transmute(p);
    f(s, c, rewritten(request), extra)
}

unsafe extern "C" fn data_task(s: Id, c: Sel, r: Id) -> Id { task1(&DATA_TASK, s, c, r) }
unsafe extern "C" fn data_task_h(s: Id, c: Sel, r: Id, h: Id) -> Id { task2(&DATA_TASK_H, s, c, r, h) }
unsafe extern "C" fn download_task(s: Id, c: Sel, r: Id) -> Id { task1(&DOWNLOAD_TASK, s, c, r) }
unsafe extern "C" fn download_task_h(s: Id, c: Sel, r: Id, h: Id) -> Id { task2(&DOWNLOAD_TASK_H, s, c, r, h) }
unsafe extern "C" fn upload_data(s: Id, c: Sel, r: Id, d: Id) -> Id { task2(&UPLOAD_DATA, s, c, r, d) }
unsafe extern "C" fn upload_file(s: Id, c: Sel, r: Id, f: Id) -> Id { task2(&UPLOAD_FILE, s, c, r, f) }

unsafe fn patch_configuration_class(c: Class) {
    if c.is_null() {
        return;
    }
    if profile_ready() {
        swap(c, "HTTPAdditionalHeaders", headers_get as Imp, &HEADERS_GET);
        swap(c, "setHTTPAdditionalHeaders:", headers_set as Imp, &HEADERS_SET);
    }
    // Independent of the profile: no QUIC for new connections.
    let getter = rt::class_getInstanceMethod(c, sel("assumesHTTP3Capable"));
    if first_patch(getter) {
        rt::method_setImplementation(getter, http3_capable as Imp);
    }
    let setter = rt::class_getInstanceMethod(c, sel("setAssumesHTTP3Capable:"));
    if first_patch(setter) {
        rt::method_setImplementation(setter, set_http3_capable as Imp);
    }
}

unsafe fn patch_session_class(c: Class) {
    if c.is_null() || !profile_ready() {
        return;
    }
    swap(c, "dataTaskWithRequest:", data_task as Imp, &DATA_TASK);
    swap(c, "dataTaskWithRequest:completionHandler:", data_task_h as Imp, &DATA_TASK_H);
    swap(c, "downloadTaskWithRequest:", download_task as Imp, &DOWNLOAD_TASK);
    swap(c, "downloadTaskWithRequest:completionHandler:", download_task_h as Imp, &DOWNLOAD_TASK_H);
    swap(c, "uploadTaskWithRequest:fromData:", upload_data as Imp, &UPLOAD_DATA);
    swap(c, "uploadTaskWithRequest:fromFile:", upload_file as Imp, &UPLOAD_FILE);
}

unsafe fn concrete(obj: Id, fallback: &str) -> Class {
    if obj.is_null() { cls(fallback) } else { rt::object_getClass(obj) }
}

pub fn init() {
    if !rt::module_enabled("services") || rt::env_set("PEAR_NO_NETWORK_SHIM") || INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    KEEP_UA.store(rt::env_set("PEAR_NETWORK_KEEP_UA"), Ordering::Relaxed);
    unsafe {
        pear_objc::autorelease_pool(|| {
            // Concrete classes: every request, configuration and the shared session.
            let ready = profile_ready();
            if ready {
                let url: Id = msg1(cls("NSURL"), "URLWithString:", nsstring("https://example.com"));
                let sample: Id = msg1(cls("NSMutableURLRequest"), "requestWithURL:", url);
                let rc = concrete(sample, "NSMutableURLRequest");
                swap(rc, "setValue:forHTTPHeaderField:", set_value as Imp, &SET_VALUE);
                swap(rc, "setAllHTTPHeaderFields:", set_all_headers as Imp, &SET_ALL);
            }
            let config = cls("NSURLSessionConfiguration");
            patch_configuration_class(concrete(msg0(config, "defaultSessionConfiguration"), "NSURLSessionConfiguration"));
            let ephemeral: Id = msg0(config, "ephemeralSessionConfiguration");
            if !ephemeral.is_null() {
                patch_configuration_class(rt::object_getClass(ephemeral));
            }
            if ready {
                patch_session_class(concrete(msg0(cls("NSURLSession"), "sharedSession"), "NSURLSession"));
            }
        });
        let ua = user_agent().map(|u| rt::ns_string(u)).unwrap_or_else(|| "(null)".into());
        rt::log(&format!("[PearRuntime][network] shim installed (UA={ua}, HTTP/3 off)"));
    }
}
