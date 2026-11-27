//! Clean-room `NSError`, `NSURLResponse`, `NSHTTPURLResponse`, `NSURLSession`,
//! and `NSURLSessionTask` / `DataTask` / `UploadTask` / `DownloadTask` stack
//! backed by system `curl` for portable HTTP/HTTPS on macOS and Linux.

use crate::{
    alloc_with_backing, f_dealloc, nsdata_from_bytes, nsdict_from_pairs, nsstring_from_rust,
    req_body, req_method, req_timeout, req_url, req_with_url, rust_bytes_from_nsdata,
    rust_str_from_nsstring, store, Backing, NSERROR_CLASS, NSHTTPURLRESPONSE_CLASS,
    NSHTTPURLRESPONSE_META, NSMUTABLEURLREQUEST_CLASS, NSURLREQUEST_CLASS, NSURLRESPONSE_CLASS,
    NSURLRESPONSE_META, NSURLSESSIONCONFIGURATION_CLASS, NSURLSESSIONDATATASK_CLASS,
    NSURLSESSIONDATATASK_META, NSURLSESSIONDOWNLOADTASK_CLASS, NSURLSESSIONDOWNLOADTASK_META,
    NSURLSESSIONTASK_CLASS, NSURLSESSIONTASK_META, NSURLSESSIONUPLOADTASK_CLASS,
    NSURLSESSIONUPLOADTASK_META, NSURLSESSION_CLASS, NSURL_CLASS,
};
use pear_libobjc::abi::{ClassT, Id, Imp, Sel};
use pear_libobjc::message::lookup_for_send;
use pear_libobjc::object::{dispose, release, retain};
use pear_libobjc::runtime::{self, object_class, sel};
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static NET_TRACE_ENABLED: OnceLock<bool> = OnceLock::new();

pub fn net_trace_enabled() -> bool {
    *NET_TRACE_ENABLED.get_or_init(|| std::env::var("PEAR_NET_TRACE").as_deref() == Ok("1"))
}

#[inline]
fn i(p: *const ()) -> Imp {
    unsafe { std::mem::transmute(p) }
}

// ---- NSError ---------------------------------------------------------------

#[derive(Clone)]
struct ErrorState {
    domain: String,
    code: isize,
    user_info: usize,
}

static ERRORS: OnceLock<Mutex<HashMap<usize, ErrorState>>> = OnceLock::new();

fn errors() -> &'static Mutex<HashMap<usize, ErrorState>> {
    ERRORS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn make_nserror(domain: &str, code: isize, desc: &str) -> Id {
    let info = nsdict_from_pairs(vec![(
        "NSLocalizedDescription".to_string(),
        nsstring_from_rust(desc),
    )]);
    let obj = unsafe { alloc_with_backing(NSERROR_CLASS.ptr(), Backing::Arr(Vec::new())) };
    errors().lock().unwrap().insert(
        obj as usize,
        ErrorState {
            domain: domain.to_string(),
            code,
            user_info: retain(info) as usize,
        },
    );
    obj
}

pub(crate) extern "C" fn err_init_domain_code_userinfo(
    this: Id,
    _cmd: Sel,
    domain: Id,
    code: isize,
    info: Id,
) -> Id {
    let dom = rust_str_from_nsstring(domain).unwrap_or_default();
    let ui = if info.is_null() {
        0
    } else {
        retain(info) as usize
    };
    let prev = errors().lock().unwrap().insert(
        this as usize,
        ErrorState {
            domain: dom,
            code,
            user_info: ui,
        },
    );
    if let Some(old) = prev {
        if old.user_info != 0 {
            release(old.user_info as Id);
        }
    }
    this
}

pub(crate) extern "C" fn err_with_domain_code_userinfo(
    _cls: Id,
    cmd: Sel,
    domain: Id,
    code: isize,
    info: Id,
) -> Id {
    let obj = unsafe { alloc_with_backing(NSERROR_CLASS.ptr(), Backing::Arr(Vec::new())) };
    err_init_domain_code_userinfo(obj, cmd, domain, code, info)
}

pub(crate) extern "C" fn err_domain(this: Id, _cmd: Sel) -> Id {
    let dom = errors()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| e.domain.clone())
        .unwrap_or_default();
    nsstring_from_rust(&dom)
}

pub(crate) extern "C" fn err_code(this: Id, _cmd: Sel) -> isize {
    errors()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| e.code)
        .unwrap_or(0)
}

pub(crate) extern "C" fn err_userinfo(this: Id, _cmd: Sel) -> Id {
    errors()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| e.user_info as Id)
        .unwrap_or(std::ptr::null_mut())
}

pub(crate) extern "C" fn err_localized_desc(this: Id, _cmd: Sel) -> Id {
    let info_opt = errors()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|e| (e.domain.clone(), e.code, e.user_info));
    let Some((dom, code, ui)) = info_opt else {
        return nsstring_from_rust("NSError");
    };
    if ui != 0 {
        if let Some(v) = crate::nsdict_lookup_str(ui as Id, "NSLocalizedDescription") {
            if let Some(s) = rust_str_from_nsstring(v) {
                return nsstring_from_rust(&s);
            }
        }
    }
    nsstring_from_rust(&format!("The operation couldn't be completed. ({dom} error {code}.)"))
}

pub(crate) extern "C" fn err_dealloc(this: Id, cmd: Sel) {
    if let Some(old) = errors().lock().unwrap().remove(&(this as usize)) {
        if old.user_info != 0 {
            release(old.user_info as Id);
        }
    }
    f_dealloc(this, cmd);
}

// ---- NSURLResponse / NSHTTPURLResponse -------------------------------------

#[derive(Clone)]
struct UrlResponseState {
    url: usize,
    mime_type: String,
    expected_len: i64,
    text_encoding: String,
    status_code: isize,
    headers: Vec<(String, String)>,
    headers_dict: usize,
}

static URL_RESPONSES: OnceLock<Mutex<HashMap<usize, UrlResponseState>>> = OnceLock::new();

fn url_responses() -> &'static Mutex<HashMap<usize, UrlResponseState>> {
    URL_RESPONSES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_headers_dict(headers: &[(String, String)]) -> usize {
    let pairs: Vec<(String, Id)> = headers
        .iter()
        .map(|(k, v)| (k.clone(), nsstring_from_rust(v)))
        .collect();
    let d = nsdict_from_pairs(pairs);
    retain(d) as usize
}

fn make_http_response(
    url_obj: Id,
    status_code: isize,
    headers: Vec<(String, String)>,
    body_len: usize,
) -> Id {
    let mut mime = String::from("application/octet-stream");
    let mut encoding = String::new();
    let mut expected_len = body_len as i64;
    for (k, v) in &headers {
        if k.eq_ignore_ascii_case("content-type") {
            let mut parts = v.split(';');
            if let Some(m) = parts.next() {
                let mt = m.trim();
                if !mt.is_empty() {
                    mime = mt.to_string();
                }
            }
            for param in parts {
                let p = param.trim();
                if let Some(cs) = p
                    .strip_prefix("charset=")
                    .or_else(|| p.strip_prefix("CHARSET="))
                {
                    encoding = cs.trim_matches('"').trim().to_string();
                }
            }
        } else if k.eq_ignore_ascii_case("content-length") {
            if let Ok(n) = v.trim().parse::<i64>() {
                expected_len = n;
            }
        }
    }
    let headers_dict = build_headers_dict(&headers);
    let url_id = if url_obj.is_null() {
        0
    } else {
        retain(url_obj) as usize
    };
    let obj = unsafe { alloc_with_backing(NSHTTPURLRESPONSE_CLASS.ptr(), Backing::Arr(Vec::new())) };
    url_responses().lock().unwrap().insert(
        obj as usize,
        UrlResponseState {
            url: url_id,
            mime_type: mime,
            expected_len,
            text_encoding: encoding,
            status_code,
            headers,
            headers_dict,
        },
    );
    obj
}

