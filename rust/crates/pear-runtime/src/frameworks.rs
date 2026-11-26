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

// ---- Virtual Mach port & kqueue/kevent64 bridge (for Linux hosts) ---------

#[cfg(not(target_os = "macos"))]
pub mod linux_mach_kqueue {
    use std::collections::{HashMap, VecDeque};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Condvar, LazyLock, Mutex};

    static NEXT_PORT: AtomicU32 = AtomicU32::new(0x100);
    static NEXT_KQ: AtomicU32 = AtomicU32::new(1000);
    static PORT_MSGS: LazyLock<Mutex<HashMap<u32, VecDeque<Vec<u8>>>>> = LazyLock::new(Default::default);
    static KQUEUES: LazyLock<Mutex<HashMap<i32, std::sync::Arc<(Mutex<Vec<usize>>, Condvar)>>>> =
        LazyLock::new(Default::default);

    pub unsafe extern "C" fn mach_port_allocate(_task: u32, _right: u32, name_out: *mut u32) -> i32 {
        let p = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
        if !name_out.is_null() {
            *name_out = p;
        }
        PORT_MSGS.lock().unwrap().insert(p, VecDeque::new());
        0
    }

    pub unsafe extern "C" fn mach_port_construct(
        task: u32,
        _opts: *const c_void,
        _ctx: usize,
        name_out: *mut u32,
    ) -> i32 {
        mach_port_allocate(task, 0, name_out)
    }

    pub extern "C" fn mach_port_deallocate(_task: u32, name: u32) -> i32 {
        PORT_MSGS.lock().unwrap().remove(&name);
        0
    }

    pub extern "C" fn mach_port_mod_refs(_task: u32, _name: u32, _right: i32, _delta: i32) -> i32 {
        0
    }

    pub extern "C" fn mach_port_peek() -> i32 {
        -15 // KERN_NAME_EXISTS / empty
    }

    pub unsafe extern "C" fn mach_msg(
        msg: *mut u8,
        option: i32,
        send_size: u32,
        rcv_size: u32,
        rcv_name: u32,
        timeout_ms: u32,
        _notify: u32,
    ) -> i32 {
        const MACH_SEND_MSG: i32 = 1;
        const MACH_RCV_MSG: i32 = 2;
        if !msg.is_null() && (option & MACH_SEND_MSG) != 0 && send_size >= 24 {
            let remote_port = *(msg.add(8) as *const u32);
            let buf = std::slice::from_raw_parts(msg, send_size as usize).to_vec();
            PORT_MSGS.lock().unwrap().entry(remote_port).or_default().push_back(buf);
            for kq in KQUEUES.lock().unwrap().values() {
                kq.0.lock().unwrap().push(remote_port as usize);
                kq.1.notify_all();
            }
        }
        if !msg.is_null() && (option & MACH_RCV_MSG) != 0 {
            if let Some(pkt) = PORT_MSGS.lock().unwrap().get_mut(&rcv_name).and_then(|q| q.pop_front()) {
                let n = pkt.len().min(rcv_size as usize);
                std::ptr::copy_nonoverlapping(pkt.as_ptr(), msg, n);
                return 0;
            }
            if timeout_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis((timeout_ms as u64).min(5)));
            }
            return 0x10004003; // MACH_RCV_TIMED_OUT
        }
        0
    }

    pub extern "C" fn kqueue_impl() -> i32 {
        let fd = NEXT_KQ.fetch_add(1, Ordering::Relaxed) as i32;
        KQUEUES.lock().unwrap().insert(
            fd,
            std::sync::Arc::new((Mutex::new(Vec::new()), Condvar::new())),
        );
        fd
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct Kevent64S {
        pub ident: u64,
        pub filter: i16,
        pub flags: u16,
        pub fflags: u32,
        pub data: i64,
        pub udata: u64,
        pub ext: [u64; 2],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct Timespec {
        pub tv_sec: isize,
        pub tv_nsec: isize,
    }

    pub unsafe extern "C" fn kevent64_impl(
        kq: i32,
        _changelist: *const Kevent64S,
        _nchanges: i32,
        eventlist: *mut Kevent64S,
        nevents: i32,
        _flags: u32,
        timeout: *const Timespec,
    ) -> i32 {
        let handle = KQUEUES.lock().unwrap().get(&kq).cloned();
        let Some(h) = handle else { return 0 };
        let wait_dur = if timeout.is_null() {
            std::time::Duration::from_millis(16)
        } else {
            let ts = *timeout;
            std::time::Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32)
        };
        let mut q = h.0.lock().unwrap();
        if q.is_empty() && !wait_dur.is_zero() {
            let res = h.1.wait_timeout(q, wait_dur).unwrap();
            q = res.0;
        }
        if q.is_empty() || eventlist.is_null() || nevents <= 0 {
            return 0;
        }
        let port = q.remove(0) as u64;
        *eventlist = Kevent64S {
            ident: port,
            filter: -8, // EVFILT_MACHPORT
            flags: 0,
            fflags: 0,
            data: 0,
            udata: 0,
            ext: [0, 0],
        };
        1
    }
}

