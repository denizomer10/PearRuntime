//! Clean-room implementations of `CFNetwork`, `Network`, `CoreMedia`,
//! `CoreVideo`, `AudioToolbox`, `CoreServices`, `VideoToolbox`, `JavaScriptCore`,
//! extended `UIKit` C APIs, and Apple framework constant tables.

use pear_foundation::{
    const_nsstr_ptr, nsarray_from_ids, nsdata_from_bytes, nsdict_from_pairs, nsstring_from_rust,
    rust_bytes_from_nsdata, rust_str_from_nsstring,
};
use pear_libobjc::abi::Id;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::net::ToSocketAddrs;
use std::sync::{LazyLock, Mutex};

// ---- CFNetwork (CFHTTPMessage, CFHost, Proxies) -----------------------------

#[derive(Clone, Default)]
pub struct HttpMessageState {
    is_request: bool,
    method: String,
    url: String,
    version: String,
    status_code: isize,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    raw_buf: Vec<u8>,
    header_complete: bool,
}

static HTTP_MSGS: LazyLock<Mutex<HashMap<usize, HttpMessageState>>> = LazyLock::new(Default::default);
static CF_HOSTS: LazyLock<Mutex<HashMap<usize, String>>> = LazyLock::new(Default::default);

pub extern "C" fn cf_http_msg_create_empty(_alloc: *const c_void, is_request: bool) -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    HTTP_MSGS.lock().unwrap().insert(
        h as usize,
        HttpMessageState {
            is_request,
            method: if is_request { "GET".into() } else { String::new() },
            version: "HTTP/1.1".into(),
            ..Default::default()
        },
    );
    h
}

pub extern "C" fn cf_http_msg_create_request(
    _alloc: *const c_void,
    method: Id,
    url: Id,
    version: Id,
) -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    HTTP_MSGS.lock().unwrap().insert(
        h as usize,
        HttpMessageState {
            is_request: true,
            method: rust_str_from_nsstring(method).unwrap_or_else(|| "GET".into()),
            url: rust_str_from_nsstring(url).unwrap_or_default(),
            version: rust_str_from_nsstring(version).unwrap_or_else(|| "HTTP/1.1".into()),
            header_complete: true,
            ..Default::default()
        },
    );
    h
}

pub extern "C" fn cf_http_msg_create_response(
    _alloc: *const c_void,
    status: isize,
    _desc: Id,
    version: Id,
) -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    HTTP_MSGS.lock().unwrap().insert(
        h as usize,
        HttpMessageState {
            is_request: false,
            status_code: status,
            version: rust_str_from_nsstring(version).unwrap_or_else(|| "HTTP/1.1".into()),
            header_complete: true,
            ..Default::default()
        },
    );
    h
}

pub unsafe extern "C" fn cf_http_msg_append_bytes(msg: *mut c_void, bytes: *const u8, len: isize) -> bool {
    if msg.is_null() || bytes.is_null() || len <= 0 {
        return false;
    }
    let slice = std::slice::from_raw_parts(bytes, len as usize);
    let mut map = HTTP_MSGS.lock().unwrap();
    let Some(st) = map.get_mut(&(msg as usize)) else { return false };
    st.raw_buf.extend_from_slice(slice);
    if !st.header_complete {
        if let Some(pos) = st.raw_buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&st.raw_buf[..pos]).to_string();
            let mut lines = head.lines();
            if let Some(first) = lines.next() {
                let parts: Vec<&str> = first.split_whitespace().collect();
                if st.is_request && parts.len() >= 2 {
                    st.method = parts[0].to_string();
                    st.url = parts[1].to_string();
                    st.version = parts.get(2).copied().unwrap_or("HTTP/1.1").to_string();
                } else if !st.is_request && parts.len() >= 2 {
                    st.version = parts[0].to_string();
                    st.status_code = parts[1].parse().unwrap_or(200);
                }
            }
            for l in lines {
                if let Some((k, v)) = l.split_once(':') {
                    st.headers.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            st.body = st.raw_buf[pos + 4..].to_vec();
            st.header_complete = true;
        }
    } else {
        st.body.extend_from_slice(slice);
    }
    true
}