extern "C" fn resp_init(
    this: Id,
    _cmd: Sel,
    url: Id,
    mime: Id,
    expected_len: i64,
    enc: Id,
) -> Id {
    let url_id = if url.is_null() { 0 } else { retain(url) as usize };
    let mime_type = rust_str_from_nsstring(mime).unwrap_or_default();
    let text_encoding = rust_str_from_nsstring(enc).unwrap_or_default();
    let prev = url_responses().lock().unwrap().insert(
        this as usize,
        UrlResponseState {
            url: url_id,
            mime_type,
            expected_len,
            text_encoding,
            status_code: 200,
            headers: Vec::new(),
            headers_dict: 0,
        },
    );
    if let Some(old) = prev {
        if old.url != 0 {
            release(old.url as Id);
        }
        if old.headers_dict != 0 {
            release(old.headers_dict as Id);
        }
    }
    this
}

extern "C" fn http_resp_init(
    this: Id,
    _cmd: Sel,
    url: Id,
    status: isize,
    _ver: Id,
    hdrs: Id,
) -> Id {
    let mut headers = Vec::new();
    if let Some(entries) = crate::nsdict_entries(hdrs) {
        for (k, v) in entries {
            if let Some(vs) = rust_str_from_nsstring(v) {
                headers.push((k, vs));
            }
        }
    }
    let headers_dict = if hdrs.is_null() {
        build_headers_dict(&headers)
    } else {
        retain(hdrs) as usize
    };
    let url_id = if url.is_null() { 0 } else { retain(url) as usize };
    let prev = url_responses().lock().unwrap().insert(
        this as usize,
        UrlResponseState {
            url: url_id,
            mime_type: String::new(),
            expected_len: -1,
            text_encoding: String::new(),
            status_code: status,
            headers,
            headers_dict,
        },
    );
    if let Some(old) = prev {
        if old.url != 0 {
            release(old.url as Id);
        }
        if old.headers_dict != 0 {
            release(old.headers_dict as Id);
        }
    }
    this
}

extern "C" fn resp_url(this: Id, _cmd: Sel) -> Id {
    url_responses()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|r| r.url as Id)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn resp_mime_type(this: Id, _cmd: Sel) -> Id {
    let s = url_responses()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|r| r.mime_type.clone())
        .unwrap_or_default();
    if s.is_empty() {
        std::ptr::null_mut()
    } else {
        nsstring_from_rust(&s)
    }
}

extern "C" fn resp_expected_len(this: Id, _cmd: Sel) -> i64 {
    url_responses()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|r| r.expected_len)
        .unwrap_or(-1)
}

extern "C" fn resp_text_encoding(this: Id, _cmd: Sel) -> Id {
    let s = url_responses()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|r| r.text_encoding.clone())
        .unwrap_or_default();
    if s.is_empty() {
        std::ptr::null_mut()
    } else {
        nsstring_from_rust(&s)
    }
}

extern "C" fn resp_suggested_filename(this: Id, _cmd: Sel) -> Id {
    let url_id = resp_url(this, _cmd);
    if !url_id.is_null() {
        let s = extract_url_string(url_id);
        if let Some(last) = s.split('?').next().and_then(|p| p.rsplit('/').next()) {
            if !last.is_empty() {
                return nsstring_from_rust(last);
            }
        }
    }
    nsstring_from_rust("Unknown")
}

extern "C" fn http_resp_status_code(this: Id, _cmd: Sel) -> isize {
    url_responses()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|r| r.status_code)
        .unwrap_or(0)
}

extern "C" fn http_resp_all_headers(this: Id, _cmd: Sel) -> Id {
    url_responses()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|r| r.headers_dict as Id)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn http_resp_header_for_field(this: Id, _cmd: Sel, field: Id) -> Id {
    let Some(name) = rust_str_from_nsstring(field) else {
        return std::ptr::null_mut();
    };
    let found = url_responses().lock().unwrap().get(&(this as usize)).and_then(|r| {
        r.headers
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(&name))
            .map(|(_, v)| v.clone())
    });
    match found {
        Some(v) => nsstring_from_rust(&v),
        None => std::ptr::null_mut(),
    }
}

extern "C" fn http_resp_localized_status(_cls: Id, _cmd: Sel, code: isize) -> Id {
    let msg = match code {
        200 => "no error",
        201 => "created",
        204 => "no content",
        301 => "moved permanently",
        302 => "found",
        304 => "not modified",
        400 => "bad request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not found",
        500 => "internal server error",
        502 => "bad gateway",
        503 => "service unavailable",
        _ => "http status",
    };
    nsstring_from_rust(msg)
}

extern "C" fn resp_dealloc(this: Id, cmd: Sel) {
    if let Some(old) = url_responses().lock().unwrap().remove(&(this as usize)) {
        if old.url != 0 {
            release(old.url as Id);
        }
        if old.headers_dict != 0 {
            release(old.headers_dict as Id);
        }
    }
    f_dealloc(this, cmd);
}

// ---- NSURLSession & NSURLSessionTask ---------------------------------------

#[derive(Clone)]
struct SessionState {
    config: usize,
    delegate: usize,
    delegate_queue: usize,
    description: usize,
    callback_lock: Arc<Mutex<()>>,
}

static URL_SESSIONS: OnceLock<Mutex<HashMap<usize, SessionState>>> = OnceLock::new();

fn url_sessions() -> &'static Mutex<HashMap<usize, SessionState>> {
    URL_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct UrlTaskState {
    task_id: usize,
    session: usize,
    delegate: usize,
    callback_lock: Arc<Mutex<()>>,
    request: usize,
    url_obj: usize,
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    timeout_secs: u64,
    is_download: bool,
    completion_block: usize,
    state: usize, // 0=Running, 1=Suspended, 2=Canceling, 3=Completed
    started: bool,
    cancelled: bool,
    priority: f32,
    description: usize,
    response: usize,
    error: usize,
    bytes_received: i64,
    bytes_expected_recv: i64,
    bytes_sent: i64,
    bytes_expected_send: i64,
}

static URL_TASKS: OnceLock<Mutex<HashMap<usize, Arc<Mutex<UrlTaskState>>>>> = OnceLock::new();
static TASK_ID_SEQ: AtomicUsize = AtomicUsize::new(1);

fn url_tasks() -> &'static Mutex<HashMap<usize, Arc<Mutex<UrlTaskState>>>> {
    URL_TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) extern "C" fn sess_shared(_cls: Id, _cmd: Sel) -> Id {
    static S: OnceLock<usize> = OnceLock::new();
    *S.get_or_init(|| {
        let obj = unsafe {
            alloc_with_backing(NSURLSESSION_CLASS.ptr(), Backing::Dict(HashMap::new()))
        };
        url_sessions().lock().unwrap().insert(
            obj as usize,
            SessionState {
                config: 0,
                delegate: 0,
                delegate_queue: 0,
                description: 0,
                callback_lock: Arc::new(Mutex::new(())),
            },
        );
        obj as usize
    }) as Id
}