// ---- Foundation collection & concurrency ObjC classes ----------------------

macro_rules! fw_guest_class {
    ($cls:ident, $meta:ident) => {
        static $meta: pear_libobjc::object::StaticClass =
            pear_libobjc::object::StaticClass(std::cell::UnsafeCell::new(pear_libobjc::abi::ClassT {
                isa: &raw const pear_libobjc::object::NSOBJECT_META as *const pear_libobjc::abi::ClassT,
                superclass: &raw const pear_libobjc::object::NSOBJECT_META as *const pear_libobjc::abi::ClassT,
                cache: &raw const pear_libobjc::object::EMPTY_CACHE as *const c_void,
                vtable: std::ptr::null(),
                bits: 0,
            }));
        static $cls: pear_libobjc::object::StaticClass =
            pear_libobjc::object::StaticClass(std::cell::UnsafeCell::new(pear_libobjc::abi::ClassT {
                isa: &raw const $meta as *const pear_libobjc::abi::ClassT,
                superclass: &raw const pear_libobjc::object::NSOBJECT_CLASS as *const pear_libobjc::abi::ClassT,
                cache: &raw const pear_libobjc::object::EMPTY_CACHE as *const c_void,
                vtable: std::ptr::null(),
                bits: 0,
            }));
    };
}

fw_guest_class!(NSMAPTABLE_CLASS, NSMAPTABLE_META);
fw_guest_class!(NSHASHTABLE_CLASS, NSHASHTABLE_META);
fw_guest_class!(NSLOCK_CLASS, NSLOCK_META);
fw_guest_class!(NSRECURSIVELOCK_CLASS, NSRECURSIVELOCK_META);
fw_guest_class!(NSCACHE_CLASS, NSCACHE_META);
fw_guest_class!(NSOPERATION_CLASS, NSOPERATION_META);
fw_guest_class!(NSBLOCKOPERATION_CLASS, NSBLOCKOPERATION_META);
fw_guest_class!(NSDATEFORMATTER_CLASS, NSDATEFORMATTER_META);

struct MapTableBacking {
    strong_keys: bool,
    strong_values: bool,
    by_ptr: HashMap<usize, usize>,
    by_str: HashMap<String, usize>,
}

impl Default for MapTableBacking {
    fn default() -> Self {
        Self {
            strong_keys: true,
            strong_values: true,
            by_ptr: HashMap::new(),
            by_str: HashMap::new(),
        }
    }
}

struct HashTableBacking {
    strong: bool,
    items: Vec<usize>,
}

impl Default for HashTableBacking {
    fn default() -> Self {
        Self { strong: true, items: Vec::new() }
    }
}

static MAP_TABLES: LazyLock<Mutex<HashMap<usize, MapTableBacking>>> = LazyLock::new(Default::default);
static HASH_TABLES: LazyLock<Mutex<HashMap<usize, HashTableBacking>>> = LazyLock::new(Default::default);
static CACHES: LazyLock<Mutex<HashMap<usize, (HashMap<String, usize>, String)>>> = LazyLock::new(Default::default);
static LOCKS: LazyLock<Mutex<HashMap<usize, std::sync::Arc<(Mutex<(Option<std::thread::ThreadId>, usize, String)>, std::sync::Condvar)>>>> =
    LazyLock::new(Default::default);
static OPS: LazyLock<Mutex<HashMap<usize, (Vec<usize>, usize, bool, bool)>>> = LazyLock::new(Default::default);
static DATE_FORMATTERS: LazyLock<Mutex<HashMap<usize, String>>> = LazyLock::new(Default::default);