pub extern "C" fn cf_http_msg_is_header_complete(msg: *mut c_void) -> bool {
    HTTP_MSGS.lock().unwrap().get(&(msg as usize)).map_or(false, |s| s.header_complete)
}
pub extern "C" fn cf_http_msg_get_status(msg: *mut c_void) -> isize {
    HTTP_MSGS.lock().unwrap().get(&(msg as usize)).map_or(0, |s| s.status_code)
}
pub extern "C" fn cf_http_msg_copy_version(msg: *mut c_void) -> Id {
    let v = HTTP_MSGS.lock().unwrap().get(&(msg as usize)).map(|s| s.version.clone()).unwrap_or_else(|| "HTTP/1.1".into());
    nsstring_from_rust(&v)
}
pub extern "C" fn cf_http_msg_copy_method(msg: *mut c_void) -> Id {
    let m = HTTP_MSGS.lock().unwrap().get(&(msg as usize)).map(|s| s.method.clone()).unwrap_or_else(|| "GET".into());
    nsstring_from_rust(&m)
}
pub extern "C" fn cf_http_msg_copy_url(msg: *mut c_void) -> Id {
    let u = HTTP_MSGS.lock().unwrap().get(&(msg as usize)).map(|s| s.url.clone()).unwrap_or_default();
    nsstring_from_rust(&u)
}
pub extern "C" fn cf_http_msg_set_header(msg: *mut c_void, header: Id, val: Id) {
    let k = rust_str_from_nsstring(header).unwrap_or_default();
    let mut map = HTTP_MSGS.lock().unwrap();
    if let Some(st) = map.get_mut(&(msg as usize)) {
        if let Some(v) = rust_str_from_nsstring(val) {
            st.headers.insert(k, v);
        } else {
            st.headers.remove(&k);
        }
    }
}
pub extern "C" fn cf_http_msg_copy_header(msg: *mut c_void, header: Id) -> Id {
    let k = rust_str_from_nsstring(header).unwrap_or_default();
    let val = HTTP_MSGS
        .lock()
        .unwrap()
        .get(&(msg as usize))
        .and_then(|s| s.headers.iter().find(|(hk, _)| hk.eq_ignore_ascii_case(&k)).map(|(_, v)| v.clone()));
    val.map(|v| nsstring_from_rust(&v)).unwrap_or(std::ptr::null_mut())
}
pub extern "C" fn cf_http_msg_copy_all_headers(msg: *mut c_void) -> Id {
    let hdrs = HTTP_MSGS
        .lock()
        .unwrap()
        .get(&(msg as usize))
        .map(|s| s.headers.clone())
        .unwrap_or_default();
    nsdict_from_pairs(hdrs.into_iter().map(|(k, v)| (k, nsstring_from_rust(&v))).collect())
}
pub extern "C" fn cf_http_msg_set_body(msg: *mut c_void, body_data: Id) {
    let bytes = rust_bytes_from_nsdata(body_data).unwrap_or_default();
    if let Some(st) = HTTP_MSGS.lock().unwrap().get_mut(&(msg as usize)) {
        st.body = bytes;
    }
}
pub extern "C" fn cf_http_msg_copy_body(msg: *mut c_void) -> Id {
    let bytes = HTTP_MSGS.lock().unwrap().get(&(msg as usize)).map(|s| s.body.clone()).unwrap_or_default();
    nsdata_from_bytes(&bytes)
}
pub extern "C" fn cf_http_msg_copy_serialized(msg: *mut c_void) -> Id {
    let map = HTTP_MSGS.lock().unwrap();
    let Some(st) = map.get(&(msg as usize)) else {
        return nsdata_from_bytes(&[]);
    };
    let mut raw = if st.is_request {
        format!("{} {} {}\r\n", st.method, st.url, st.version)
    } else {
        format!("{} {}\r\n", st.version, st.status_code)
    };
    for (k, v) in &st.headers {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    raw.push_str("\r\n");
    let mut bytes = raw.into_bytes();
    bytes.extend_from_slice(&st.body);
    nsdata_from_bytes(&bytes)
}

extern "C" fn cf_host_create_with_name(_alloc: *const c_void, name: Id) -> *mut c_void {
    let host = rust_str_from_nsstring(name).unwrap_or_default();
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    CF_HOSTS.lock().unwrap().insert(h as usize, host);
    h
}
extern "C" fn cf_host_get_addressing(host: *mut c_void, resolved: *mut bool) -> Id {
    if !resolved.is_null() {
        unsafe { *resolved = true };
    }
    let name = CF_HOSTS.lock().unwrap().get(&(host as usize)).cloned().unwrap_or_default();
    if name.is_empty() {
        return nsarray_from_ids(Vec::new());
    }
    let addrs: Vec<Id> = (name.as_str(), 0u16)
        .to_socket_addrs()
        .into_iter()
        .flatten()
        .map(|sa| match sa {
            std::net::SocketAddr::V4(v4) => {
                let mut raw = [0u8; 16];
                raw[0] = 16;
                raw[1] = 2; // AF_INET
                raw[4..8].copy_from_slice(&v4.ip().octets());
                nsdata_from_bytes(&raw)
            }
            std::net::SocketAddr::V6(v6) => {
                let mut raw = [0u8; 28];
                raw[0] = 28;
                raw[1] = 30; // AF_INET6
                raw[8..24].copy_from_slice(&v6.ip().octets());
                nsdata_from_bytes(&raw)
            }
        })
        .collect();
    nsarray_from_ids(addrs)
}
extern "C" fn cf_host_noop_rl(_h: *mut c_void, _rl: *mut c_void, _m: Id) {}
extern "C" fn cf_host_set_client(_h: *mut c_void, _cb: *const c_void, _ctx: *mut c_void) -> bool { true }
extern "C" fn cf_host_start_info(_h: *mut c_void, _info: i32, _err: *mut c_void) -> bool { true }
extern "C" fn cf_net_copy_sys_proxy() -> Id { nsdict_from_pairs(Vec::new()) }
extern "C" fn cf_net_copy_proxies_for_url(_url: Id, _settings: Id) -> Id { nsarray_from_ids(Vec::new()) }

static TASK_PRIORITY_DEFAULT: f32 = 0.5;
static TASK_PRIORITY_HIGH: f32 = 1.0;
static TASK_PRIORITY_LOW: f32 = 0.0;

pub fn cfnetwork_ext_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CFHTTPMessageCreateEmpty" => f(cf_http_msg_create_empty as *const ()),
        "_CFHTTPMessageCreateRequest" => f(cf_http_msg_create_request as *const ()),
        "_CFHTTPMessageCreateResponse" => f(cf_http_msg_create_response as *const ()),
        "_CFHTTPMessageAppendBytes" => f(cf_http_msg_append_bytes as *const ()),
        "_CFHTTPMessageIsHeaderComplete" => f(cf_http_msg_is_header_complete as *const ()),
        "_CFHTTPMessageGetResponseStatusCode" => f(cf_http_msg_get_status as *const ()),
        "_CFHTTPMessageCopyVersion" => f(cf_http_msg_copy_version as *const ()),
        "_CFHTTPMessageCopyRequestMethod" => f(cf_http_msg_copy_method as *const ()),
        "_CFHTTPMessageCopyRequestURL" => f(cf_http_msg_copy_url as *const ()),
        "_CFHTTPMessageSetHeaderFieldValue" => f(cf_http_msg_set_header as *const ()),
        "_CFHTTPMessageCopyHeaderFieldValue" => f(cf_http_msg_copy_header as *const ()),
        "_CFHTTPMessageCopyAllHeaderFields" => f(cf_http_msg_copy_all_headers as *const ()),
        "_CFHTTPMessageSetBody" => f(cf_http_msg_set_body as *const ()),
        "_CFHTTPMessageCopyBody" => f(cf_http_msg_copy_body as *const ()),
        "_CFHTTPMessageCopySerializedMessage" => f(cf_http_msg_copy_serialized as *const ()),
        "_CFHostCreateWithName" => f(cf_host_create_with_name as *const ()),
        "_CFHostGetAddressing" => f(cf_host_get_addressing as *const ()),
        "_CFHostScheduleWithRunLoop" | "_CFHostUnscheduleFromRunLoop" => f(cf_host_noop_rl as *const ()),
        "_CFHostSetClient" => f(cf_host_set_client as *const ()),
        "_CFHostStartInfoResolution" => f(cf_host_start_info as *const ()),
        "_CFNetworkCopySystemProxySettings" => f(cf_net_copy_sys_proxy as *const ()),
        "_CFNetworkCopyProxiesForURL" => f(cf_net_copy_proxies_for_url as *const ()),
        "_NSURLSessionTaskPriorityDefault" => Some(&raw const TASK_PRIORITY_DEFAULT as usize),
        "_NSURLSessionTaskPriorityHigh" => Some(&raw const TASK_PRIORITY_HIGH as usize),
        "_NSURLSessionTaskPriorityLow" => Some(&raw const TASK_PRIORITY_LOW as usize),
        "_NSURLAuthenticationMethodServerTrust" => Some(const_nsstr_ptr("NSURLAuthenticationMethodServerTrust")),
        s if s.starts_with("_NSHTTPCookie") || s.starts_with("_kCF") => {
            let leaked: &'static str = Box::leak(s.trim_start_matches('_').to_string().into_boxed_str());
            Some(const_nsstr_ptr(leaked))
        }
        _ => None,
    }
}

