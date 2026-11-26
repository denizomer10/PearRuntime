//! Clean-room CoreFoundation & Foundation C API extensions (`CFStream`,
//! `CFRunLoop`, `CFSocket`, `CFString`, `CFPreferences`, `CFError`, and
//! exported Darwin NS/CF string constants).

use super::{
    alloc_with_backing, cf_array_create, cf_array_get_count, cf_array_get_value_at_index,
    cf_dictionary_create_mutable, cf_dictionary_get_value, cf_dictionary_set_value,
    cf_release, cf_string_get_length, const_nsstr_ptr, nsarray_from_ids, nsstring_from_rust,
    rust_str_from_nsstring, store, Backing, K_CF_ALLOCATOR_DEFAULT, NSDATA_CLASS,
    NSMUTABLEARRAY_CLASS, NSMUTABLEDATA_CLASS,
};
use pear_libobjc::abi::Id;
use pear_libobjc::runtime;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{LazyLock, Mutex};

static CF_FOUNDATION_VERSION: f64 = 1953.1;

#[derive(Default)]
struct StreamState {
    buf: Vec<u8>,
    pos: usize,
    status: isize,
    props: HashMap<String, usize>,
}

static STREAMS: LazyLock<Mutex<HashMap<usize, StreamState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static SOCKETS: LazyLock<Mutex<HashMap<usize, i32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static UNCAUGHT_HANDLER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// ---- CFAllocator / CFAutorelease / CFCollection ----------------------------

extern "C" fn cf_allocator_get_default() -> *const c_void {
    &raw const K_CF_ALLOCATOR_DEFAULT as *const c_void
}

extern "C" fn cf_autorelease(obj: Id) -> Id {
    if !obj.is_null() {
        pear_libobjc::object::autorelease(obj)
    } else {
        obj
    }
}

extern "C" fn cf_make_collectable(obj: Id) -> Id {
    obj
}

extern "C" fn cf_array_create_mutable(_alloc: *const c_void, _cap: isize, _cb: *const c_void) -> Id {
    unsafe { alloc_with_backing(NSMUTABLEARRAY_CLASS.ptr(), Backing::Arr(Vec::new())) }
}

extern "C" fn cf_array_append_value(arr: Id, val: *const c_void) {
    if arr.is_null() {
        return;
    }
    if let Some(Backing::Arr(v)) = store().lock().unwrap().get_mut(&(arr as usize)) {
        v.push(val as usize);
    }
}

extern "C" fn cf_data_create_mutable(_alloc: *const c_void, _cap: isize) -> Id {
    unsafe { alloc_with_backing(NSMUTABLEDATA_CLASS.ptr(), Backing::Bytes(Vec::new())) }
}

unsafe extern "C" fn cf_data_get_bytes(data: Id, range_loc: isize, range_len: isize, buf: *mut u8) {
    if data.is_null() || buf.is_null() || range_len <= 0 || range_loc < 0 {
        return;
    }
    if let Some(Backing::Bytes(b)) = store().lock().unwrap().get(&(data as usize)) {
        let start = (range_loc as usize).min(b.len());
        let end = start.saturating_add(range_len as usize).min(b.len());
        let len = end - start;
        if len > 0 {
            std::ptr::copy_nonoverlapping(b[start..end].as_ptr(), buf, len);
        }
    }
}

extern "C" fn cf_data_set_length(data: Id, new_len: isize) {
    if data.is_null() || new_len < 0 {
        return;
    }
    if let Some(Backing::Bytes(b)) = store().lock().unwrap().get_mut(&(data as usize)) {
        b.resize(new_len as usize, 0);
    }
}

extern "C" fn cf_data_get_type_id() -> usize {
    NSDATA_CLASS.ptr() as usize
}

extern "C" fn cf_dict_add_value(dict: Id, key: Id, val: *const c_void) {
    if dict.is_null() {
        return;
    }
    let key_str = rust_str_from_nsstring(key).unwrap_or_else(|| format!("{:#x}", key as usize));
    if let Some(Backing::Dict(m)) = store().lock().unwrap().get_mut(&(dict as usize)) {
        m.entry(key_str).or_insert(val as usize);
    }
}