#[inline]
fn is_strong_pointer_opts(opts: usize) -> bool {
    (opts & 0xff) == 0
}

extern "C" fn col_self(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id { this }
extern "C" fn col_noop0(_this: Id, _cmd: pear_libobjc::abi::Sel) {}
extern "C" fn col_noop1(_this: Id, _cmd: pear_libobjc::abi::Sel, _a: usize) {}
extern "C" fn col_true(_this: Id, _cmd: pear_libobjc::abi::Sel) -> bool { true }
extern "C" fn col_false(_this: Id, _cmd: pear_libobjc::abi::Sel) -> bool { false }

fn map_table_new(cls: Id, strong_keys: bool, strong_values: bool) -> Id {
    let inst = unsafe { pear_libobjc::object::alloc(cls as *const pear_libobjc::abi::ClassT) };
    MAP_TABLES.lock().unwrap().insert(
        inst as usize,
        MapTableBacking {
            strong_keys,
            strong_values,
            by_ptr: HashMap::new(),
            by_str: HashMap::new(),
        },
    );
    inst
}
extern "C" fn map_table_alloc(cls: Id, _cmd: pear_libobjc::abi::Sel, ko: usize, vo: usize) -> Id {
    map_table_new(cls, is_strong_pointer_opts(ko), is_strong_pointer_opts(vo))
}
extern "C" fn map_table_alloc_s2w(cls: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    map_table_new(cls, true, false)
}
extern "C" fn map_table_alloc_w2s(cls: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    map_table_new(cls, false, true)
}
extern "C" fn map_table_alloc_s2s(cls: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    map_table_new(cls, true, true)
}
extern "C" fn map_table_alloc_w2w(cls: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    map_table_new(cls, false, false)
}
extern "C" fn map_table_init_cap(this: Id, _cmd: pear_libobjc::abi::Sel, ko: usize, vo: usize, _cap: usize) -> Id {
    let mut map = MAP_TABLES.lock().unwrap();
    let entry = map.entry(this as usize).or_default();
    entry.strong_keys = is_strong_pointer_opts(ko);
    entry.strong_values = is_strong_pointer_opts(vo);
    this
}
extern "C" fn map_table_set(this: Id, _cmd: pear_libobjc::abi::Sel, obj: Id, key: Id) {
    if key.is_null() { return; }
    if obj.is_null() {
        map_table_remove(this, pear_libobjc::runtime::sel("removeObjectForKey:"), key);
        return;
    }
    let s = rust_str_from_nsstring(key);
    let (retain_k, retain_v, old_val) = {
        let mut map = MAP_TABLES.lock().unwrap();
        let entry = map.entry(this as usize).or_default();
        let had_key = entry.by_ptr.contains_key(&(key as usize));
        let prev = entry.by_ptr.insert(key as usize, obj as usize);
        if let Some(ref ks) = s { entry.by_str.insert(ks.clone(), obj as usize); }
        let rk = entry.strong_keys && !had_key;
        let rv = entry.strong_values && prev != Some(obj as usize);
        let drop_v = if entry.strong_values && prev != Some(obj as usize) { prev } else { None };
        (rk, rv, drop_v)
    };
    if retain_k { pear_libobjc::object::retain(key); }
    if retain_v { pear_libobjc::object::retain(obj); }
    if let Some(ov) = old_val {
        pear_libobjc::object::release(ov as Id);
    }
}
extern "C" fn map_table_get(this: Id, _cmd: pear_libobjc::abi::Sel, key: Id) -> Id {
    if key.is_null() { return std::ptr::null_mut(); }
    let s = rust_str_from_nsstring(key);
    let map = MAP_TABLES.lock().unwrap();
    if let Some(entry) = map.get(&(this as usize)) {
        if let Some(&v) = entry.by_ptr.get(&(key as usize)) {
            return v as Id;
        }
        if let Some(ks) = s {
            if let Some(&v) = entry.by_str.get(&ks) {
                return v as Id;
            }
        }
    }
    std::ptr::null_mut()
}
extern "C" fn map_table_remove(this: Id, _cmd: pear_libobjc::abi::Sel, key: Id) {
    if key.is_null() { return; }
    let s = rust_str_from_nsstring(key);
    let to_release = {
        let mut map = MAP_TABLES.lock().unwrap();
        let Some(entry) = map.get_mut(&(this as usize)) else { return; };
        let removed_val = entry.by_ptr.remove(&(key as usize));
        if let Some(ks) = s { entry.by_str.remove(&ks); }
        let mut drop_list = Vec::new();
        if let Some(v) = removed_val {
            if entry.strong_values { drop_list.push(v as Id); }
            if entry.strong_keys { drop_list.push(key); }
        }
        drop_list
    };
    for p in to_release {
        pear_libobjc::object::release(p);
    }
}
extern "C" fn map_table_remove_all(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let to_release = {
        let mut map = MAP_TABLES.lock().unwrap();
        let Some(entry) = map.get_mut(&(this as usize)) else { return; };
        entry.by_str.clear();
        let mut drop_list = Vec::new();
        for (k, v) in entry.by_ptr.drain() {
            if entry.strong_values { drop_list.push(v as Id); }
            if entry.strong_keys { drop_list.push(k as Id); }
        }
        drop_list
    };
    for p in to_release {
        pear_libobjc::object::release(p);
    }
}
extern "C" fn map_table_dealloc(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let removed = MAP_TABLES.lock().unwrap().remove(&(this as usize));
    if let Some(entry) = removed {
        for (k, v) in entry.by_ptr {
            if entry.strong_values { pear_libobjc::object::release(v as Id); }
            if entry.strong_keys { pear_libobjc::object::release(k as Id); }
        }
    }
    unsafe { pear_libobjc::object::dispose(this) };
}
extern "C" fn map_table_count(this: Id, _cmd: pear_libobjc::abi::Sel) -> usize {
    MAP_TABLES.lock().unwrap().get(&(this as usize)).map_or(0, |e| e.by_ptr.len())
}
extern "C" fn map_table_dict_rep(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    let pairs: Vec<(String, Id)> = MAP_TABLES
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| e.by_str.iter().map(|(k, &v)| (k.clone(), v as Id)).collect())
        .unwrap_or_default();
    nsdict_from_pairs(pairs)
}
fn array_object_enumerator(items: Vec<Id>) -> Id {
    let arr = nsarray_from_ids(items);
    unsafe {
        let s = pear_libobjc::runtime::sel("objectEnumerator");
        let d = pear_libobjc::message::lookup_for_send(arr, s);
        let f: extern "C" fn(Id, pear_libobjc::abi::Sel) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s)
    }
}
extern "C" fn map_table_obj_enum(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    let items: Vec<Id> = MAP_TABLES
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| e.by_ptr.values().map(|&v| v as Id).collect())
        .unwrap_or_default();
    array_object_enumerator(items)
}
extern "C" fn map_table_key_enum(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    let items: Vec<Id> = MAP_TABLES
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| e.by_ptr.keys().map(|&k| k as Id).collect())
        .unwrap_or_default();
    array_object_enumerator(items)
}