// ---- Network.framework -----------------------------------------------------

static NW_DEFAULT_STUB: usize = 0;

extern "C" fn nw_ret_handle() -> *mut c_void { Box::into_raw(Box::new(0usize)) as *mut c_void }
extern "C" fn nw_noop_1(_a: *mut c_void) {}
extern "C" fn nw_noop_2(_a: *mut c_void, _b: *mut c_void) {}
extern "C" fn nw_recv(_c: *mut c_void, _min: u32, _max: u32, _cb: *const c_void) {}
extern "C" fn nw_send(_c: *mut c_void, _d: *mut c_void, _ctx: *mut c_void, _is_complete: bool, _cb: *const c_void) {}
extern "C" fn nw_is_final(_ctx: *mut c_void) -> bool { true }
extern "C" fn nw_err_code(_err: *mut c_void) -> i32 { 0 }

pub fn network_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "__nw_content_context_default_message"
        | "__nw_parameters_configure_protocol_default_configuration"
        | "__nw_parameters_configure_protocol_disable" => Some(&raw const NW_DEFAULT_STUB as usize),
        "_nw_endpoint_create_host"
        | "_nw_parameters_create_secure_tcp"
        | "_nw_connection_create" => f(nw_ret_handle as *const ()),
        "_nw_connection_cancel" | "_nw_connection_start" => f(nw_noop_1 as *const ()),
        "_nw_connection_set_queue"
        | "_nw_connection_set_state_changed_handler"
        | "_nw_parameters_set_required_interface_type" => f(nw_noop_2 as *const ()),
        "_nw_connection_receive" => f(nw_recv as *const ()),
        "_nw_connection_send" => f(nw_send as *const ()),
        "_nw_content_context_get_is_final" => f(nw_is_final as *const ()),
        "_nw_error_get_error_code" => f(nw_err_code as *const ()),
        _ => None,
    }
}

// ---- CoreMedia -------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CMTime {
    pub value: i64,
    pub timescale: i32,
    pub flags: u32,
    pub epoch: i64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CMTimeRange {
    pub start: CMTime,
    pub duration: CMTime,
}

static K_CM_TIME_ZERO: CMTime = CMTime { value: 0, timescale: 1, flags: 1, epoch: 0 };
static K_CM_TIME_INDEFINITE: CMTime = CMTime { value: 0, timescale: 0, flags: 1 | 16, epoch: 0 };
static K_CM_TIME_POS_INF: CMTime = CMTime { value: 0, timescale: 0, flags: 1 | 4, epoch: 0 };

static CM_BLOCKS: LazyLock<Mutex<HashMap<usize, Vec<u8>>>> = LazyLock::new(Default::default);
static CM_SAMPLES: LazyLock<Mutex<HashMap<usize, (usize, CMTime)>>> = LazyLock::new(Default::default);