pub(crate) extern "C" fn sess_with_cfg(cls: Id, cmd: Sel, cfg: Id) -> Id {
    sess_with_cfg3(cls, cmd, cfg, std::ptr::null_mut(), std::ptr::null_mut())
}

pub(crate) extern "C" fn sess_with_cfg3(
    _cls: Id,
    _cmd: Sel,
    cfg: Id,
    del: Id,
    q: Id,
) -> Id {
    let obj = unsafe { alloc_with_backing(NSURLSESSION_CLASS.ptr(), Backing::Dict(HashMap::new())) };
    url_sessions().lock().unwrap().insert(
        obj as usize,
        SessionState {
            config: if cfg.is_null() { 0 } else { retain(cfg) as usize },
            delegate: if del.is_null() { 0 } else { retain(del) as usize },
            delegate_queue: if q.is_null() { 0 } else { retain(q) as usize },
            description: 0,
            callback_lock: Arc::new(Mutex::new(())),
        },
    );
    obj
}

pub(crate) extern "C" fn sess_delegate(this: Id, _cmd: Sel) -> Id {
    url_sessions()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.delegate as Id)
        .unwrap_or(std::ptr::null_mut())
}

pub(crate) extern "C" fn sess_delegate_queue(this: Id, _cmd: Sel) -> Id {
    url_sessions()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.delegate_queue as Id)
        .unwrap_or(std::ptr::null_mut())
}

pub(crate) extern "C" fn sess_configuration(this: Id, _cmd: Sel) -> Id {
    let cfg = url_sessions()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.config as Id)
        .unwrap_or(std::ptr::null_mut());
    if cfg.is_null() {
        unsafe { alloc_with_backing(NSURLSESSIONCONFIGURATION_CLASS.ptr(), Backing::Dict(HashMap::new())) }
    } else {
        cfg
    }
}

pub(crate) extern "C" fn sess_description(this: Id, _cmd: Sel) -> Id {
    url_sessions()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.description as Id)
        .unwrap_or(std::ptr::null_mut())
}

pub(crate) extern "C" fn sess_set_description(this: Id, _cmd: Sel, desc: Id) {
    let new_d = if desc.is_null() {
        0
    } else {
        retain(desc) as usize
    };
    let mut g = url_sessions().lock().unwrap();
    if let Some(s) = g.get_mut(&(this as usize)) {
        let old = s.description;
        s.description = new_d;
        drop(g);
        if old != 0 {
            release(old as Id);
        }
    } else if new_d != 0 {
        drop(g);
        release(new_d as Id);
    }
}

pub(crate) extern "C" fn sess_invalidate(this: Id, _cmd: Sel) {
    let del = url_sessions()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.delegate as Id)
        .unwrap_or(std::ptr::null_mut());
    if !del.is_null() {
        let s = sel("URLSession:didBecomeInvalidWithError:");
        let cls = unsafe { object_class(del) };
        if !cls.is_null() && runtime::lookup(cls, s).is_some() {
            unsafe {
                let d = lookup_for_send(del, s);
                let f: extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(d.imp);
                f(d.receiver, s, this, std::ptr::null_mut());
            }
        }
    }
}

pub(crate) extern "C" fn sess_dealloc(this: Id, cmd: Sel) {
    if let Some(old) = url_sessions().lock().unwrap().remove(&(this as usize)) {
        if old.config != 0 {
            release(old.config as Id);
        }
        if old.delegate != 0 {
            release(old.delegate as Id);
        }
        if old.delegate_queue != 0 {
            release(old.delegate_queue as Id);
        }
        if old.description != 0 {
            release(old.description as Id);
        }
    }
    f_dealloc(this, cmd);
}

fn extract_url_string(url_obj: Id) -> String {
    if url_obj.is_null() {
        return String::new();
    }
    if let Some(s) = rust_str_from_nsstring(url_obj) {
        return s;
    }
    let s_abs = sel("absoluteString");
    let cls = unsafe { object_class(url_obj) };
    if !cls.is_null() && runtime::lookup(cls, s_abs).is_some() {
        unsafe {
            let d = lookup_for_send(url_obj, s_abs);
            let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
            let abs = f(d.receiver, s_abs);
            if let Some(s) = rust_str_from_nsstring(abs) {
                return s;
            }
        }
    }
    String::new()
}

fn is_request_instance(obj: Id) -> bool {
    if obj.is_null() {
        return false;
    }
    let mut cls = unsafe { object_class(obj) };
    while !cls.is_null() {
        if cls == NSURLREQUEST_CLASS.ptr() || cls == NSMUTABLEURLREQUEST_CLASS.ptr() {
            return true;
        }
        cls = unsafe { (*cls).superclass };
    }
    false
}

fn create_task_common(
    session: Id,
    task_cls: *const ClassT,
    req_or_url: Id,
    explicit_method: Option<&str>,
    explicit_body: Option<Vec<u8>>,
    is_download: bool,
    cb: *const c_void,
) -> Id {
    if session.is_null() {
        return std::ptr::null_mut();
    }
    let req_obj = if is_request_instance(req_or_url) {
        retain(req_or_url)
    } else if !req_or_url.is_null() {
        req_with_url(NSURLREQUEST_CLASS.ptr() as Id, sel("requestWithURL:"), req_or_url)
    } else {
        std::ptr::null_mut()
    };

    let url_obj = if !req_obj.is_null() {
        let u = req_url(req_obj, sel("URL"));
        if !u.is_null() {
            u
        } else if !is_request_instance(req_or_url) {
            req_or_url
        } else {
            std::ptr::null_mut()
        }
    } else {
        std::ptr::null_mut()
    };
    let url_str = extract_url_string(url_obj);

    let method = if let Some(m) = explicit_method {
        m.to_string()
    } else if !req_obj.is_null() {
        rust_str_from_nsstring(req_method(req_obj, sel("HTTPMethod")))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "GET".to_string())
    } else {
        "GET".to_string()
    };

    let (config_id, delegate_id, callback_lock) = url_sessions()
        .lock()
        .unwrap()
        .get(&(session as usize))
        .map(|s| (s.config, s.delegate, Arc::clone(&s.callback_lock)))
        .unwrap_or_else(|| (0, 0, Arc::new(Mutex::new(()))));

    let mut headers: Vec<(String, String)> = Vec::new();
    if config_id != 0 {
        let cfg_hdrs_obj = crate::nsdict_lookup_str(config_id as Id, "additional_headers")
            .unwrap_or(std::ptr::null_mut());
        if let Some(map) = crate::nsdict_entries(cfg_hdrs_obj) {
            for (k, v) in map {
                if let Some(vs) = rust_str_from_nsstring(v) {
                    headers.push((k, vs));
                }
            }
        }
    }
    if !req_obj.is_null() {
        for (k, v) in crate::extract_request_headers(req_obj) {
            if let Some(existing) = headers.iter_mut().find(|(ek, _)| ek.eq_ignore_ascii_case(&k)) {
                existing.1 = v;
            } else {
                headers.push((k, v));
            }
        }
    }

    let body = if let Some(b) = explicit_body {
        b
    } else if !req_obj.is_null() {
        let b_id = req_body(req_obj, sel("HTTPBody"));
        rust_bytes_from_nsdata(b_id).unwrap_or_default()
    } else {
        Vec::new()
    };

    let timeout_secs = if !req_obj.is_null() {
        let t = req_timeout(req_obj, sel("timeoutInterval"));
        if t.is_finite() && t > 0.0 {
            (t.round() as u64).clamp(3, 25)
        } else {
            15
        }
    } else {
        15
    };

    let completion_block = if cb.is_null() {
        0
    } else {
        unsafe { pear_libobjc::blocks::block_copy(cb) as usize }
    };

    if net_trace_enabled() {
        eprintln!(
            "[PearRuntime][net][NSURLSession] create method={} url={} has_cb={} has_delegate={} dl={}",
            method,
            url_str,
            completion_block != 0,
            delegate_id != 0,
            is_download
        );
    }

    let task_id = TASK_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let task_obj = unsafe { alloc_with_backing(task_cls, Backing::Arr(Vec::new())) };
    let st = Arc::new(Mutex::new(UrlTaskState {
        task_id,
        session: retain(session) as usize,
        delegate: if delegate_id == 0 {
            0
        } else {
            retain(delegate_id as Id) as usize
        },
        callback_lock,
        request: req_obj as usize,
        url_obj: if url_obj.is_null() {
            0
        } else {
            retain(url_obj) as usize
        },
        url: url_str,
        method,
        headers,
        body,
        timeout_secs,
        is_download,
        completion_block,
        state: 1, // NSURLSessionTaskStateSuspended
        started: false,
        cancelled: false,
        priority: 0.5,
        description: 0,
        response: 0,
        error: 0,
        bytes_received: 0,
        bytes_expected_recv: -1,
        bytes_sent: 0,
        bytes_expected_send: 0,
    }));
    url_tasks().lock().unwrap().insert(task_obj as usize, st);
    task_obj
}