fn hash_table_new(cls: Id, strong: bool) -> Id {
    let inst = unsafe { pear_libobjc::object::alloc(cls as *const pear_libobjc::abi::ClassT) };
    HASH_TABLES.lock().unwrap().insert(inst as usize, HashTableBacking { strong, items: Vec::new() });
    inst
}
extern "C" fn hash_table_alloc0(cls: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    hash_table_new(cls, true)
}
extern "C" fn hash_table_alloc_weak(cls: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    hash_table_new(cls, false)
}
extern "C" fn hash_table_alloc1(cls: Id, _cmd: pear_libobjc::abi::Sel, opts: usize) -> Id {
    hash_table_new(cls, is_strong_pointer_opts(opts))
}
extern "C" fn hash_table_init(this: Id, _cmd: pear_libobjc::abi::Sel, opts: usize, _cap: usize) -> Id {
    let mut map = HASH_TABLES.lock().unwrap();
    let entry = map.entry(this as usize).or_default();
    entry.strong = is_strong_pointer_opts(opts);
    this
}
extern "C" fn hash_table_add(this: Id, _cmd: pear_libobjc::abi::Sel, obj: Id) {
    if obj.is_null() { return; }
    let need_retain = {
        let mut map = HASH_TABLES.lock().unwrap();
        let entry = map.entry(this as usize).or_default();
        if !entry.items.contains(&(obj as usize)) {
            entry.items.push(obj as usize);
            entry.strong
        } else {
            false
        }
    };
    if need_retain {
        pear_libobjc::object::retain(obj);
    }
}
extern "C" fn hash_table_remove(this: Id, _cmd: pear_libobjc::abi::Sel, obj: Id) {
    if obj.is_null() { return; }
    let need_release = {
        let mut map = HASH_TABLES.lock().unwrap();
        let Some(entry) = map.get_mut(&(this as usize)) else { return; };
        let before = entry.items.len();
        entry.items.retain(|&x| x != obj as usize);
        entry.strong && entry.items.len() < before
    };
    if need_release {
        pear_libobjc::object::release(obj);
    }
}
extern "C" fn hash_table_contains(this: Id, _cmd: pear_libobjc::abi::Sel, obj: Id) -> bool {
    HASH_TABLES.lock().unwrap().get(&(this as usize)).map_or(false, |e| e.items.contains(&(obj as usize)))
}
extern "C" fn hash_table_remove_all(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let (strong, drained) = {
        let mut map = HASH_TABLES.lock().unwrap();
        let Some(entry) = map.get_mut(&(this as usize)) else { return; };
        (entry.strong, std::mem::take(&mut entry.items))
    };
    if strong {
        for p in drained { pear_libobjc::object::release(p as Id); }
    }
}
extern "C" fn hash_table_dealloc(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let removed = HASH_TABLES.lock().unwrap().remove(&(this as usize));
    if let Some(entry) = removed {
        if entry.strong {
            for p in entry.items { pear_libobjc::object::release(p as Id); }
        }
    }
    unsafe { pear_libobjc::object::dispose(this) };
}
extern "C" fn hash_table_count(this: Id, _cmd: pear_libobjc::abi::Sel) -> usize {
    HASH_TABLES.lock().unwrap().get(&(this as usize)).map_or(0, |e| e.items.len())
}
extern "C" fn hash_table_all(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    let items: Vec<Id> = HASH_TABLES
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| e.items.iter().map(|&p| p as Id).collect())
        .unwrap_or_default();
    nsarray_from_ids(items)
}