pub extern "C" fn cm_time_make(value: i64, timescale: i32) -> CMTime {
    CMTime { value, timescale, flags: 1, epoch: 0 }
}
pub extern "C" fn cm_time_make_with_seconds(seconds: f64, preferred_timescale: i32) -> CMTime {
    let ts = if preferred_timescale <= 0 { 1000 } else { preferred_timescale };
    CMTime { value: (seconds * ts as f64).round() as i64, timescale: ts, flags: 1, epoch: 0 }
}
pub extern "C" fn cm_time_get_seconds(t: CMTime) -> f64 {
    if t.timescale == 0 { 0.0 } else { t.value as f64 / t.timescale as f64 }
}
pub extern "C" fn cm_time_add(lhs: CMTime, rhs: CMTime) -> CMTime {
    if lhs.timescale == rhs.timescale {
        cm_time_make(lhs.value.saturating_add(rhs.value), lhs.timescale)
    } else {
        cm_time_make_with_seconds(cm_time_get_seconds(lhs) + cm_time_get_seconds(rhs), lhs.timescale.max(rhs.timescale))
    }
}
pub extern "C" fn cm_time_subtract(lhs: CMTime, rhs: CMTime) -> CMTime {
    if lhs.timescale == rhs.timescale {
        cm_time_make(lhs.value.saturating_sub(rhs.value), lhs.timescale)
    } else {
        cm_time_make_with_seconds(cm_time_get_seconds(lhs) - cm_time_get_seconds(rhs), lhs.timescale.max(rhs.timescale))
    }
}
pub extern "C" fn cm_time_abs(t: CMTime) -> CMTime {
    CMTime { value: t.value.saturating_abs(), ..t }
}
pub extern "C" fn cm_time_compare(t1: CMTime, t2: CMTime) -> i32 {
    let (s1, s2) = (cm_time_get_seconds(t1), cm_time_get_seconds(t2));
    if s1 < s2 { -1 } else if s1 > s2 { 1 } else { 0 }
}
pub extern "C" fn cm_time_mul_f64(t: CMTime, mult: f64) -> CMTime {
    CMTime { value: (t.value as f64 * mult).round() as i64, ..t }
}
pub extern "C" fn cm_time_range_make(start: CMTime, duration: CMTime) -> CMTimeRange {
    CMTimeRange { start, duration }
}

unsafe extern "C" fn cm_block_create(
    _alloc: *const c_void,
    mem: *const u8,
    len: usize,
    _alloc2: *const c_void,
    _cs: *const c_void,
    off: usize,
    data_len: usize,
    _flags: u32,
    out: *mut *mut c_void,
) -> i32 {
    let mut v = Vec::new();
    if !mem.is_null() && data_len > 0 && off + data_len <= len {
        v.extend_from_slice(std::slice::from_raw_parts(mem.add(off), data_len));
    }
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    CM_BLOCKS.lock().unwrap().insert(h as usize, v);
    if !out.is_null() {
        *out = h;
    }
    0
}
unsafe extern "C" fn cm_block_append(
    buf: *mut c_void,
    mem: *const u8,
    len: usize,
    _alloc: *const c_void,
    _cs: *const c_void,
    off: usize,
    data_len: usize,
    _flags: u32,
) -> i32 {
    if !mem.is_null() && data_len > 0 && off + data_len <= len {
        if let Some(v) = CM_BLOCKS.lock().unwrap().get_mut(&(buf as usize)) {
            v.extend_from_slice(std::slice::from_raw_parts(mem.add(off), data_len));
        }
    }
    0
}
extern "C" fn cm_block_get_len(buf: *mut c_void) -> usize {
    CM_BLOCKS.lock().unwrap().get(&(buf as usize)).map_or(0, |v| v.len())
}
unsafe extern "C" fn cm_block_replace(src: *const u8, buf: *mut c_void, off: usize, len: usize) -> i32 {
    if src.is_null() {
        return -50;
    }
    if let Some(v) = CM_BLOCKS.lock().unwrap().get_mut(&(buf as usize)) {
        if off + len > v.len() {
            v.resize(off + len, 0);
        }
        v[off..off + len].copy_from_slice(std::slice::from_raw_parts(src, len));
    }
    0
}

unsafe extern "C" fn cm_sample_create(
    _alloc: *const c_void,
    _data_buf: *mut c_void,
    _data_ready: bool,
    _cb: *const c_void,
    _refcon: *mut c_void,
    _fmt: *const c_void,
    _num_samples: isize,
    _num_timing: isize,
    timing: *const CMTime,
    _num_sizes: isize,
    _sizes: *const usize,
    out: *mut *mut c_void,
) -> i32 {
    let pts = if timing.is_null() { K_CM_TIME_ZERO } else { *timing };
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    CM_SAMPLES.lock().unwrap().insert(h as usize, (0usize, pts));
    if !out.is_null() {
        *out = h;
    }
    0
}
extern "C" fn cm_sample_get_image_buf(sb: *mut c_void) -> *mut c_void {
    CM_SAMPLES.lock().unwrap().get(&(sb as usize)).map_or(std::ptr::null_mut(), |v| v.0 as *mut c_void)
}
extern "C" fn cm_sample_get_pts(sb: *mut c_void) -> CMTime {
    CM_SAMPLES.lock().unwrap().get(&(sb as usize)).map_or(K_CM_TIME_ZERO, |v| v.1)
}
extern "C" fn cm_sample_get_attachments(_sb: *mut c_void, _create: bool) -> Id {
    nsarray_from_ids(Vec::new())
}
unsafe extern "C" fn cm_video_fmt_from_h264(
    _alloc: *const c_void,
    _cnt: usize,
    _ptrs: *const *const u8,
    _sizes: *const usize,
    _nal_len: i32,
    out: *mut *mut c_void,
) -> i32 {
    if !out.is_null() {
        *out = Box::into_raw(Box::new(0usize)) as *mut c_void;
    }
    0
}