pub(crate) extern "C" fn sess_data_task(this: Id, _cmd: Sel, req: Id) -> Id {
    create_task_common(
        this,
        NSURLSESSIONDATATASK_CLASS.ptr(),
        req,
        None,
        None,
        false,
        std::ptr::null(),
    )
}

pub(crate) extern "C" fn sess_data_task_cb(
    this: Id,
    _cmd: Sel,
    req: Id,
    cb: *const c_void,
) -> Id {
    create_task_common(
        this,
        NSURLSESSIONDATATASK_CLASS.ptr(),
        req,
        None,
        None,
        false,
        cb,
    )
}

pub(crate) extern "C" fn sess_upload_data(this: Id, _cmd: Sel, req: Id, data: Id) -> Id {
    let body = rust_bytes_from_nsdata(data).unwrap_or_default();
    create_task_common(
        this,
        NSURLSESSIONUPLOADTASK_CLASS.ptr(),
        req,
        None,
        Some(body),
        false,
        std::ptr::null(),
    )
}

pub(crate) extern "C" fn sess_upload_data_cb(
    this: Id,
    _cmd: Sel,
    req: Id,
    data: Id,
    cb: *const c_void,
) -> Id {
    let body = rust_bytes_from_nsdata(data).unwrap_or_default();
    create_task_common(
        this,
        NSURLSESSIONUPLOADTASK_CLASS.ptr(),
        req,
        None,
        Some(body),
        false,
        cb,
    )
}

pub(crate) extern "C" fn sess_upload_file(this: Id, _cmd: Sel, req: Id, file_url: Id) -> Id {
    let p_str = if !file_url.is_null() {
        let s_path = sel("path");
        let cls = unsafe { object_class(file_url) };
        if !cls.is_null() && runtime::lookup(cls, s_path).is_some() {
            unsafe {
                let d = lookup_for_send(file_url, s_path);
                let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
                rust_str_from_nsstring(f(d.receiver, s_path)).unwrap_or_default()
            }
        } else {
            extract_url_string(file_url)
        }
    } else {
        String::new()
    };
    let clean_path = p_str.strip_prefix("file://").unwrap_or(&p_str);
    let body = std::fs::read(clean_path).unwrap_or_default();
    create_task_common(
        this,
        NSURLSESSIONUPLOADTASK_CLASS.ptr(),
        req,
        None,
        Some(body),
        false,
        std::ptr::null(),
    )
}

pub(crate) extern "C" fn sess_download_task(this: Id, _cmd: Sel, req: Id) -> Id {
    create_task_common(
        this,
        NSURLSESSIONDOWNLOADTASK_CLASS.ptr(),
        req,
        None,
        None,
        true,
        std::ptr::null(),
    )
}

pub(crate) extern "C" fn sess_download_task_cb(
    this: Id,
    _cmd: Sel,
    req: Id,
    cb: *const c_void,
) -> Id {
    create_task_common(
        this,
        NSURLSESSIONDOWNLOADTASK_CLASS.ptr(),
        req,
        None,
        None,
        true,
        cb,
    )
}

#[repr(C)]
struct DispositionBlock {
    isa: *const ClassT,
    flags: AtomicI32,
    reserved: i32,
    invoke: *const c_void,
    descriptor: *const pear_libobjc::blocks::BlockDescriptor1,
    disposition: AtomicIsize,
}

static DISPOSITION_DESC: pear_libobjc::blocks::BlockDescriptor1 =
    pear_libobjc::blocks::BlockDescriptor1 {
        reserved: 0,
        size: std::mem::size_of::<DispositionBlock>(),
    };

extern "C" fn disposition_block_invoke(blk: *mut c_void, disp: isize) {
    if !blk.is_null() {
        unsafe {
            (*(blk as *const DispositionBlock))
                .disposition
                .store(disp, Ordering::Release);
        }
    }
}