unsafe extern "C" fn cf_dict_apply_fn(
    dict: Id,
    applier: extern "C" fn(*const c_void, *const c_void, *mut c_void),
    ctx: *mut c_void,
) {
    let entries: Vec<(String, usize)> = if let Some(Backing::Dict(m)) = store().lock().unwrap().get(&(dict as usize)) {
        m.iter().map(|(k, &v)| (k.clone(), v)).collect()
    } else {
        return;
    };
    for (k, v) in entries {
        let k_obj = nsstring_from_rust(&k);
        applier(k_obj as *const c_void, v as *const c_void, ctx);
    }
}

extern "C" fn cf_dict_contains_key(dict: Id, key: Id) -> bool {
    !cf_dictionary_get_value(dict, key).is_null()
}

extern "C" fn cf_dict_get_count(dict: Id) -> isize {
    if dict.is_null() {
        return 0;
    }
    if let Some(Backing::Dict(m)) = store().lock().unwrap().get(&(dict as usize)) {
        return m.len() as isize;
    }
    0
}

extern "C" fn cf_number_get_type(num: Id) -> isize {
    if num.is_null() {
        return 3; // kCFNumberSInt32Type
    }
    match store().lock().unwrap().get(&(num as usize)) {
        Some(Backing::NumF64(_)) => 13, // kCFNumberFloat64Type
        _ => 4,                         // kCFNumberSInt64Type
    }
}

extern "C" fn cf_error_copy_user_info(err: Id) -> Id {
    if err.is_null() {
        return unsafe { cf_dictionary_create_mutable(std::ptr::null(), 0, std::ptr::null(), std::ptr::null()) };
    }
    let d = unsafe { cf_dictionary_create_mutable(std::ptr::null(), 0, std::ptr::null(), std::ptr::null()) };
    let msg = rust_str_from_nsstring(err).unwrap_or_else(|| "Error".into());
    unsafe {
        cf_dictionary_set_value(
            d,
            nsstring_from_rust("NSLocalizedDescription"),
            nsstring_from_rust(&msg),
        );
    }
    d
}

extern "C" fn cf_error_get_code(err: Id) -> isize {
    if err.is_null() { 0 } else { -1 }
}

extern "C" fn cf_error_get_domain(_err: Id) -> Id {
    nsstring_from_rust("NSCocoaErrorDomain")
}

extern "C" fn cf_bundle_get_identifier(_bundle: Id) -> Id {
    nsstring_from_rust("com.pearruntime.app")
}

extern "C" fn cf_locale_get_identifier(_locale: Id) -> Id {
    nsstring_from_rust("en_US")
}

extern "C" fn cf_notif_noop(_center: *mut c_void, _obs: *const c_void, _name: Id, _obj: *const c_void) {}
extern "C" fn cf_notif_remove_every(_center: *mut c_void, _obs: *const c_void) {}
extern "C" fn cf_prefs_remove_suite(_app_id: Id, _suite_name: Id) {}

// ---- CFString / CFURL extensions -------------------------------------------

unsafe extern "C" fn cf_string_create_with_chars(_alloc: *const c_void, chars: *const u16, num: isize) -> Id {
    if chars.is_null() || num <= 0 {
        return nsstring_from_rust("");
    }
    let units = std::slice::from_raw_parts(chars, num as usize);
    nsstring_from_rust(&String::from_utf16_lossy(units))
}

unsafe extern "C" fn cf_string_create_with_format_and_args(
    _alloc: *const c_void,
    _opts: *const c_void,
    fmt: Id,
    _args: *mut c_void,
) -> Id {
    nsstring_from_rust(&rust_str_from_nsstring(fmt).unwrap_or_default())
}

extern "C" fn cf_string_transform(s: Id, _range: isize, _range_len: isize, _transform: Id, _reverse: bool) -> bool {
    !s.is_null()
}

extern "C" fn cf_string_trim_whitespace(s: Id) {
    if s.is_null() {
        return;
    }
    if let Some(Backing::Str(st)) = store().lock().unwrap().get_mut(&(s as usize)) {
        *st = st.trim().to_string();
    }
}