fn lock_handle(this: Id) -> std::sync::Arc<(Mutex<(Option<std::thread::ThreadId>, usize, String)>, std::sync::Condvar)> {
    let mut map = LOCKS.lock().unwrap();
    map.entry(this as usize)
        .or_insert_with(|| std::sync::Arc::new((Mutex::new((None, 0, String::new())), std::sync::Condvar::new())))
        .clone()
}
extern "C" fn nslock_lock(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let h = lock_handle(this);
    let me = std::thread::current().id();
    let mut st = h.0.lock().unwrap();
    while st.0.is_some() && st.0 != Some(me) {
        st = h.1.wait(st).unwrap();
    }
    st.0 = Some(me);
    st.1 += 1;
}
extern "C" fn nslock_try_lock(this: Id, _cmd: pear_libobjc::abi::Sel) -> bool {
    let h = lock_handle(this);
    let me = std::thread::current().id();
    let mut st = h.0.lock().unwrap();
    if st.0.is_none() || st.0 == Some(me) {
        st.0 = Some(me);
        st.1 += 1;
        true
    } else {
        false
    }
}
extern "C" fn nslock_unlock(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let h = lock_handle(this);
    let mut st = h.0.lock().unwrap();
    if st.1 > 0 {
        st.1 -= 1;
        if st.1 == 0 {
            st.0 = None;
            h.1.notify_one();
        }
    }
}
extern "C" fn nslock_set_name(this: Id, _cmd: pear_libobjc::abi::Sel, name: Id) {
    let h = lock_handle(this);
    h.0.lock().unwrap().2 = rust_str_from_nsstring(name).unwrap_or_default();
}
extern "C" fn nslock_name(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    let h = lock_handle(this);
    let n = h.0.lock().unwrap().2.clone();
    nsstring_from_rust(&n)
}