fn parse_curl_headers(raw: &str) -> (isize, Vec<(String, String)>) {
    let mut blocks: Vec<(isize, Vec<(String, String)>)> = Vec::new();
    let mut cur_status: Option<isize> = None;
    let mut cur_hdrs: Vec<(String, String)> = Vec::new();
    for line in raw.lines() {
        let l = line.trim_end_matches('\r');
        if l.starts_with("HTTP/") {
            if let Some(st) = cur_status.take() {
                blocks.push((st, std::mem::take(&mut cur_hdrs)));
            }
            let code = l
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<isize>().ok())
                .unwrap_or(200);
            cur_status = Some(code);
        } else if l.is_empty() {
            if let Some(st) = cur_status.take() {
                blocks.push((st, std::mem::take(&mut cur_hdrs)));
            }
        } else if cur_status.is_some() {
            if let Some((k, v)) = l.split_once(':') {
                cur_hdrs.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    if let Some(st) = cur_status.take() {
        blocks.push((st, cur_hdrs));
    }
    blocks.pop().unwrap_or((200, Vec::new()))
}

fn execute_http_via_curl(
    task_id: usize,
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: &[u8],
    timeout_secs: u64,
) -> Result<(isize, Vec<(String, String)>, Vec<u8>), (isize, String)> {
    if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
        return Err((-1002, format!("unsupported URL: {url}")));
    }
    let hdr_path = std::env::temp_dir().join(format!(
        "pear_curl_hdr_{}_{}.txt",
        std::process::id(),
        task_id
    ));
    let mut cmd = Command::new("curl");
    cmd.arg("-sS")
        .arg("-L")
        .arg("--connect-timeout")
        .arg("10")
        .arg("--max-time")
        .arg(timeout_secs.to_string())
        .arg("-X")
        .arg(method)
        .arg("-D")
        .arg(&hdr_path);
    for (k, v) in headers {
        cmd.arg("-H").arg(format!("{k}: {v}"));
    }
    if !body.is_empty() {
        cmd.arg("--data-binary").arg("@-").stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    cmd.arg("--").arg(url);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(&hdr_path);
            return Err((-1000, format!("curl spawn failed: {e}")));
        }
    };
    if !body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(body);
        }
    }
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            let _ = std::fs::remove_file(&hdr_path);
            return Err((-1000, format!("curl wait failed: {e}")));
        }
    };
    let hdr_raw = std::fs::read_to_string(&hdr_path).unwrap_or_default();
    let _ = std::fs::remove_file(&hdr_path);

    if !out.status.success() {
        let code_exit = out.status.code().unwrap_or(-1);
        let ns_code = match code_exit {
            28 => -1001,     // NSURLErrorTimedOut
            6 | 7 => -1004,  // NSURLErrorCannotConnectToHost
            _ => -1000,      // NSURLErrorUnknown
        };
        let err_msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let desc = if err_msg.is_empty() {
            format!("curl exited with code {code_exit}")
        } else {
            err_msg
        };
        return Err((ns_code, desc));
    }

    let (status_code, resp_hdrs) = parse_curl_headers(&hdr_raw);
    Ok((status_code, resp_hdrs, out.stdout))
}

fn task_worker(task_ptr: usize, task_arc: Arc<Mutex<UrlTaskState>>) {
    let (task_id, url, method, headers, body, timeout_secs, url_obj, session, delegate, cb_lock, is_download) = {
        let g = task_arc.lock().unwrap();
        (
            g.task_id,
            g.url.clone(),
            g.method.clone(),
            g.headers.clone(),
            g.body.clone(),
            g.timeout_secs,
            g.url_obj,
            g.session,
            g.delegate,
            Arc::clone(&g.callback_lock),
            g.is_download,
        )
    };

    let result = if task_arc.lock().unwrap().cancelled {
        Err((-999isize, "cancelled".to_string()))
    } else {
        execute_http_via_curl(task_id, &url, &method, &headers, &body, timeout_secs)
    };

    let (resp_id, err_id, body_bytes, cb_ptr) = {
        let mut g = task_arc.lock().unwrap();
        let cancelled = g.cancelled;
        let final_res = if cancelled {
            Err((-999isize, "cancelled".to_string()))
        } else {
            result
        };
        let (r_id, e_id, b_vec) = match final_res {
            Ok((status, hdrs, bytes)) => {
                if net_trace_enabled() {
                    eprintln!(
                        "[PearRuntime][net][NSURLSession] done status={} bytes={} url={}",
                        status,
                        bytes.len(),
                        url
                    );
                }
                let u_for_resp = if url_obj != 0 {
                    url_obj as Id
                } else {
                    nsstring_from_rust(&url)
                };
                let r = make_http_response(u_for_resp, status, hdrs, bytes.len());
                g.response = retain(r) as usize;
                g.bytes_sent = body.len() as i64;
                g.bytes_expected_send = body.len() as i64;
                g.bytes_received = bytes.len() as i64;
                g.bytes_expected_recv = bytes.len() as i64;
                (r, std::ptr::null_mut(), bytes)
            }
            Err((code, msg)) => {
                if net_trace_enabled() {
                    eprintln!(
                        "[PearRuntime][net][NSURLSession] err code={} msg={:?} url={}",
                        code, msg, url
                    );
                }
                let e = make_nserror("NSURLErrorDomain", code, &msg);
                g.error = retain(e) as usize;
                (std::ptr::null_mut(), e, Vec::new())
            }
        };
        let cb = g.completion_block;
        g.completion_block = 0;
        if cb != 0 {
            g.state = 3; // NSURLSessionTaskStateCompleted
        }
        (r_id, e_id, b_vec, cb)
    };

    if cb_ptr != 0 {
        let arg1: Id = if !err_id.is_null() {
            std::ptr::null_mut()
        } else if is_download {
            let dl_path = std::env::temp_dir().join(format!(
                "pear_download_{}_{}.bin",
                std::process::id(),
                task_id
            ));
            let _ = std::fs::write(&dl_path, &body_bytes);
            let p_str = nsstring_from_rust(&dl_path.to_string_lossy());
            unsafe { alloc_with_backing(NSURL_CLASS.ptr(), Backing::Str(rust_str_from_nsstring(p_str).unwrap_or_default())) }
        } else {
            nsdata_from_bytes(&body_bytes)
        };
        unsafe {
            let blk = cb_ptr as *const c_void;
            let layout = &*(blk as *const pear_libobjc::blocks::BlockLayout);
            if !layout.invoke.is_null() {
                let f: extern "C" fn(*const c_void, Id, Id, Id) = std::mem::transmute(layout.invoke);
                f(blk, arg1, resp_id, err_id);
            }
            pear_libobjc::blocks::block_release(blk);
        }
    } else if delegate != 0 {
        let _cb_guard = cb_lock.lock().unwrap();
        let del = delegate as Id;
        let del_cls = unsafe { object_class(del) };
        let sess = session as Id;
        let task_id_ptr = task_ptr as Id;

        if err_id.is_null() {
            let mut allow = true;
            if !is_download {
                let s_resp = sel("URLSession:dataTask:didReceiveResponse:completionHandler:");
                if !del_cls.is_null() && runtime::lookup(del_cls, s_resp).is_some() {
                    let disp_blk = Box::into_raw(Box::new(DispositionBlock {
                        isa: pear_libobjc::blocks::NSGLOBALBLOCK_CLASS.ptr(),
                        flags: AtomicI32::new(1 << 28),
                        reserved: 0,
                        invoke: disposition_block_invoke as *const c_void,
                        descriptor: &DISPOSITION_DESC,
                        disposition: AtomicIsize::new(1),
                    }));
                    unsafe {
                        let d = lookup_for_send(del, s_resp);
                        let f: extern "C" fn(Id, Sel, Id, Id, Id, *const c_void) =
                            std::mem::transmute(d.imp);
                        f(d.receiver, s_resp, sess, task_id_ptr, resp_id, disp_blk as *const c_void);
                        allow = (*disp_blk).disposition.load(Ordering::Acquire) != 0;
                    }
                }
                if allow && !body_bytes.is_empty() {
                    let s_data = sel("URLSession:dataTask:didReceiveData:");
                    if !del_cls.is_null() && runtime::lookup(del_cls, s_data).is_some() {
                        let data_obj = nsdata_from_bytes(&body_bytes);
                        unsafe {
                            let d = lookup_for_send(del, s_data);
                            let f: extern "C" fn(Id, Sel, Id, Id, Id) = std::mem::transmute(d.imp);
                            f(d.receiver, s_data, sess, task_id_ptr, data_obj);
                        }
                    }
                }
            } else {
                let s_dl = sel("URLSession:downloadTask:didFinishDownloadingToURL:");
                if !del_cls.is_null() && runtime::lookup(del_cls, s_dl).is_some() {
                    let dl_path = std::env::temp_dir().join(format!(
                        "pear_download_{}_{}.bin",
                        std::process::id(),
                        task_id
                    ));
                    let _ = std::fs::write(&dl_path, &body_bytes);
                    let url_dl = unsafe {
                        alloc_with_backing(
                            NSURL_CLASS.ptr(),
                            Backing::Str(dl_path.to_string_lossy().into_owned()),
                        )
                    };
                    unsafe {
                        let d = lookup_for_send(del, s_dl);
                        let f: extern "C" fn(Id, Sel, Id, Id, Id) = std::mem::transmute(d.imp);
                        f(d.receiver, s_dl, sess, task_id_ptr, url_dl);
                    }
                }
            }
        }

        task_arc.lock().unwrap().state = 3; // Completed
        let s_comp = sel("URLSession:task:didCompleteWithError:");
        if !del_cls.is_null() && runtime::lookup(del_cls, s_comp).is_some() {
            unsafe {
                let d = lookup_for_send(del, s_comp);
                let f: extern "C" fn(Id, Sel, Id, Id, Id) = std::mem::transmute(d.imp);
                f(d.receiver, s_comp, sess, task_id_ptr, err_id);
            }
        }
    } else {
        task_arc.lock().unwrap().state = 3;
    }

    release(task_ptr as Id);
}