extern "C" fn cf_url_replace_percent_escapes(_alloc: *const c_void, orig: Id, _chars_to_leave: Id) -> Id {
    let s = rust_str_from_nsstring(orig).unwrap_or_default();
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    nsstring_from_rust(&String::from_utf8_lossy(&out))
}

unsafe extern "C" fn cf_url_get_fs_rep(url: Id, _resolve_against_base: bool, buf: *mut u8, max_len: isize) -> bool {
    if url.is_null() || buf.is_null() || max_len <= 0 {
        return false;
    }
    let s = rust_str_from_nsstring(url).unwrap_or_default();
    let path = s.strip_prefix("file://").unwrap_or(&s);
    let b = path.as_bytes();
    let n = b.len().min((max_len as usize).saturating_sub(1));
    std::ptr::copy_nonoverlapping(b.as_ptr(), buf, n);
    *buf.add(n) = 0;
    true
}

// ---- CFRunLoop / CFSocket / CFStream ---------------------------------------

fn alloc_handle() -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}

extern "C" fn cf_runloop_add_or_remove(_rl: *mut c_void, _item: *mut c_void, _mode: Id) {}
extern "C" fn cf_runloop_observer_create(
    _alloc: *const c_void,
    _activities: usize,
    _repeats: bool,
    _order: isize,
    _cb: *const c_void,
    _ctx: *mut c_void,
) -> *mut c_void {
    alloc_handle()
}
extern "C" fn cf_runloop_source_create(_alloc: *const c_void, _order: isize, _ctx: *mut c_void) -> *mut c_void {
    alloc_handle()
}
extern "C" fn cf_runloop_source_signal(_src: *mut c_void) {}
extern "C" fn cf_runloop_timer_create(
    _alloc: *const c_void,
    _fire_date: f64,
    _interval: f64,
    _flags: usize,
    _order: isize,
    _cb: *const c_void,
    _ctx: *mut c_void,
) -> *mut c_void {
    alloc_handle()
}
extern "C" fn cf_runloop_timer_set_next_fire_date(_timer: *mut c_void, _date: f64) {}
extern "C" fn cf_runloop_timer_set_tolerance(_timer: *mut c_void, _tol: f64) {}
extern "C" fn cf_runloop_run() {}
extern "C" fn cf_runloop_run_in_mode(_mode: Id, _seconds: f64, _return_after: bool) -> i32 {
    2 // kCFRunLoopRunTimedOut
}
extern "C" fn cf_runloop_stop(_rl: *mut c_void) {}
extern "C" fn cf_runloop_wake_up(_rl: *mut c_void) {}

extern "C" fn cf_socket_create_with_native(
    _alloc: *const c_void,
    sock: i32,
    _callout_types: usize,
    _callout: *const c_void,
    _ctx: *const c_void,
) -> *mut c_void {
    let h = alloc_handle();
    SOCKETS.lock().unwrap().insert(h as usize, sock);
    h
}
extern "C" fn cf_socket_create_runloop_source(_alloc: *const c_void, _sock: *mut c_void, _order: isize) -> *mut c_void {
    alloc_handle()
}
extern "C" fn cf_socket_get_native(sock: *mut c_void) -> i32 {
    SOCKETS.lock().unwrap().get(&(sock as usize)).copied().unwrap_or(-1)
}
extern "C" fn cf_socket_get_flags(_sock: *mut c_void) -> usize {
    0
}
extern "C" fn cf_socket_invalidate(sock: *mut c_void) {
    SOCKETS.lock().unwrap().remove(&(sock as usize));
}

unsafe extern "C" fn cf_stream_create_bound_pair(
    _alloc: *const c_void,
    read_stream: *mut *mut c_void,
    write_stream: *mut *mut c_void,
    _transfer_buf_size: isize,
) {
    let r = alloc_handle();
    let w = alloc_handle();
    let mut map = STREAMS.lock().unwrap();
    map.insert(r as usize, StreamState::default());
    map.insert(w as usize, StreamState::default());
    if !read_stream.is_null() {
        *read_stream = r;
    }
    if !write_stream.is_null() {
        *write_stream = w;
    }
}