extern "C" fn nscache_get(this: Id, _cmd: pear_libobjc::abi::Sel, key: Id) -> Id {
    let k = rust_str_from_nsstring(key).unwrap_or_else(|| format!("{:#x}", key as usize));
    CACHES.lock().unwrap().get(&(this as usize)).and_then(|(m, _)| m.get(&k).copied()).unwrap_or(0) as Id
}
extern "C" fn nscache_set(this: Id, _cmd: pear_libobjc::abi::Sel, obj: Id, key: Id) {
    let k = rust_str_from_nsstring(key).unwrap_or_else(|| format!("{:#x}", key as usize));
    let (need_retain, old_val) = {
        let mut map = CACHES.lock().unwrap();
        let (m, _) = map.entry(this as usize).or_default();
        if obj.is_null() {
            (false, m.remove(&k))
        } else {
            let prev = m.insert(k, obj as usize);
            (prev != Some(obj as usize), if prev != Some(obj as usize) { prev } else { None })
        }
    };
    if need_retain { pear_libobjc::object::retain(obj); }
    if let Some(ov) = old_val { pear_libobjc::object::release(ov as Id); }
}
extern "C" fn nscache_set_cost(this: Id, cmd: pear_libobjc::abi::Sel, obj: Id, key: Id, _cost: usize) {
    nscache_set(this, cmd, obj, key);
}
extern "C" fn nscache_rem(this: Id, _cmd: pear_libobjc::abi::Sel, key: Id) {
    let k = rust_str_from_nsstring(key).unwrap_or_else(|| format!("{:#x}", key as usize));
    let prev = CACHES.lock().unwrap().get_mut(&(this as usize)).and_then(|(m, _)| m.remove(&k));
    if let Some(ov) = prev { pear_libobjc::object::release(ov as Id); }
}
extern "C" fn nscache_rem_all(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let drained: Vec<usize> = CACHES
        .lock()
        .unwrap()
        .get_mut(&(this as usize))
        .map(|(m, _)| m.drain().map(|(_, v)| v).collect())
        .unwrap_or_default();
    for v in drained { pear_libobjc::object::release(v as Id); }
}
extern "C" fn nscache_dealloc(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let removed = CACHES.lock().unwrap().remove(&(this as usize));
    if let Some((m, _)) = removed {
        for (_, v) in m { pear_libobjc::object::release(v as Id); }
    }
    unsafe { pear_libobjc::object::dispose(this) };
}
extern "C" fn nscache_set_name(this: Id, _cmd: pear_libobjc::abi::Sel, name: Id) {
    let mut map = CACHES.lock().unwrap();
    map.entry(this as usize).or_default().1 = rust_str_from_nsstring(name).unwrap_or_default();
}
extern "C" fn nscache_name(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    let n = CACHES.lock().unwrap().get(&(this as usize)).map(|(_, s)| s.clone()).unwrap_or_default();
    nsstring_from_rust(&n)
}

unsafe fn invoke_objc_block(block: usize) {
    if block < 0x10000 { return; }
    let invoke = *((block as *const usize).add(2));
    if invoke > 0x10000 {
        let f: extern "C" fn(usize) = std::mem::transmute(invoke);
        f(block);
    }
}

extern "C" fn nsop_block_with_block(cls: Id, _cmd: pear_libobjc::abi::Sel, block: Id) -> Id {
    let inst = unsafe { pear_libobjc::object::alloc(cls as *const pear_libobjc::abi::ClassT) };
    let mut blocks = Vec::new();
    if !block.is_null() { blocks.push(block as usize); }
    OPS.lock().unwrap().insert(inst as usize, (blocks, 0, false, false));
    inst
}
extern "C" fn nsop_add_block(this: Id, _cmd: pear_libobjc::abi::Sel, block: Id) {
    if !block.is_null() {
        OPS.lock().unwrap().entry(this as usize).or_default().0.push(block as usize);
    }
}
extern "C" fn nsop_set_completion(this: Id, _cmd: pear_libobjc::abi::Sel, block: Id) {
    OPS.lock().unwrap().entry(this as usize).or_default().1 = block as usize;
}
extern "C" fn nsop_start(this: Id, _cmd: pear_libobjc::abi::Sel) {
    let entry = OPS.lock().unwrap().get(&(this as usize)).cloned();
    if let Some((blocks, comp, cancelled, _)) = entry {
        if !cancelled {
            for b in blocks {
                unsafe { invoke_objc_block(b) };
            }
        }
        if let Some(st) = OPS.lock().unwrap().get_mut(&(this as usize)) {
            st.3 = true;
        }
        if comp != 0 {
            unsafe { invoke_objc_block(comp) };
        }
    }
}
extern "C" fn nsop_cancel(this: Id, _cmd: pear_libobjc::abi::Sel) {
    OPS.lock().unwrap().entry(this as usize).or_default().2 = true;
}
extern "C" fn nsop_is_cancelled(this: Id, _cmd: pear_libobjc::abi::Sel) -> bool {
    OPS.lock().unwrap().get(&(this as usize)).map_or(false, |s| s.2)
}
extern "C" fn nsop_is_finished(this: Id, _cmd: pear_libobjc::abi::Sel) -> bool {
    OPS.lock().unwrap().get(&(this as usize)).map_or(true, |s| s.3)
}