extern "C" fn task_resume(this: Id, _cmd: Sel) {
    let task_arc = {
        let g = url_tasks().lock().unwrap();
        g.get(&(this as usize)).cloned()
    };
    let Some(st_arc) = task_arc else { return };
    let should_spawn = {
        let mut g = st_arc.lock().unwrap();
        if g.started || g.cancelled {
            false
        } else {
            g.started = true;
            g.state = 0; // NSURLSessionTaskStateRunning
            true
        }
    };
    if should_spawn {
        let task_ptr = retain(this) as usize;
        std::thread::spawn(move || task_worker(task_ptr, st_arc));
    }
}

extern "C" fn task_cancel(this: Id, _cmd: Sel) {
    if let Some(st_arc) = url_tasks().lock().unwrap().get(&(this as usize)).cloned() {
        let mut g = st_arc.lock().unwrap();
        g.cancelled = true;
        if g.state != 3 {
            g.state = 2; // NSURLSessionTaskStateCanceling
        }
    }
}

extern "C" fn task_suspend(this: Id, _cmd: Sel) {
    if let Some(st_arc) = url_tasks().lock().unwrap().get(&(this as usize)).cloned() {
        let mut g = st_arc.lock().unwrap();
        if !g.started && g.state != 3 {
            g.state = 1;
        }
    }
}

extern "C" fn task_state(this: Id, _cmd: Sel) -> usize {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().state)
        .unwrap_or(3)
}

extern "C" fn task_identifier(this: Id, _cmd: Sel) -> usize {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().task_id)
        .unwrap_or(0)
}

extern "C" fn task_request(this: Id, _cmd: Sel) -> Id {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().request as Id)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn task_response(this: Id, _cmd: Sel) -> Id {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().response as Id)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn task_error(this: Id, _cmd: Sel) -> Id {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().error as Id)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn task_bytes_received(this: Id, _cmd: Sel) -> i64 {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().bytes_received)
        .unwrap_or(0)
}

extern "C" fn task_bytes_expected_recv(this: Id, _cmd: Sel) -> i64 {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().bytes_expected_recv)
        .unwrap_or(-1)
}

extern "C" fn task_bytes_sent(this: Id, _cmd: Sel) -> i64 {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().bytes_sent)
        .unwrap_or(0)
}

extern "C" fn task_bytes_expected_send(this: Id, _cmd: Sel) -> i64 {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().bytes_expected_send)
        .unwrap_or(0)
}

extern "C" fn task_priority(this: Id, _cmd: Sel) -> f32 {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().priority)
        .unwrap_or(0.5)
}

extern "C" fn task_set_priority(this: Id, _cmd: Sel, p: f32) {
    if let Some(s) = url_tasks().lock().unwrap().get(&(this as usize)).cloned() {
        s.lock().unwrap().priority = p;
    }
}

extern "C" fn task_description(this: Id, _cmd: Sel) -> Id {
    url_tasks()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|s| s.lock().unwrap().description as Id)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn task_set_description(this: Id, _cmd: Sel, d: Id) {
    let new_d = if d.is_null() { 0 } else { retain(d) as usize };
    if let Some(s) = url_tasks().lock().unwrap().get(&(this as usize)).cloned() {
        let old = {
            let mut g = s.lock().unwrap();
            let o = g.description;
            g.description = new_d;
            o
        };
        if old != 0 {
            release(old as Id);
        }
    } else if new_d != 0 {
        release(new_d as Id);
    }
}

extern "C" fn task_dealloc(this: Id, cmd: Sel) {
    if let Some(st_arc) = url_tasks().lock().unwrap().remove(&(this as usize)) {
        let mut g = st_arc.lock().unwrap();
        if g.completion_block != 0 {
            let blk = g.completion_block as *const c_void;
            g.completion_block = 0;
            unsafe { pear_libobjc::blocks::block_release(blk) };
        }
        let ids = [
            g.session,
            g.delegate,
            g.request,
            g.url_obj,
            g.description,
            g.response,
            g.error,
        ];
        g.session = 0;
        g.delegate = 0;
        g.request = 0;
        g.url_obj = 0;
        g.description = 0;
        g.response = 0;
        g.error = 0;
        drop(g);
        for id in ids {
            if id != 0 {
                release(id as Id);
            }
        }
    }
    store().lock().unwrap().remove(&(this as usize));
    unsafe { dispose(this) };
    let _ = cmd;
}