pub fn coremedia_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_kCMTimeZero" => Some(&raw const K_CM_TIME_ZERO as usize),
        "_kCMTimeIndefinite" => Some(&raw const K_CM_TIME_INDEFINITE as usize),
        "_kCMTimePositiveInfinity" => Some(&raw const K_CM_TIME_POS_INF as usize),
        "_CMTimeMake" => f(cm_time_make as *const ()),
        "_CMTimeMakeWithSeconds" => f(cm_time_make_with_seconds as *const ()),
        "_CMTimeGetSeconds" => f(cm_time_get_seconds as *const ()),
        "_CMTimeAdd" => f(cm_time_add as *const ()),
        "_CMTimeSubtract" => f(cm_time_subtract as *const ()),
        "_CMTimeAbsoluteValue" => f(cm_time_abs as *const ()),
        "_CMTimeCompare" => f(cm_time_compare as *const ()),
        "_CMTimeMultiplyByFloat64" => f(cm_time_mul_f64 as *const ()),
        "_CMTimeRangeMake" => f(cm_time_range_make as *const ()),
        "_CMBlockBufferCreateWithMemoryBlock" => f(cm_block_create as *const ()),
        "_CMBlockBufferAppendMemoryBlock" => f(cm_block_append as *const ()),
        "_CMBlockBufferGetDataLength" => f(cm_block_get_len as *const ()),
        "_CMBlockBufferReplaceDataBytes" => f(cm_block_replace as *const ()),
        "_CMSampleBufferCreate" => f(cm_sample_create as *const ()),
        "_CMSampleBufferGetImageBuffer" => f(cm_sample_get_image_buf as *const ()),
        "_CMSampleBufferGetPresentationTimeStamp" => f(cm_sample_get_pts as *const ()),
        "_CMSampleBufferGetSampleAttachmentsArray" => f(cm_sample_get_attachments as *const ()),
        "_CMVideoFormatDescriptionCreateFromH264ParameterSets" => f(cm_video_fmt_from_h264 as *const ()),
        s if s.starts_with("_kCMSampleAttachmentKey_") => {
            let leaked: &'static str = Box::leak(s.trim_start_matches('_').to_string().into_boxed_str());
            Some(const_nsstr_ptr(leaked))
        }
        _ => None,
    }
}

// ---- CoreVideo -------------------------------------------------------------

pub struct PixelBufferState {
    pub width: usize,
    pub height: usize,
    pub format: u32,
    pub bytes_per_row: usize,
    pub pixels: Vec<u8>,
}

static PIXEL_BUFFERS: LazyLock<Mutex<HashMap<usize, PixelBufferState>>> = LazyLock::new(Default::default);

pub unsafe extern "C" fn cv_pixel_buffer_create(
    _alloc: *const c_void,
    width: usize,
    height: usize,
    pixel_format: u32,
    _attrs: Id,
    out: *mut *mut c_void,
) -> i32 {
    let w = width.clamp(1, 8192);
    let h = height.clamp(1, 8192);
    let bpr = w * 4;
    let st = PixelBufferState {
        width: w,
        height: h,
        format: pixel_format,
        bytes_per_row: bpr,
        pixels: vec![0u8; bpr * h],
    };
    let handle = Box::into_raw(Box::new(0usize)) as *mut c_void;
    PIXEL_BUFFERS.lock().unwrap().insert(handle as usize, st);
    if !out.is_null() {
        *out = handle;
    }
    0
}

pub extern "C" fn cv_pb_lock(_pb: *mut c_void, _flags: u64) -> i32 { 0 }
pub extern "C" fn cv_pb_get_base(pb: *mut c_void) -> *mut u8 {
    let mut map = PIXEL_BUFFERS.lock().unwrap();
    map.get_mut(&(pb as usize)).map_or(std::ptr::null_mut(), |s| s.pixels.as_mut_ptr())
}
pub extern "C" fn cv_pb_get_base_plane(pb: *mut c_void, _plane: usize) -> *mut u8 {
    cv_pb_get_base(pb)
}
pub extern "C" fn cv_pb_get_width(pb: *mut c_void) -> usize {
    PIXEL_BUFFERS.lock().unwrap().get(&(pb as usize)).map_or(0, |s| s.width)
}
pub extern "C" fn cv_pb_get_width_plane(pb: *mut c_void, _plane: usize) -> usize {
    cv_pb_get_width(pb)
}
pub extern "C" fn cv_pb_get_height(pb: *mut c_void) -> usize {
    PIXEL_BUFFERS.lock().unwrap().get(&(pb as usize)).map_or(0, |s| s.height)
}
pub extern "C" fn cv_pb_get_height_plane(pb: *mut c_void, _plane: usize) -> usize {
    cv_pb_get_height(pb)
}
pub extern "C" fn cv_pb_get_bpr(pb: *mut c_void) -> usize {
    PIXEL_BUFFERS.lock().unwrap().get(&(pb as usize)).map_or(0, |s| s.bytes_per_row)
}
pub extern "C" fn cv_pb_get_bpr_plane(pb: *mut c_void, _plane: usize) -> usize {
    cv_pb_get_bpr(pb)
}
pub extern "C" fn cv_pb_get_fmt(pb: *mut c_void) -> u32 {
    PIXEL_BUFFERS.lock().unwrap().get(&(pb as usize)).map_or(0, |s| s.format)
}
pub extern "C" fn cv_pb_get_plane_count(_pb: *mut c_void) -> usize { 1 }
pub extern "C" fn cv_pb_retain(pb: *mut c_void) -> *mut c_void { pb }
pub extern "C" fn cv_pb_release(pb: *mut c_void) {
    PIXEL_BUFFERS.lock().unwrap().remove(&(pb as usize));
}
extern "C" fn cv_tex_cache_create(
    _a: *const c_void, _b: Id, _c: *mut c_void, _d: Id, out: *mut *mut c_void,
) -> i32 {
    if !out.is_null() {
        unsafe { *out = Box::into_raw(Box::new(0usize)) as *mut c_void };
    }
    0
}
extern "C" fn cv_tex_cache_create_tex() -> i32 { -6660 }
extern "C" fn cv_tex_cache_flush(_c: *mut c_void, _o: u64) {}
extern "C" fn cv_tex_get_obj(_t: *mut c_void) -> *mut c_void { std::ptr::null_mut() }
extern "C" fn cv_tex_get_name(_t: *mut c_void) -> u32 { 0 }