unsafe extern "C" fn cf_stream_create_pair_with_socket(
    _alloc: *const c_void,
    _sock: i32,
    read_stream: *mut *mut c_void,
    write_stream: *mut *mut c_void,
) {
    cf_stream_create_bound_pair(std::ptr::null(), read_stream, write_stream, 4096);
}

extern "C" fn cf_stream_copy_error(_stream: *mut c_void) -> Id {
    std::ptr::null_mut()
}
extern "C" fn cf_stream_set_client(_stream: *mut c_void, _events: usize, _cb: *const c_void, _ctx: *mut c_void) -> bool {
    true
}
extern "C" fn cf_stream_unschedule(_stream: *mut c_void, _rl: *mut c_void, _mode: Id) {}
extern "C" fn cf_write_stream_open(stream: *mut c_void) -> bool {
    let mut map = STREAMS.lock().unwrap();
    map.entry(stream as usize).or_default().status = 2; // kCFStreamStatusOpen
    true
}
extern "C" fn cf_write_stream_close(stream: *mut c_void) {
    if let Some(st) = STREAMS.lock().unwrap().get_mut(&(stream as usize)) {
        st.status = 6; // kCFStreamStatusClosed
    }
}
extern "C" fn cf_write_stream_can_accept(_stream: *mut c_void) -> bool {
    true
}
extern "C" fn cf_write_stream_get_status(stream: *mut c_void) -> isize {
    STREAMS.lock().unwrap().get(&(stream as usize)).map_or(0, |s| s.status)
}
extern "C" fn cf_write_stream_schedule(_stream: *mut c_void, _rl: *mut c_void, _mode: Id) {}
extern "C" fn cf_write_stream_set_prop(stream: *mut c_void, key: Id, val: *const c_void) -> bool {
    let k = rust_str_from_nsstring(key).unwrap_or_default();
    let mut map = STREAMS.lock().unwrap();
    map.entry(stream as usize).or_default().props.insert(k, val as usize);
    true
}
unsafe extern "C" fn cf_write_stream_write(stream: *mut c_void, buf: *const u8, len: isize) -> isize {
    if buf.is_null() || len <= 0 {
        return 0;
    }
    let slice = std::slice::from_raw_parts(buf, len as usize);
    let mut map = STREAMS.lock().unwrap();
    let st = map.entry(stream as usize).or_default();
    st.buf.extend_from_slice(slice);
    len
}

// ---- Foundation functions --------------------------------------------------

unsafe extern "C" fn ns_log_v(fmt: Id, args: *mut c_void) {
    if let Some(s) = rust_str_from_nsstring(fmt) {
        let formatted = super::format_nsstring_slots(&s, args as *const u64);
        eprintln!("[NSLogv] {formatted}");
    }
}

extern "C" fn ns_string_from_protocol(_proto: *const c_void) -> Id {
    nsstring_from_rust("Protocol")
}

unsafe extern "C" fn ns_selector_from_string(s: Id) -> pear_libobjc::abi::Sel {
    let name = rust_str_from_nsstring(s).unwrap_or_default();
    runtime::sel(&name)
}

unsafe extern "C" fn ns_protocol_from_string(s: Id) -> *const c_void {
    let name = rust_str_from_nsstring(s).unwrap_or_default();
    runtime::protocol_named(&name).map_or(std::ptr::null(), |p| p as *const c_void)
}

extern "C" fn ns_get_uncaught_handler() -> usize {
    UNCAUGHT_HANDLER.load(std::sync::atomic::Ordering::Acquire)
}

extern "C" fn ns_set_uncaught_handler(h: usize) {
    UNCAUGHT_HANDLER.store(h, std::sync::atomic::Ordering::Release);
}

#[allow(dead_code)]
fn ensure_unused_imports_used() {
    let _ = (cf_array_create, cf_array_get_count, cf_array_get_value_at_index, cf_release, cf_string_get_length, nsarray_from_ids);
}