pub(crate) fn register_classes() {
    runtime::register_builtin(
        NSURLRESPONSE_CLASS.ptr(),
        NSURLRESPONSE_META.ptr(),
        "NSURLResponse",
        16,
        &[
            ("URL", i(resp_url as *const ())),
            ("MIMEType", i(resp_mime_type as *const ())),
            ("expectedContentLength", i(resp_expected_len as *const ())),
            ("textEncodingName", i(resp_text_encoding as *const ())),
            ("suggestedFilename", i(resp_suggested_filename as *const ())),
            (
                "initWithURL:MIMEType:expectedContentLength:textEncodingName:",
                i(resp_init as *const ()),
            ),
            ("dealloc", i(resp_dealloc as *const ())),
        ],
        &[],
    );
    runtime::register_builtin(
        NSHTTPURLRESPONSE_CLASS.ptr(),
        NSHTTPURLRESPONSE_META.ptr(),
        "NSHTTPURLResponse",
        16,
        &[
            (
                "initWithURL:statusCode:HTTPVersion:headerFields:",
                i(http_resp_init as *const ()),
            ),
            ("statusCode", i(http_resp_status_code as *const ())),
            ("allHeaderFields", i(http_resp_all_headers as *const ())),
            ("valueForHTTPHeaderField:", i(http_resp_header_for_field as *const ())),
            ("dealloc", i(resp_dealloc as *const ())),
        ],
        &[(
            "localizedStringForStatusCode:",
            i(http_resp_localized_status as *const ()),
        )],
    );
    runtime::register_builtin(
        NSURLSESSIONTASK_CLASS.ptr(),
        NSURLSESSIONTASK_META.ptr(),
        "NSURLSessionTask",
        16,
        &[
            ("resume", i(task_resume as *const ())),
            ("cancel", i(task_cancel as *const ())),
            ("suspend", i(task_suspend as *const ())),
            ("state", i(task_state as *const ())),
            ("taskIdentifier", i(task_identifier as *const ())),
            ("originalRequest", i(task_request as *const ())),
            ("currentRequest", i(task_request as *const ())),
            ("response", i(task_response as *const ())),
            ("error", i(task_error as *const ())),
            ("countOfBytesReceived", i(task_bytes_received as *const ())),
            (
                "countOfBytesExpectedToReceive",
                i(task_bytes_expected_recv as *const ()),
            ),
            ("countOfBytesSent", i(task_bytes_sent as *const ())),
            (
                "countOfBytesExpectedToSend",
                i(task_bytes_expected_send as *const ()),
            ),
            ("priority", i(task_priority as *const ())),
            ("setPriority:", i(task_set_priority as *const ())),
            ("taskDescription", i(task_description as *const ())),
            ("setTaskDescription:", i(task_set_description as *const ())),
            ("dealloc", i(task_dealloc as *const ())),
        ],
        &[],
    );
    runtime::register_builtin(
        NSURLSESSIONDATATASK_CLASS.ptr(),
        NSURLSESSIONDATATASK_META.ptr(),
        "NSURLSessionDataTask",
        16,
        &[],
        &[],
    );
    runtime::register_builtin(
        NSURLSESSIONUPLOADTASK_CLASS.ptr(),
        NSURLSESSIONUPLOADTASK_META.ptr(),
        "NSURLSessionUploadTask",
        16,
        &[],
        &[],
    );
    runtime::register_builtin(
        NSURLSESSIONDOWNLOADTASK_CLASS.ptr(),
        NSURLSESSIONDOWNLOADTASK_META.ptr(),
        "NSURLSessionDownloadTask",
        16,
        &[],
        &[],
    );
}

// ---- NSURLConnection Backend Helpers ---------------------------------------

struct ExtractedRequest {
    req_obj: usize,
    url_obj: usize,
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    timeout_secs: u64,
}

fn extract_request_parts(req: Id) -> ExtractedRequest {
    let req_obj = if is_request_instance(req) {
        retain(req) as usize
    } else if !req.is_null() {
        req_with_url(NSURLREQUEST_CLASS.ptr() as Id, sel("requestWithURL:"), req) as usize
    } else {
        0
    };
    let r = req_obj as Id;
    let url_obj = if !r.is_null() {
        let u = req_url(r, sel("URL"));
        if !u.is_null() {
            retain(u) as usize
        } else {
            0
        }
    } else {
        0
    };
    let url = extract_url_string(url_obj as Id);
    let method = if !r.is_null() {
        rust_str_from_nsstring(req_method(r, sel("HTTPMethod")))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "GET".to_string())
    } else {
        "GET".to_string()
    };
    let headers = if !r.is_null() {
        crate::extract_request_headers(r)
    } else {
        Vec::new()
    };
    let body = if !r.is_null() {
        rust_bytes_from_nsdata(req_body(r, sel("HTTPBody"))).unwrap_or_default()
    } else {
        Vec::new()
    };
    let timeout_secs = if !r.is_null() {
        let t = req_timeout(r, sel("timeoutInterval"));
        if t.is_finite() && t > 0.0 {
            (t.round() as u64).clamp(3, 25)
        } else {
            15
        }
    } else {
        15
    };
    ExtractedRequest {
        req_obj,
        url_obj,
        url,
        method,
        headers,
        body,
        timeout_secs,
    }
}

struct UrlConnState {
    conn_id: usize,
    req_obj: usize,
    url_obj: usize,
    delegate: usize,
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    timeout_secs: u64,
    started: bool,
    cancelled: bool,
}

static URL_CONNECTIONS: OnceLock<Mutex<HashMap<usize, Arc<Mutex<UrlConnState>>>>> = OnceLock::new();

fn url_connections() -> &'static Mutex<HashMap<usize, Arc<Mutex<UrlConnState>>>> {
    URL_CONNECTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Initializes backing state for an `NSURLConnection` instance and optionally starts immediately.
pub fn url_conn_init(conn: Id, req: Id, del: Id, start_immediately: bool) {
    if conn.is_null() {
        return;
    }
    let parts = extract_request_parts(req);
    let del_id = if del.is_null() { 0 } else { retain(del) as usize };
    if net_trace_enabled() && !parts.url.is_empty() {
        eprintln!(
            "[PearRuntime][net][NSURLConnection] init method={} url={} start={} has_del={}",
            parts.method,
            parts.url,
            start_immediately,
            del_id != 0
        );
    }
    let conn_id = TASK_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let st = Arc::new(Mutex::new(UrlConnState {
        conn_id,
        req_obj: parts.req_obj,
        url_obj: parts.url_obj,
        delegate: del_id,
        url: parts.url,
        method: parts.method,
        headers: parts.headers,
        body: parts.body,
        timeout_secs: parts.timeout_secs,
        started: false,
        cancelled: false,
    }));
    let prev = url_connections().lock().unwrap().insert(conn as usize, st);
    if let Some(old_arc) = prev {
        let mut old = old_arc.lock().unwrap();
        let ids = [old.req_obj, old.url_obj, old.delegate];
        old.req_obj = 0;
        old.url_obj = 0;
        old.delegate = 0;
        drop(old);
        for id in ids {
            if id != 0 {
                release(id as Id);
            }
        }
    }
    if start_immediately && !req.is_null() {
        url_conn_start(conn);
    }
}