pub fn corevideo_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_CVPixelBufferCreate" => f(cv_pixel_buffer_create as *const ()),
        "_CVPixelBufferLockBaseAddress" | "_CVPixelBufferUnlockBaseAddress" => f(cv_pb_lock as *const ()),
        "_CVPixelBufferGetBaseAddress" => f(cv_pb_get_base as *const ()),
        "_CVPixelBufferGetBaseAddressOfPlane" => f(cv_pb_get_base_plane as *const ()),
        "_CVPixelBufferGetWidth" => f(cv_pb_get_width as *const ()),
        "_CVPixelBufferGetWidthOfPlane" => f(cv_pb_get_width_plane as *const ()),
        "_CVPixelBufferGetHeight" => f(cv_pb_get_height as *const ()),
        "_CVPixelBufferGetHeightOfPlane" => f(cv_pb_get_height_plane as *const ()),
        "_CVPixelBufferGetBytesPerRow" => f(cv_pb_get_bpr as *const ()),
        "_CVPixelBufferGetBytesPerRowOfPlane" => f(cv_pb_get_bpr_plane as *const ()),
        "_CVPixelBufferGetPixelFormatType" => f(cv_pb_get_fmt as *const ()),
        "_CVPixelBufferGetPlaneCount" => f(cv_pb_get_plane_count as *const ()),
        "_CVBufferRetain" => f(cv_pb_retain as *const ()),
        "_CVPixelBufferRelease" | "_CVBufferRelease" => f(cv_pb_release as *const ()),
        "_CVMetalTextureCacheCreate" | "_CVOpenGLESTextureCacheCreate" => f(cv_tex_cache_create as *const ()),
        "_CVMetalTextureCacheCreateTextureFromImage"
        | "_CVOpenGLESTextureCacheCreateTextureFromImage" => f(cv_tex_cache_create_tex as *const ()),
        "_CVMetalTextureCacheFlush" | "_CVOpenGLESTextureCacheFlush" => f(cv_tex_cache_flush as *const ()),
        "_CVMetalTextureGetTexture" => f(cv_tex_get_obj as *const ()),
        "_CVOpenGLESTextureGetName" => f(cv_tex_get_name as *const ()),
        s if s.starts_with("_kCVPixelBuffer") => {
            let leaked: &'static str = Box::leak(s.trim_start_matches('_').to_string().into_boxed_str());
            Some(const_nsstr_ptr(leaked))
        }
        _ => None,
    }
}

// ---- AudioToolbox / CoreServices / VideoToolbox / JSC ----------------------

extern "C" fn audio_ok0() -> i32 { 0 }
extern "C" fn audio_void(_x: u32) {}
unsafe extern "C" fn audio_find_next(_comp: *mut c_void, _desc: *const c_void) -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}
unsafe extern "C" fn audio_comp_new(_comp: *mut c_void, out: *mut *mut c_void) -> i32 {
    if !out.is_null() {
        *out = Box::into_raw(Box::new(0usize)) as *mut c_void;
    }
    0
}

pub fn audiotoolbox_ext_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_AudioComponentFindNext" => f(audio_find_next as *const ()),
        "_AudioComponentInstanceNew" => f(audio_comp_new as *const ()),
        "_AudioComponentInstanceDispose"
        | "_AudioConverterDispose"
        | "_AudioConverterFillComplexBuffer"
        | "_AudioConverterNew"
        | "_AudioFileClose"
        | "_AudioFileGetProperty"
        | "_AudioFileGetPropertyInfo"
        | "_AudioFileOpenWithCallbacks"
        | "_AudioFileReadPacketData"
        | "_AudioOutputUnitStart"
        | "_AudioOutputUnitStop"
        | "_AudioQueueAllocateBuffer"
        | "_AudioQueueAllocateBufferWithPacketDescriptions"
        | "_AudioQueueDispose"
        | "_AudioQueueEnqueueBuffer"
        | "_AudioQueueEnqueueBufferWithParameters"
        | "_AudioQueueFlush"
        | "_AudioQueueNewOutput"
        | "_AudioQueueOfflineRender"
        | "_AudioQueueSetOfflineRenderFormat"
        | "_AudioQueueSetProperty"
        | "_AudioQueueStart"
        | "_AudioQueueStop"
        | "_AudioSessionAddPropertyListener"
        | "_AudioSessionRemovePropertyListenerWithUserData"
        | "_AudioUnitGetProperty"
        | "_AudioUnitInitialize"
        | "_AudioUnitRender"
        | "_AudioUnitSetProperty"
        | "_AudioUnitUninitialize"
        | "_ExtAudioFileDispose"
        | "_ExtAudioFileGetProperty"
        | "_ExtAudioFileOpenURL"
        | "_ExtAudioFileRead"
        | "_ExtAudioFileSeek"
        | "_ExtAudioFileSetProperty"
        | "_ExtAudioFileTell" => f(audio_ok0 as *const ()),
        "_AudioServicesPlaySystemSound" => f(audio_void as *const ()),
        _ => None,
    }
}

extern "C" fn uttype_conforms_to(a: Id, b: Id) -> bool {
    let sa = rust_str_from_nsstring(a).unwrap_or_default();
    let sb = rust_str_from_nsstring(b).unwrap_or_default();
    sa == sb || sb == "public.data" || sb == "public.content" || (sb == "public.image" && sa.contains("png"))
}
extern "C" fn uttype_for_tag(_cls: Id, tag: Id, _conform: Id) -> Id {
    let ext = rust_str_from_nsstring(tag).unwrap_or_default().to_ascii_lowercase();
    let uti = match ext.as_str() {
        "png" => "public.png",
        "jpg" | "jpeg" => "public.jpeg",
        "gif" => "com.compuserve.gif",
        "tiff" | "tif" => "public.tiff",
        "pdf" => "com.adobe.pdf",
        "mp4" | "mov" => "public.movie",
        "mp3" | "wav" | "aac" => "public.audio",
        _ => "public.data",
    };
    nsstring_from_rust(uti)
}