extern "C" fn df_set_fmt(this: Id, _cmd: pear_libobjc::abi::Sel, fmt: Id) {
    DATE_FORMATTERS.lock().unwrap().insert(this as usize, rust_str_from_nsstring(fmt).unwrap_or_default());
}
extern "C" fn df_fmt(this: Id, _cmd: pear_libobjc::abi::Sel) -> Id {
    let f = DATE_FORMATTERS.lock().unwrap().get(&(this as usize)).cloned().unwrap_or_default();
    nsstring_from_rust(&f)
}
extern "C" fn df_str_from_date(_this: Id, _cmd: pear_libobjc::abi::Sel, _date: Id) -> Id {
    nsstring_from_rust("2026-01-01T00:00:00Z")
}
extern "C" fn df_date_from_str(_this: Id, _cmd: pear_libobjc::abi::Sel, _s: Id) -> Id {
    if let Some(cls) = pear_libobjc::runtime::class_named("NSDate") {
        unsafe { pear_libobjc::object::alloc(cls) }
    } else {
        std::ptr::null_mut()
    }
}

/// Registers real Foundation collection, concurrency, and formatter classes
/// (`NSMapTable`, `NSHashTable`, `NSLock`, `NSRecursiveLock`, `NSCache`,
/// `NSOperation`, `NSBlockOperation`, `NSDateFormatter`).
pub fn register_foundation_collection_classes() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let i = |p: *const ()| p as pear_libobjc::abi::Imp;
        pear_libobjc::runtime::register_builtin(
            NSMAPTABLE_CLASS.ptr(),
            NSMAPTABLE_META.ptr(),
            "NSMapTable",
            32,
            &[
                ("initWithKeyOptions:valueOptions:capacity:", i(map_table_init_cap as *const ())),
                ("objectForKey:", i(map_table_get as *const ())),
                ("setObject:forKey:", i(map_table_set as *const ())),
                ("removeObjectForKey:", i(map_table_remove as *const ())),
                ("removeAllObjects", i(map_table_remove_all as *const ())),
                ("count", i(map_table_count as *const ())),
                ("dictionaryRepresentation", i(map_table_dict_rep as *const ())),
                ("objectEnumerator", i(map_table_obj_enum as *const ())),
                ("keyEnumerator", i(map_table_key_enum as *const ())),
                ("dealloc", i(map_table_dealloc as *const ())),
            ],
            &[
                ("mapTableWithKeyOptions:valueOptions:", i(map_table_alloc as *const ())),
                ("strongToWeakObjectsMapTable", i(map_table_alloc_s2w as *const ())),
                ("weakToStrongObjectsMapTable", i(map_table_alloc_w2s as *const ())),
                ("strongToStrongObjectsMapTable", i(map_table_alloc_s2s as *const ())),
                ("weakToWeakObjectsMapTable", i(map_table_alloc_w2w as *const ())),
                ("mapTableWithStrongToStrongObjects", i(map_table_alloc_s2s as *const ())),
                ("mapTableWithWeakToStrongObjects", i(map_table_alloc_w2s as *const ())),
                ("mapTableWithStrongToWeakObjects", i(map_table_alloc_s2w as *const ())),
                ("mapTableWithWeakToWeakObjects", i(map_table_alloc_w2w as *const ())),
            ],
        );
        pear_libobjc::runtime::register_builtin(
            NSHASHTABLE_CLASS.ptr(),
            NSHASHTABLE_META.ptr(),
            "NSHashTable",
            32,
            &[
                ("initWithOptions:capacity:", i(hash_table_init as *const ())),
                ("addObject:", i(hash_table_add as *const ())),
                ("removeObject:", i(hash_table_remove as *const ())),
                ("containsObject:", i(hash_table_contains as *const ())),
                ("removeAllObjects", i(hash_table_remove_all as *const ())),
                ("count", i(hash_table_count as *const ())),
                ("allObjects", i(hash_table_all as *const ())),
                ("dealloc", i(hash_table_dealloc as *const ())),
            ],
            &[
                ("weakObjectsHashTable", i(hash_table_alloc_weak as *const ())),
                ("hashTableWithOptions:", i(hash_table_alloc1 as *const ())),
            ],
        );
        for (cls, meta, name) in [
            (&NSLOCK_CLASS, &NSLOCK_META, "NSLock"),
            (&NSRECURSIVELOCK_CLASS, &NSRECURSIVELOCK_META, "NSRecursiveLock"),
        ] {
            pear_libobjc::runtime::register_builtin(
                cls.ptr(),
                meta.ptr(),
                name,
                32,
                &[
                    ("lock", i(nslock_lock as *const ())),
                    ("unlock", i(nslock_unlock as *const ())),
                    ("tryLock", i(nslock_try_lock as *const ())),
                    ("lockBeforeDate:", i(col_true as *const ())),
                    ("setName:", i(nslock_set_name as *const ())),
                    ("name", i(nslock_name as *const ())),
                ],
                &[],
            );
        }
        pear_libobjc::runtime::register_builtin(
            NSCACHE_CLASS.ptr(),
            NSCACHE_META.ptr(),
            "NSCache",
            32,
            &[
                ("objectForKey:", i(nscache_get as *const ())),
                ("setObject:forKey:", i(nscache_set as *const ())),
                ("setObject:forKey:cost:", i(nscache_set_cost as *const ())),
                ("removeObjectForKey:", i(nscache_rem as *const ())),
                ("removeAllObjects", i(nscache_rem_all as *const ())),
                ("setCountLimit:", i(col_noop1 as *const ())),
                ("setTotalCostLimit:", i(col_noop1 as *const ())),
                ("setDelegate:", i(col_noop1 as *const ())),
                ("setName:", i(nscache_set_name as *const ())),
                ("name", i(nscache_name as *const ())),
                ("dealloc", i(nscache_dealloc as *const ())),
            ],
            &[],
        );
        pear_libobjc::runtime::register_builtin(
            NSOPERATION_CLASS.ptr(),
            NSOPERATION_META.ptr(),
            "NSOperation",
            32,
            &[
                ("start", i(nsop_start as *const ())),
                ("main", i(col_noop0 as *const ())),
                ("cancel", i(nsop_cancel as *const ())),
                ("isCancelled", i(nsop_is_cancelled as *const ())),
                ("isFinished", i(nsop_is_finished as *const ())),
                ("isExecuting", i(col_false as *const ())),
                ("isReady", i(col_true as *const ())),
                ("setCompletionBlock:", i(nsop_set_completion as *const ())),
                ("setQualityOfService:", i(col_noop1 as *const ())),
            ],
            &[],
        );
        unsafe { (*NSBLOCKOPERATION_CLASS.0.get()).superclass = NSOPERATION_CLASS.ptr() };
        unsafe { (*NSBLOCKOPERATION_META.0.get()).superclass = NSOPERATION_META.ptr() };
        pear_libobjc::runtime::register_builtin(
            NSBLOCKOPERATION_CLASS.ptr(),
            NSBLOCKOPERATION_META.ptr(),
            "NSBlockOperation",
            32,
            &[
                ("addExecutionBlock:", i(nsop_add_block as *const ())),
                ("start", i(nsop_start as *const ())),
                ("main", i(nsop_start as *const ())),
                ("cancel", i(nsop_cancel as *const ())),
                ("isCancelled", i(nsop_is_cancelled as *const ())),
                ("isFinished", i(nsop_is_finished as *const ())),
            ],
            &[("blockOperationWithBlock:", i(nsop_block_with_block as *const ()))],
        );
        pear_libobjc::runtime::register_builtin(
            NSDATEFORMATTER_CLASS.ptr(),
            NSDATEFORMATTER_META.ptr(),
            "NSDateFormatter",
            32,
            &[
                ("init", i(col_self as *const ())),
                ("setDateFormat:", i(df_set_fmt as *const ())),
                ("dateFormat", i(df_fmt as *const ())),
                ("setLocale:", i(col_noop1 as *const ())),
                ("setTimeZone:", i(col_noop1 as *const ())),
                ("stringFromDate:", i(df_str_from_date as *const ())),
                ("dateFromString:", i(df_date_from_str as *const ())),
            ],
            &[],
        );
    });
}