fn url_conn_worker(conn_ptr: usize, st_arc: Arc<Mutex<UrlConnState>>) {
    let (conn_id, url, method, headers, body, timeout_secs, url_obj, delegate) = {
        let g = st_arc.lock().unwrap();
        (
            g.conn_id,
            g.url.clone(),
            g.method.clone(),
            g.headers.clone(),
            g.body.clone(),
            g.timeout_secs,
            g.url_obj,
            g.delegate,
        )
    };
    let result = if st_arc.lock().unwrap().cancelled {
        Err((-999isize, "cancelled".to_string()))
    } else {
        execute_http_via_curl(conn_id, &url, &method, &headers, &body, timeout_secs)
    };
    let cancelled = st_arc.lock().unwrap().cancelled;
    let final_res = if cancelled {
        Err((-999isize, "cancelled".to_string()))
    } else {
        result
    };
    if delegate != 0 {
        let del = delegate as Id;
        let del_cls = unsafe { object_class(del) };
        let conn = conn_ptr as Id;
        match final_res {
            Ok((status, hdrs, bytes)) => {
                if net_trace_enabled() {
                    eprintln!(
                        "[PearRuntime][net][NSURLConnection] done status={} bytes={} url={}",
                        status,
                        bytes.len(),
                        url
                    );
                }
                let u_for_resp = if url_obj != 0 {
                    url_obj as Id
                } else {
                    nsstring_from_rust(&url)
                };
                let resp = make_http_response(u_for_resp, status, hdrs, bytes.len());
                let s_resp = sel("connection:didReceiveResponse:");
                if !del_cls.is_null() && runtime::lookup(del_cls, s_resp).is_some() {
                    unsafe {
                        let d = lookup_for_send(del, s_resp);
                        let f: extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(d.imp);
                        f(d.receiver, s_resp, conn, resp);
                    }
                }
                if !bytes.is_empty() {
                    let s_data = sel("connection:didReceiveData:");
                    if !del_cls.is_null() && runtime::lookup(del_cls, s_data).is_some() {
                        let data_obj = nsdata_from_bytes(&bytes);
                        unsafe {
                            let d = lookup_for_send(del, s_data);
                            let f: extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(d.imp);
                            f(d.receiver, s_data, conn, data_obj);
                        }
                    }
                }
                let s_fin = sel("connectionDidFinishLoading:");
                if !del_cls.is_null() && runtime::lookup(del_cls, s_fin).is_some() {
                    unsafe {
                        let d = lookup_for_send(del, s_fin);
                        let f: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d.imp);
                        f(d.receiver, s_fin, conn);
                    }
                }
            }
            Err((code, msg)) => {
                if net_trace_enabled() {
                    eprintln!(
                        "[PearRuntime][net][NSURLConnection] err code={} msg={:?} url={}",
                        code, msg, url
                    );
                }
                let err = make_nserror("NSURLErrorDomain", code, &msg);
                let s_fail = sel("connection:didFailWithError:");
                if !del_cls.is_null() && runtime::lookup(del_cls, s_fail).is_some() {
                    unsafe {
                        let d = lookup_for_send(del, s_fail);
                        let f: extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(d.imp);
                        f(d.receiver, s_fail, conn, err);
                    }
                }
            }
        }
    }
    release(conn_ptr as Id);
}

/// Starts a previously initialized `NSURLConnection` on a background worker thread.
pub fn url_conn_start(conn: Id) {
    let st_arc = url_connections().lock().unwrap().get(&(conn as usize)).cloned();
    let Some(arc) = st_arc else { return };
    let should_spawn = {
        let mut g = arc.lock().unwrap();
        if g.started || g.cancelled || g.req_obj == 0 || g.url.is_empty() {
            false
        } else {
            g.started = true;
            true
        }
    };
    if should_spawn {
        let conn_ptr = retain(conn) as usize;
        std::thread::spawn(move || url_conn_worker(conn_ptr, arc));
    }
}

/// Marks an `NSURLConnection` as cancelled.
pub fn url_conn_cancel(conn: Id) {
    if let Some(arc) = url_connections().lock().unwrap().get(&(conn as usize)).cloned() {
        arc.lock().unwrap().cancelled = true;
    }
}

/// Releases retained request/delegate references for an `NSURLConnection` during dealloc.
pub fn url_conn_dealloc(conn: Id) {
    if let Some(arc) = url_connections().lock().unwrap().remove(&(conn as usize)) {
        let mut g = arc.lock().unwrap();
        let ids = [g.req_obj, g.url_obj, g.delegate];
        g.req_obj = 0;
        g.url_obj = 0;
        g.delegate = 0;
        drop(g);
        for id in ids {
            if id != 0 {
                release(id as Id);
            }
        }
    }
}

/// Executes an `NSURLConnection` synchronous HTTP/HTTPS request.
pub fn url_conn_send_sync(req: Id, resp_out: *mut Id, err_out: *mut Id) -> Id {
    let parts = extract_request_parts(req);
    let task_id = TASK_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    if net_trace_enabled() {
        eprintln!(
            "[PearRuntime][net][NSURLConnection] sync method={} url={}",
            parts.method, parts.url
        );
    }
    let res = execute_http_via_curl(
        task_id,
        &parts.url,
        &parts.method,
        &parts.headers,
        &parts.body,
        parts.timeout_secs,
    );
    let ret = match res {
        Ok((status, hdrs, bytes)) => {
            if net_trace_enabled() {
                eprintln!(
                    "[PearRuntime][net][NSURLConnection] sync done status={} bytes={} url={}",
                    status,
                    bytes.len(),
                    parts.url
                );
            }
            let u_for_resp = if parts.url_obj != 0 {
                parts.url_obj as Id
            } else {
                nsstring_from_rust(&parts.url)
            };
            let resp = make_http_response(u_for_resp, status, hdrs, bytes.len());
            if !resp_out.is_null() {
                unsafe { *resp_out = resp };
            }
            if !err_out.is_null() {
                unsafe { *err_out = std::ptr::null_mut() };
            }
            nsdata_from_bytes(&bytes)
        }
        Err((code, msg)) => {
            if net_trace_enabled() {
                eprintln!(
                    "[PearRuntime][net][NSURLConnection] sync err code={} msg={:?} url={}",
                    code, msg, parts.url
                );
            }
            if !resp_out.is_null() {
                unsafe { *resp_out = std::ptr::null_mut() };
            }
            if !err_out.is_null() {
                unsafe { *err_out = make_nserror("NSURLErrorDomain", code, &msg) };
            }
            std::ptr::null_mut()
        }
    };
    if parts.req_obj != 0 {
        release(parts.req_obj as Id);
    }
    if parts.url_obj != 0 {
        release(parts.url_obj as Id);
    }
    ret
}

/// Executes `NSURLConnection +sendAsynchronousRequest:queue:completionHandler:`.
/// Block receives `(NSURLResponse *response, NSData *data, NSError *error)`.
pub fn url_conn_send_async(req: Id, _queue: Id, cb: *const c_void) {
    let parts = extract_request_parts(req);
    let task_id = TASK_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let cb_ptr = if cb.is_null() {
        0
    } else {
        unsafe { pear_libobjc::blocks::block_copy(cb) as usize }
    };
    if net_trace_enabled() {
        eprintln!(
            "[PearRuntime][net][NSURLConnection] async method={} url={} has_cb={}",
            parts.method,
            parts.url,
            cb_ptr != 0
        );
    }
    std::thread::spawn(move || {
        let res = execute_http_via_curl(
            task_id,
            &parts.url,
            &parts.method,
            &parts.headers,
            &parts.body,
            parts.timeout_secs,
        );
        let (resp, data, err) = match res {
            Ok((status, hdrs, bytes)) => {
                let u_for_resp = if parts.url_obj != 0 {
                    parts.url_obj as Id
                } else {
                    nsstring_from_rust(&parts.url)
                };
                let r = make_http_response(u_for_resp, status, hdrs, bytes.len());
                (r, nsdata_from_bytes(&bytes), std::ptr::null_mut())
            }
            Err((code, msg)) => (
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                make_nserror("NSURLErrorDomain", code, &msg),
            ),
        };
        if cb_ptr != 0 {
            unsafe {
                let blk = cb_ptr as *const c_void;
                let layout = &*(blk as *const pear_libobjc::blocks::BlockLayout);
                if !layout.invoke.is_null() {
                    let f: extern "C" fn(*const c_void, Id, Id, Id) = std::mem::transmute(layout.invoke);
                    f(blk, resp, data, err);
                }
                pear_libobjc::blocks::block_release(blk);
            }
        }
        if parts.req_obj != 0 {
            release(parts.req_obj as Id);
        }
        if parts.url_obj != 0 {
            release(parts.url_obj as Id);
        }
    });
}