pub fn coreservices_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_UTTypeConformsTo" => f(uttype_conforms_to as *const ()),
        "_UTTypeCreatePreferredIdentifierForTag" => f(uttype_for_tag as *const ()),
        "_kUTTagClassFilenameExtension" => Some(const_nsstr_ptr("public.filename-extension")),
        "_kUTTypeAudio" => Some(const_nsstr_ptr("public.audio")),
        "_kUTTypeGIF" => Some(const_nsstr_ptr("com.compuserve.gif")),
        "_kUTTypeImage" => Some(const_nsstr_ptr("public.image")),
        "_kUTTypeJPEG" => Some(const_nsstr_ptr("public.jpeg")),
        "_kUTTypeLivePhoto" => Some(const_nsstr_ptr("com.apple.live-photo")),
        "_kUTTypeMovie" => Some(const_nsstr_ptr("public.movie")),
        "_kUTTypePDF" => Some(const_nsstr_ptr("com.adobe.pdf")),
        "_kUTTypePNG" => Some(const_nsstr_ptr("public.png")),
        "_kUTTypeScalableVectorGraphics" => Some(const_nsstr_ptr("public.svg-image")),
        "_kUTTypeTIFF" => Some(const_nsstr_ptr("public.tiff")),
        "_kUTTypeVideo" => Some(const_nsstr_ptr("public.video")),
        _ => None,
    }
}

pub fn videotoolbox_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_VTDecompressionSessionCanAcceptFormatDescription" => {
            extern "C" fn accept(_s: *mut c_void, _d: *const c_void) -> bool { false }
            f(accept as *const ())
        }
        "_VTDecompressionSessionCreate" => {
            unsafe extern "C" fn create(_a: *const c_void, _d: *const c_void, _spec: Id, _attr: Id, _cb: *const c_void, out: *mut *mut c_void) -> i32 {
                if !out.is_null() {
                    *out = Box::into_raw(Box::new(0usize)) as *mut c_void;
                }
                0
            }
            f(create as *const ())
        }
        "_VTDecompressionSessionDecodeFrame"
        | "_VTDecompressionSessionWaitForAsynchronousFrames" => f(audio_ok0 as *const ()),
        "_VTDecompressionSessionInvalidate" => f(nw_noop_1 as *const ()),
        _ => None,
    }
}

extern "C" fn jsc_gc(_ctx: *mut c_void) {}
extern "C" fn jsc_buf_len(_ctx: *mut c_void, _obj: *mut c_void, _exc: *mut *mut c_void) -> usize { 0 }
extern "C" fn jsc_buf_ptr(_ctx: *mut c_void, _obj: *mut c_void, _exc: *mut *mut c_void) -> *mut c_void { std::ptr::null_mut() }
extern "C" fn jsc_make_buf(_ctx: *mut c_void, _bytes: *mut c_void, _len: usize, _dealloc: *const c_void, _dealloc_ctx: *mut c_void, _exc: *mut *mut c_void) -> *mut c_void { std::ptr::null_mut() }
extern "C" fn jsc_typed_type(_ctx: *mut c_void, _val: *mut c_void, _exc: *mut *mut c_void) -> u32 { 9 } // kJSTypedArrayTypeNone

pub fn jsc_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_JSGarbageCollect" => f(jsc_gc as *const ()),
        "_JSObjectGetArrayBufferByteLength" => f(jsc_buf_len as *const ()),
        "_JSObjectGetArrayBufferBytesPtr" => f(jsc_buf_ptr as *const ()),
        "_JSObjectMakeArrayBufferWithBytesNoCopy" => f(jsc_make_buf as *const ()),
        "_JSValueGetTypedArrayType" => f(jsc_typed_type as *const ()),
        _ => None,
    }
}

// ---- Extended UIKit C APIs -------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
pub struct UIEdgeInsets {
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
}

static UI_EDGE_INSETS_ZERO: UIEdgeInsets = UIEdgeInsets { top: 0.0, left: 0.0, bottom: 0.0, right: 0.0 };
static UI_FONT_WEIGHT_SEMIBOLD: f64 = 0.30;
static UI_BG_TASK_INVALID: usize = 0;
static UI_LAYOUT_FITTING_COMPRESSED: pear_graphics::CGSize = pear_graphics::CGSize { width: 0.0, height: 0.0 };
static UI_VIEW_NO_INTRINSIC: f64 = -1.0;
static UI_WIN_LEVEL_NORMAL: f64 = 0.0;
static UI_WIN_LEVEL_STATUS: f64 = 1000.0;
static UI_WIN_LEVEL_ALERT: f64 = 2000.0;
static UI_ACCESSIBILITY_DIRECT: u64 = 0x0000_0000_0000_2000;

static UI_CTX_STACK: LazyLock<Mutex<Vec<usize>>> = LazyLock::new(Default::default);

extern "C" fn ns_string_from_insets(ins: UIEdgeInsets) -> Id {
    nsstring_from_rust(&format!("{{{}, {}, {}, {}}}", ins.top, ins.left, ins.bottom, ins.right))
}
extern "C" fn ui_acc_false() -> bool { false }
extern "C" fn ui_begin_img_ctx_opts(size: pear_graphics::CGSize, _opaque: bool, scale: f64) {
    let s = if scale <= 0.0 { 1.0 } else { scale };
    let w = (size.width * s).max(1.0) as usize;
    let h = (size.height * s).max(1.0) as usize;
    if let Some(f) = pear_graphics::symbol("_CGBitmapContextCreate") {
        let create: extern "C" fn(*mut u8, usize, usize, usize, usize, *mut c_void, u32) -> *mut c_void =
            unsafe { std::mem::transmute(f) };
        let ctx = create(std::ptr::null_mut(), w, h, 8, w * 4, std::ptr::null_mut(), 0);
        UI_CTX_STACK.lock().unwrap().push(ctx as usize);
    }
}
extern "C" fn ui_begin_img_ctx(size: pear_graphics::CGSize) {
    ui_begin_img_ctx_opts(size, false, 1.0);
}
extern "C" fn ui_get_cur_ctx() -> *mut c_void {
    UI_CTX_STACK.lock().unwrap().last().copied().unwrap_or(0) as *mut c_void
}
extern "C" fn ui_get_img_from_cur_ctx() -> Id {
    let ctx = ui_get_cur_ctx();
    if ctx.is_null() {
        return std::ptr::null_mut();
    }
    if let Some(f) = pear_graphics::symbol("_CGBitmapContextCreateImage") {
        let mk: extern "C" fn(*mut c_void) -> *mut c_void = unsafe { std::mem::transmute(f) };
        return mk(ctx) as Id;
    }
    std::ptr::null_mut()
}
extern "C" fn ui_end_img_ctx() {
    if let Some(ctx) = UI_CTX_STACK.lock().unwrap().pop() {
        if let Some(f) = pear_graphics::symbol("_CGContextRelease") {
            let rel: extern "C" fn(*mut c_void) = unsafe { std::mem::transmute(f) };
            rel(ctx as *mut c_void);
        }
    }
}
extern "C" fn ui_image_rep(_img: Id, _q: f64) -> Id { nsdata_from_bytes(&[]) }
extern "C" fn ui_image_png(_img: Id) -> Id { nsdata_from_bytes(&[]) }
extern "C" fn ui_save_noop(_a: Id, _b: Id, _c: *const c_void, _d: *mut c_void) {}
extern "C" fn ui_video_compat(_p: Id) -> bool { false }