pub fn symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        // CFAllocator / Autorelease / Collections
        "_CFAllocatorGetDefault" => f(cf_allocator_get_default as *const ()),
        "_CFAutorelease" => f(cf_autorelease as *const ()),
        "_CFMakeCollectable" => f(cf_make_collectable as *const ()),
        "_CFArrayCreateMutable" => f(cf_array_create_mutable as *const ()),
        "_CFArrayAppendValue" => f(cf_array_append_value as *const ()),
        "_CFDataCreateMutable" => f(cf_data_create_mutable as *const ()),
        "_CFDataGetBytes" => f(cf_data_get_bytes as *const ()),
        "_CFDataSetLength" => f(cf_data_set_length as *const ()),
        "_CFDataGetTypeID" => f(cf_data_get_type_id as *const ()),
        "_CFDictionaryAddValue" => f(cf_dict_add_value as *const ()),
        "_CFDictionaryApplyFunction" => f(cf_dict_apply_fn as *const ()),
        "_CFDictionaryContainsKey" => f(cf_dict_contains_key as *const ()),
        "_CFDictionaryGetCount" => f(cf_dict_get_count as *const ()),
        "_CFNumberGetType" => f(cf_number_get_type as *const ()),
        "_CFErrorCopyUserInfo" => f(cf_error_copy_user_info as *const ()),
        "_CFErrorGetCode" => f(cf_error_get_code as *const ()),
        "_CFErrorGetDomain" => f(cf_error_get_domain as *const ()),
        "_CFBundleGetIdentifier" => f(cf_bundle_get_identifier as *const ()),
        "_CFLocaleGetIdentifier" => f(cf_locale_get_identifier as *const ()),
        "_CFNotificationCenterRemoveObserver" => f(cf_notif_noop as *const ()),
        "_CFNotificationCenterRemoveEveryObserver" => f(cf_notif_remove_every as *const ()),
        "_CFPreferencesRemoveSuitePreferencesFromApp" => f(cf_prefs_remove_suite as *const ()),
        // CFString / CFURL
        "_CFStringCreateWithCharacters" => f(cf_string_create_with_chars as *const ()),
        "_CFStringCreateWithFormatAndArguments" => f(cf_string_create_with_format_and_args as *const ()),
        "_CFStringTransform" => f(cf_string_transform as *const ()),
        "_CFStringTrimWhitespace" => f(cf_string_trim_whitespace as *const ()),
        "_CFURLCreateStringByReplacingPercentEscapes" => f(cf_url_replace_percent_escapes as *const ()),
        "_CFURLGetFileSystemRepresentation" => f(cf_url_get_fs_rep as *const ()),
        // CFRunLoop / CFSocket / CFStream
        "_CFRunLoopAddObserver"
        | "_CFRunLoopRemoveObserver"
        | "_CFRunLoopAddSource"
        | "_CFRunLoopRemoveSource"
        | "_CFRunLoopAddTimer"
        | "_CFRunLoopRemoveTimer" => f(cf_runloop_add_or_remove as *const ()),
        "_CFRunLoopObserverCreate" => f(cf_runloop_observer_create as *const ()),
        "_CFRunLoopSourceCreate" => f(cf_runloop_source_create as *const ()),
        "_CFRunLoopSourceSignal" => f(cf_runloop_source_signal as *const ()),
        "_CFRunLoopTimerCreate" => f(cf_runloop_timer_create as *const ()),
        "_CFRunLoopTimerSetNextFireDate" => f(cf_runloop_timer_set_next_fire_date as *const ()),
        "_CFRunLoopTimerSetTolerance" => f(cf_runloop_timer_set_tolerance as *const ()),
        "_CFRunLoopRun" => f(cf_runloop_run as *const ()),
        "_CFRunLoopRunInMode" => f(cf_runloop_run_in_mode as *const ()),
        "_CFRunLoopStop" => f(cf_runloop_stop as *const ()),
        "_CFRunLoopWakeUp" => f(cf_runloop_wake_up as *const ()),
        "_CFSocketCreateWithNative" => f(cf_socket_create_with_native as *const ()),
        "_CFSocketCreateRunLoopSource" => f(cf_socket_create_runloop_source as *const ()),
        "_CFSocketGetNative" => f(cf_socket_get_native as *const ()),
        "_CFSocketGetSocketFlags" => f(cf_socket_get_flags as *const ()),
        "_CFSocketInvalidate" => f(cf_socket_invalidate as *const ()),
        "_CFStreamCreateBoundPair" => f(cf_stream_create_bound_pair as *const ()),
        "_CFStreamCreatePairWithSocket" => f(cf_stream_create_pair_with_socket as *const ()),
        "_CFReadStreamCopyError" | "_CFWriteStreamCopyError" => f(cf_stream_copy_error as *const ()),
        "_CFReadStreamSetClient" | "_CFWriteStreamSetClient" => f(cf_stream_set_client as *const ()),
        "_CFReadStreamUnscheduleFromRunLoop" => f(cf_stream_unschedule as *const ()),
        "_CFWriteStreamOpen" => f(cf_write_stream_open as *const ()),
        "_CFWriteStreamClose" => f(cf_write_stream_close as *const ()),
        "_CFWriteStreamCanAcceptBytes" => f(cf_write_stream_can_accept as *const ()),
        "_CFWriteStreamGetStatus" => f(cf_write_stream_get_status as *const ()),
        "_CFWriteStreamScheduleWithRunLoop" | "_CFWriteStreamUnscheduleFromRunLoop" => f(cf_write_stream_schedule as *const ()),
        "_CFWriteStreamSetProperty" => f(cf_write_stream_set_prop as *const ()),
        "_CFWriteStreamWrite" => f(cf_write_stream_write as *const ()),
        // CoreFoundation & Foundation globals / constants
        "_kCFAllocatorNull" => Some(&raw const K_CF_ALLOCATOR_DEFAULT as usize),
        "_kCFCoreFoundationVersionNumber" | "_NSFoundationVersionNumber" => {
            Some(&raw const CF_FOUNDATION_VERSION as usize)
        }
        "_kCFBundleExecutableKey" => Some(const_nsstr_ptr("CFBundleExecutable")),
        "_kCFBundleIdentifierKey" => Some(const_nsstr_ptr("CFBundleIdentifier")),
        "_kCFBundleNameKey" => Some(const_nsstr_ptr("CFBundleName")),
        "_kCFLocaleCountryCode" => Some(const_nsstr_ptr("kCFLocaleCountryCode")),
        "_kCFLocaleLanguageCode" => Some(const_nsstr_ptr("kCFLocaleLanguageCode")),
        "_kCFLocaleScriptCode" => Some(const_nsstr_ptr("kCFLocaleScriptCode")),
        "_kCFStreamPropertyShouldCloseNativeSocket" => Some(const_nsstr_ptr("kCFStreamPropertyShouldCloseNativeSocket")),
        "_kCFStreamSocketSecurityLevelNegotiatedSSL" => Some(const_nsstr_ptr("kCFStreamSocketSecurityLevelNegotiatedSSL")),
        // Foundation functions
        "_NSLogv" => f(ns_log_v as *const ()),
        "_NSStringFromProtocol" => f(ns_string_from_protocol as *const ()),
        "_NSSelectorFromString" => f(ns_selector_from_string as *const ()),
        "_NSProtocolFromString" => f(ns_protocol_from_string as *const ()),
        "_NSGetUncaughtExceptionHandler" => f(ns_get_uncaught_handler as *const ()),
        "_NSSetUncaughtExceptionHandler" => f(ns_set_uncaught_handler as *const ()),
        // Foundation string constants
        "_NSBuddhistCalendar" => Some(const_nsstr_ptr("buddhist")),
        "_NSCalendarIdentifierGregorian" => Some(const_nsstr_ptr("gregorian")),
        "_NSChineseCalendar" => Some(const_nsstr_ptr("chinese")),
        "_NSGregorianCalendar" => Some(const_nsstr_ptr("gregorian")),
        "_NSHebrewCalendar" => Some(const_nsstr_ptr("hebrew")),
        "_NSISO8601Calendar" => Some(const_nsstr_ptr("iso8601")),
        "_NSIndianCalendar" => Some(const_nsstr_ptr("indian")),
        "_NSIslamicCalendar" => Some(const_nsstr_ptr("islamic")),
        "_NSIslamicCivilCalendar" => Some(const_nsstr_ptr("islamic-civil")),
        "_NSJapaneseCalendar" => Some(const_nsstr_ptr("japanese")),
        "_NSPersianCalendar" => Some(const_nsstr_ptr("persian")),
        "_NSRepublicOfChinaCalendar" => Some(const_nsstr_ptr("roc")),
        "_NSGenericException" => Some(const_nsstr_ptr("NSGenericException")),
        "_NSInternalInconsistencyException" => Some(const_nsstr_ptr("NSInternalInconsistencyException")),
        "_NSInvalidArgumentException" => Some(const_nsstr_ptr("NSInvalidArgumentException")),
        "_NSLocaleLanguageCode" => Some(const_nsstr_ptr("NSLocaleLanguageCode")),
        "_NSURLAttributeModificationDateKey" => Some(const_nsstr_ptr("NSURLAttributeModificationDateKey")),
        "_NSURLContentAccessDateKey" => Some(const_nsstr_ptr("NSURLContentAccessDateKey")),
        "_NSURLCreationDateKey" => Some(const_nsstr_ptr("NSURLCreationDateKey")),
        "_NSURLFileSizeKey" => Some(const_nsstr_ptr("NSURLFileSizeKey")),
        "_NSURLIsDirectoryKey" => Some(const_nsstr_ptr("NSURLIsDirectoryKey")),
        "_NSURLIsRegularFileKey" => Some(const_nsstr_ptr("NSURLIsRegularFileKey")),
        "_NSURLTotalFileAllocatedSizeKey" => Some(const_nsstr_ptr("NSURLTotalFileAllocatedSizeKey")),
        "_NSCocoaErrorDomain" => Some(const_nsstr_ptr("NSCocoaErrorDomain")),
        "_NSMachErrorDomain" => Some(const_nsstr_ptr("NSMachErrorDomain")),
        "_NSOSStatusErrorDomain" => Some(const_nsstr_ptr("NSOSStatusErrorDomain")),
        "_NSPOSIXErrorDomain" => Some(const_nsstr_ptr("NSPOSIXErrorDomain")),
        "_NSURLErrorDomain" => Some(const_nsstr_ptr("NSURLErrorDomain")),
        "_NSURLErrorFailingURLStringErrorKey" => Some(const_nsstr_ptr("NSErrorFailingURLStringKey")),
        "_NSURLErrorKey" => Some(const_nsstr_ptr("NSErrorFailingURLKey")),
        "_NSUnderlyingErrorKey" => Some(const_nsstr_ptr("NSUnderlyingError")),
        "_NSLocalizedFailureReasonErrorKey" => Some(const_nsstr_ptr("NSLocalizedFailureReason")),
        "_NSLocalizedRecoveryOptionsErrorKey" => Some(const_nsstr_ptr("NSLocalizedRecoveryOptions")),
        "_NSLocalizedRecoverySuggestionErrorKey" => Some(const_nsstr_ptr("NSLocalizedRecoverySuggestion")),
        "_NSRecoveryAttempterErrorKey" => Some(const_nsstr_ptr("NSRecoveryAttempterErrorKey")),
        "_NSFileCreationDate" => Some(const_nsstr_ptr("NSFileCreationDate")),
        "_NSFileImmutable" => Some(const_nsstr_ptr("NSFileImmutable")),
        "_NSFileModificationDate" => Some(const_nsstr_ptr("NSFileModificationDate")),
        "_NSFilePosixPermissions" => Some(const_nsstr_ptr("NSFilePosixPermissions")),
        "_NSFileProtectionKey" => Some(const_nsstr_ptr("NSFileProtectionKey")),
        "_NSFileProtectionNone" => Some(const_nsstr_ptr("NSFileProtectionNone")),
        "_NSFileSize" => Some(const_nsstr_ptr("NSFileSize")),
        "_NSFileSystemFreeSize" => Some(const_nsstr_ptr("NSFileSystemFreeSize")),
        "_NSFileSystemSize" => Some(const_nsstr_ptr("NSFileSystemSize")),
        "_NSKeyValueChangeNewKey" => Some(const_nsstr_ptr("new")),
        "_NSKeyValueChangeOldKey" => Some(const_nsstr_ptr("old")),
        "_NSKeyedArchiveRootObjectKey" => Some(const_nsstr_ptr("root")),
        "_NSUserActivityTypeBrowsingWeb" => Some(const_nsstr_ptr("NSUserActivityTypeBrowsingWeb")),
        _ => None,
    }
}