pub fn uikit_ext_symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        "_NSStringFromUIEdgeInsets" => f(ns_string_from_insets as *const ()),
        "_UIGraphicsBeginImageContext" => f(ui_begin_img_ctx as *const ()),
        "_UIGraphicsBeginImageContextWithOptions" => f(ui_begin_img_ctx_opts as *const ()),
        "_UIGraphicsEndImageContext" => f(ui_end_img_ctx as *const ()),
        "_UIGraphicsGetCurrentContext" => f(ui_get_cur_ctx as *const ()),
        "_UIGraphicsGetImageFromCurrentImageContext" => f(ui_get_img_from_cur_ctx as *const ()),
        "_UIImagePNGRepresentation" => f(ui_image_png as *const ()),
        "_UIImageJPEGRepresentation" => f(ui_image_rep as *const ()),
        "_UIImageWriteToSavedPhotosAlbum" | "_UISaveVideoAtPathToSavedPhotosAlbum" => f(ui_save_noop as *const ()),
        "_UIVideoAtPathIsCompatibleWithSavedPhotosAlbum" => f(ui_video_compat as *const ()),
        "_UIAccessibilityTraitAllowsDirectInteraction" => Some(&raw const UI_ACCESSIBILITY_DIRECT as usize),
        s if s.starts_with("_UIAccessibility") => f(ui_acc_false as *const ()),
        "_UIEdgeInsetsZero" => Some(&raw const UI_EDGE_INSETS_ZERO as usize),
        "_UIFontWeightSemibold" => Some(&raw const UI_FONT_WEIGHT_SEMIBOLD as usize),
        "_UIBackgroundTaskInvalid" => Some(&raw const UI_BG_TASK_INVALID as usize),
        "_UILayoutFittingCompressedSize" => Some(&raw const UI_LAYOUT_FITTING_COMPRESSED as usize),
        "_UIViewNoIntrinsicMetric" => Some(&raw const UI_VIEW_NO_INTRINSIC as usize),
        "_UIWindowLevelNormal" => Some(&raw const UI_WIN_LEVEL_NORMAL as usize),
        "_UIWindowLevelStatusBar" => Some(&raw const UI_WIN_LEVEL_STATUS as usize),
        "_UIWindowLevelAlert" => Some(&raw const UI_WIN_LEVEL_ALERT as usize),
        "_NSBaselineOffsetAttributeName"
        | "_NSFontAttributeName"
        | "_NSForegroundColorAttributeName"
        | "_NSKernAttributeName"
        | "_NSLinkAttributeName"
        | "_NSParagraphStyleAttributeName"
        | "_NSUnderlineStyleAttributeName" => {
            let leaked: &'static str = Box::leak(name.trim_start_matches('_').to_string().into_boxed_str());
            Some(const_nsstr_ptr(leaked))
        }
        _ => None,
    }
}

// ---- Misc frameworks (CoreTelephony, MediaPlayer, CoreHaptics, etc.) -------

static PH_MAX_SIZE: pear_graphics::CGSize = pear_graphics::CGSize { width: -1.0, height: -1.0 };

extern "C" fn cn_copy_interfaces() -> Id { nsarray_from_ids(Vec::new()) }
extern "C" fn cn_copy_net_info(_iface: Id) -> Id { std::ptr::null_mut() }
extern "C" fn sc_error() -> i32 { 0 }

pub fn misc_framework_symbol(path: &str, name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    if path.contains("SystemConfiguration") {
        return match name {
            "_CNCopySupportedInterfaces" => f(cn_copy_interfaces as *const ()),
            "_CNCopyCurrentNetworkInfo" => f(cn_copy_net_info as *const ()),
            "_SCError" => f(sc_error as *const ()),
            _ => None,
        };
    }
    if path.contains("Photos") && name == "_PHImageManagerMaximumSize" {
        return Some(&raw const PH_MAX_SIZE as usize);
    }
    if path.contains("CoreTelephony")
        || path.contains("MediaPlayer")
        || path.contains("CoreHaptics")
        || path.contains("AuthenticationServices")
        || path.contains("CoreImage")
        || path.contains("StoreKit")
        || path.contains("GameKit")
        || path.contains("WebKit")
    {
        if name.starts_with('_') && !name.starts_with("_OBJC_") {
            let leaked: &'static str = Box::leak(name.trim_start_matches('_').to_string().into_boxed_str());
            return Some(const_nsstr_ptr(leaked));
        }
    }
    None
}

#[allow(dead_code)]
fn _keep_misc_types(_c: *const c_char, _cs: Option<&CStr>) {}
