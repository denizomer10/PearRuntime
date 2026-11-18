//! Cold-state client for pear-host: a big-endian length followed by a binary
//! plist dictionary over a Unix socket. Configuration is fixed on first use;
//! the presence of PEAR_HOST_SOCKET selects host-authoritative mode even when
//! the path/token is invalid or the host is down (never a fallback to the
//! local store). Not for input, render or audio paths.
//!
//! Every call carries version=1 and the launch token and verifies the peer
//! UID. Main-thread calls have a 100 ms budget, others 500 ms, including lock
//! waits and all I/O; failures back off for 250 ms. No request is replayed
//! after any byte was sent. Wire limits match HostServiceProtocol.swift.

use pear_objc::{cls, msg0, msg1, msg2, nsarray_count, nsarray_object_at, nsstring, objc_retain, Id, NIL};
use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::OnceLock;

const MAX_FRAME: usize = 1 << 20;
const MAX_KEY: usize = 1024;
const MAIN_BUDGET: u64 = 100_000_000;
const WORKER_BUDGET: u64 = 500_000_000;
const FAILURE_BACKOFF: u64 = 250_000_000;

const AF_UNIX: c_int = 1;
const SOCK_STREAM: c_int = 1;
const SOL_SOCKET: c_int = 0xffff;
const SO_NOSIGPIPE: c_int = 0x1022;
const SO_ERROR: c_int = 0x1007;
const F_GETFD: c_int = 1;
const F_SETFD: c_int = 2;
const F_GETFL: c_int = 3;
const F_SETFL: c_int = 4;
const O_NONBLOCK: c_int = 4;
const FD_CLOEXEC: c_int = 1;
const EINTR: c_int = 4;
const EAGAIN: c_int = 35;
const EINPROGRESS: c_int = 36;
const MSG_PEEK: c_int = 2;
const POLLIN: i16 = 0x1;
const POLLOUT: i16 = 0x4;
const POLLERR: i16 = 0x8;
const POLLHUP: i16 = 0x10;
const POLLNVAL: i16 = 0x20;
const CLOCK_MONOTONIC: c_int = 6;
const BINARY_PLIST: usize = 200; // NSPropertyListBinaryFormat_v1_0

#[repr(C)]
struct SockaddrUn {
    sun_len: u8,
    sun_family: u8,
    sun_path: [u8; 104],
}

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}

extern "C" {
    fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn setsockopt(fd: c_int, level: c_int, name: c_int, value: *const c_void, len: u32) -> c_int;
    fn getsockopt(fd: c_int, level: c_int, name: c_int, value: *mut c_void, len: *mut u32) -> c_int;
    fn connect(fd: c_int, addr: *const SockaddrUn, len: u32) -> c_int;
    fn poll(fds: *mut PollFd, n: u32, timeout: c_int) -> c_int;
    fn send(fd: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize;
    fn recv(fd: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize;
    fn close(fd: c_int) -> c_int;
    fn getpeereid(fd: c_int, uid: *mut u32, gid: *mut u32) -> c_int;
    fn getuid() -> u32;
    fn clock_gettime(clock: c_int, ts: *mut Timespec) -> c_int;
    fn __error() -> *mut c_int;
    fn dispatch_semaphore_create(value: isize) -> Id;
    fn dispatch_semaphore_wait(sema: Id, timeout: u64) -> isize;
    fn dispatch_semaphore_signal(sema: Id) -> isize;
    fn dispatch_time(when: u64, delta: i64) -> u64;
    fn CFGetTypeID(cf: Id) -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFNumberIsFloatType(n: Id) -> bool;
}

fn errno() -> c_int {
    unsafe { *__error() }
}

fn now() -> u64 {
    let mut ts = Timespec { sec: 0, nsec: 0 };
    unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) };
    ts.sec as u64 * 1_000_000_000 + ts.nsec as u64
}

fn is_main_thread() -> bool {
    unsafe { msg0::<bool>(cls("NSThread"), "isMainThread") }
}

/// Start of one cold-state operation budget (CLOCK_MONOTONIC ns).
#[export_name = "_Z16PearHostDeadlinev"]
pub extern "C" fn deadline() -> u64 {
    now() + if is_main_thread() { MAIN_BUDGET } else { WORKER_BUDGET }
}

/// On true the caller owns `gate` and must signal it; on false it owns nothing,
/// including when the deadline expires just as the gate is acquired.
#[export_name = "_Z20PearHostAcquireUntilPU32objcproto21OS_dispatch_semaphore8NSObjecty"]
pub unsafe extern "C" fn acquire_until(gate: Id, deadline: u64) -> bool {
    let t = now();
    if t >= deadline || dispatch_semaphore_wait(gate, dispatch_time(0, (deadline - t) as i64)) != 0 {
        return false;
    }
    if now() >= deadline {
        dispatch_semaphore_signal(gate);
        return false;
    }
    true
}

struct Config {
    configured: bool,
    socket: Option<Vec<u8>>,
    token: Id, // retained NSString or nil
    lock: usize,
}
unsafe impl Send for Config {}
unsafe impl Sync for Config {}

fn config() -> &'static Config {
    static C: OnceLock<Config> = OnceLock::new();
    C.get_or_init(|| unsafe {
        let lock = objc_retain(dispatch_semaphore_create(1)) as usize;
        let mut c = Config { configured: false, socket: None, token: NIL, lock };
        // Presence, not validity, selects the exclusive host-writer mode.
        let Some(path) = std::env::var_os("PEAR_HOST_SOCKET") else { return c };
        c.configured = true;
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_bytes();
        if bytes.is_empty() || bytes.len() >= 104 || bytes[0] != b'/' || bytes.contains(&0) {
            return c;
        }
        let Ok(text) = std::str::from_utf8(bytes) else { return c };
        let last = text.rsplit('/').next().unwrap_or("");
        if text.ends_with('/') || last == "." || last == ".." {
            return c;
        }
        c.socket = Some(bytes.to_vec());
        let Some(token) = std::env::var_os("PEAR_HOST_TOKEN") else { return c };
        let t = token.as_bytes();
        if t.is_empty() || t.len() > MAX_FRAME {
            return c;
        }
        if let Ok(s) = std::str::from_utf8(t) {
            c.token = objc_retain(nsstring(s));
        }
        c
    })
}

#[export_name = "_Z20PearHostIsConfiguredv"]
pub extern "C" fn is_configured() -> bool {
    config().configured
}

// Connection state, protected by the config lock.
static FD: AtomicI32 = AtomicI32::new(-1);
static RETRY_AFTER: AtomicU64 = AtomicU64::new(0);

fn reset_locked() {
    let fd = FD.swap(-1, Ordering::AcqRel);
    if fd >= 0 {
        unsafe { close(fd) };
    }
}

fn wait(fd: c_int, events: i16, deadline: u64) -> bool {
    loop {
        let t = now();
        if t >= deadline {
            return false;
        }
        let ms = ((deadline - t + 999_999) / 1_000_000).min(c_int::MAX as u64) as c_int;
        let mut p = PollFd { fd, events, revents: 0 };
        let r = unsafe { poll(&mut p, 1, ms) };
        if r > 0 {
            if p.revents & POLLNVAL != 0 {
                return false;
            }
            if p.revents & (events | POLLHUP | POLLERR) != 0 {
                return true;
            }
        } else if r < 0 && errno() != EINTR {
            return false;
        }
    }
}

unsafe fn connect_locked(path: &[u8], deadline: u64) -> bool {
    if now() >= deadline {
        return false;
    }
    let fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if fd < 0 {
        return false;
    }
    let enabled: c_int = 1;
    let flags = fcntl(fd, F_GETFL);
    let fd_flags = fcntl(fd, F_GETFD);
    if setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &enabled as *const c_int as *const c_void, 4) != 0
        || flags < 0
        || fcntl(fd, F_SETFL, flags | O_NONBLOCK) != 0
        || fd_flags < 0
        || fcntl(fd, F_SETFD, fd_flags | FD_CLOEXEC) != 0
    {
        close(fd);
        return false;
    }
    let mut addr = SockaddrUn { sun_len: std::mem::size_of::<SockaddrUn>() as u8, sun_family: AF_UNIX as u8, sun_path: [0; 104] };
    addr.sun_path[..path.len()].copy_from_slice(path);
    if connect(fd, &addr, std::mem::size_of::<SockaddrUn>() as u32) != 0 {
        let code = errno();
        if !matches!(code, EINPROGRESS | EINTR | EAGAIN) || !wait(fd, POLLOUT, deadline) {
            close(fd);
            return false;
        }
        let mut err: c_int = 0;
        let mut size = 4u32;
        if getsockopt(fd, SOL_SOCKET, SO_ERROR, &mut err as *mut c_int as *mut c_void, &mut size) != 0 || err != 0 {
            close(fd);
            return false;
        }
    }
    let (mut uid, mut gid) = (0u32, 0u32);
    if getpeereid(fd, &mut uid, &mut gid) != 0 || uid != getuid() || now() >= deadline {
        close(fd);
        return false;
    }
    FD.store(fd, Ordering::Release);
    true
}

/// The server expires idle connections: detect EOF (or unsolicited bytes)
/// before sending, while reconnecting is still unambiguously safe.
fn connection_reusable_locked(deadline: u64) -> bool {
    let fd = FD.load(Ordering::Acquire);
    let mut byte = 0u8;
    while now() < deadline {
        let n = unsafe { recv(fd, &mut byte as *mut u8 as *mut c_void, 1, MSG_PEEK) };
        if n < 0 && errno() == EINTR {
            continue;
        }
        return n < 0 && errno() == EAGAIN;
    }
    false
}

fn write_all(fd: c_int, frame: &[u8], sent: &mut usize, deadline: u64) -> bool {
    while *sent < frame.len() {
        if !wait(fd, POLLOUT, deadline) {
            return false;
        }
        let n = unsafe { send(fd, frame[*sent..].as_ptr() as *const c_void, frame.len() - *sent, 0) };
        if n > 0 {
            *sent += n as usize;
        } else if n == 0 || !matches!(errno(), EINTR | EAGAIN) {
            return false;
        }
    }
    true
}

fn read_exact(fd: c_int, buf: &mut [u8], deadline: u64) -> bool {
    let mut off = 0;
    while off < buf.len() {
        if !wait(fd, POLLIN, deadline) {
            return false;
        }
        let n = unsafe { recv(fd, buf[off..].as_mut_ptr() as *mut c_void, buf.len() - off, 0) };
        if n > 0 {
            off += n as usize;
        } else if n == 0 || !matches!(errno(), EINTR | EAGAIN) {
            return false;
        }
    }
    true
}

unsafe fn kind(v: Id, name: &str) -> bool {
    pear_objc::is_kind_of(v, name)
}

/// Rejects nulls, non-string keys and excessive nesting before
/// serialization and before a decoded value reaches a caller.
unsafe fn valid_plist(v: Id, depth: usize) -> bool {
    if v.is_null() || depth > 128 {
        return false;
    }
    if kind(v, "NSString") || kind(v, "NSNumber") || kind(v, "NSDate") {
        return true;
    }
    if kind(v, "NSData") {
        return msg0::<usize>(v, "length") <= MAX_FRAME;
    }
    if kind(v, "NSArray") {
        let n = nsarray_count(v);
        return n <= MAX_FRAME && (0..n).all(|i| valid_plist(nsarray_object_at(v, i), depth + 1));
    }
    if kind(v, "NSDictionary") {
        let keys: Id = msg0(v, "allKeys");
        let n = nsarray_count(keys);
        return n <= MAX_FRAME
            && (0..n).all(|i| {
                let k = nsarray_object_at(keys, i);
                kind(k, "NSString") && valid_plist(msg1(v, "objectForKey:", k), depth + 1)
            });
    }
    false
}

unsafe fn is_boolean(v: Id) -> bool {
    !v.is_null() && kind(v, "NSNumber") && CFGetTypeID(v) == CFBooleanGetTypeID()
}

unsafe fn get(d: Id, key: &str) -> Id {
    msg1(d, "objectForKey:", nsstring(key))
}

unsafe fn valid_response(r: Id, operation: Id) -> bool {
    let ok = get(r, "ok");
    if !is_boolean(ok) {
        return false;
    }
    if !msg0::<bool>(ok, "boolValue") {
        return kind(get(r, "error"), "NSString");
    }
    if operation.is_null() || !kind(operation, "NSString") {
        return true;
    }
    match crate::rt::ns_string(operation).as_str() {
        "ping" => {
            let version = get(r, "version");
            msg1::<Id, bool>(nsstring("pear-host"), "isEqual:", get(r, "service"))
                && kind(get(r, "bundle"), "NSString")
                && kind(version, "NSNumber")
                && !is_boolean(version)
                && !CFNumberIsFloatType(version)
                && msg0::<i64>(version, "longLongValue") == 1
        }
        "kv.get" => {
            let found = get(r, "found");
            is_boolean(found) && msg0::<bool>(found, "boolValue") == !get(r, "value").is_null()
        }
        "kv.all" => kind(get(r, "value"), "NSDictionary"),
        _ => true,
    }
}

unsafe fn fail() -> Id {
    reset_locked();
    RETRY_AFTER.store(now() + FAILURE_BACKOFF, Ordering::Release);
    NIL
}

unsafe fn call_locked(c: &Config, path: &[u8], request: Id, deadline: u64) -> Id {
    if now() < RETRY_AFTER.load(Ordering::Acquire) || now() >= deadline {
        return NIL;
    }
    let authenticated: Id = crate::rt::autoreleased(msg0(request, "mutableCopy"));
    msg2::<Id, Id, ()>(authenticated, "setObject:forKey:", pear_objc::nsnumber_i64(1), nsstring("version"));
    msg2::<Id, Id, ()>(authenticated, "setObject:forKey:", c.token, nsstring("token"));
    let serializer = cls("NSPropertyListSerialization");
    if !valid_plist(authenticated, 0) || !msg2::<Id, usize, bool>(serializer, "propertyList:isValidForFormat:", authenticated, BINARY_PLIST) {
        return NIL;
    }
    let payload: Id = pear_objc::msg4(serializer, "dataWithPropertyList:format:options:error:", authenticated, BINARY_PLIST, 0usize, std::ptr::null_mut::<Id>());
    let Some(payload) = pear_objc::nsdata_to_vec(payload) else { return NIL };
    if payload.is_empty() || payload.len() > MAX_FRAME || now() >= deadline {
        return NIL;
    }
    let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&payload);

    let mut reused = FD.load(Ordering::Acquire) >= 0;
    if reused && !connection_reusable_locked(deadline) {
        reset_locked();
        reused = false;
    }
    if FD.load(Ordering::Acquire) < 0 && !connect_locked(path, deadline) {
        return fail();
    }
    let mut sent = 0usize;
    if !write_all(FD.load(Ordering::Acquire), &frame, &mut sent, deadline) {
        reset_locked();
        // Even a partial header may have reached the server: never replay a
        // mutation after a lost ACK. Only a zero-byte send on an old
        // connection is retried, once, within the same deadline.
        if !reused || sent != 0 || !connect_locked(path, deadline) || !write_all(FD.load(Ordering::Acquire), &frame, &mut sent, deadline) {
            return fail();
        }
    }
    let fd = FD.load(Ordering::Acquire);
    let mut header = [0u8; 4];
    if !read_exact(fd, &mut header, deadline) {
        return fail();
    }
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > MAX_FRAME {
        return fail();
    }
    let mut body = vec![0u8; size];
    if !read_exact(fd, &mut body, deadline) {
        return fail();
    }
    let data = pear_objc::nsdata_from_bytes(&body);
    let mut format: usize = BINARY_PLIST;
    let object: Id = pear_objc::msg4(serializer, "propertyListWithData:options:format:error:", data, 0usize, &mut format as *mut usize, std::ptr::null_mut::<Id>());
    if format != BINARY_PLIST
        || object.is_null()
        || !kind(object, "NSDictionary")
        || !valid_plist(object, 0)
        || !valid_response(object, get(request, "op"))
        || now() >= deadline
    {
        return fail();
    }
    RETRY_AFTER.store(0, Ordering::Release);
    // Auth/version errors can close the peer: never keep an errored connection.
    if !msg0::<bool>(get(object, "ok"), "boolValue") {
        reset_locked();
    }
    object
}

#[export_name = "_Z17PearHostCallUntilP12NSDictionaryy"]
pub unsafe extern "C" fn call_until(request: Id, deadline_in: u64) -> Id {
    let deadline = deadline_in.min(deadline());
    let c = config();
    let Some(path) = c.socket.as_deref() else { return NIL };
    if !c.configured || c.token.is_null() || request.is_null() || !kind(request, "NSDictionary") {
        return NIL;
    }
    let lock = c.lock as Id;
    if !acquire_until(lock, deadline) {
        return NIL;
    }
    let result = call_locked(c, path, request, deadline);
    dispatch_semaphore_signal(lock);
    result
}

#[export_name = "_Z12PearHostCallP12NSDictionary"]
pub unsafe extern "C" fn call(request: Id) -> Id {
    call_until(request, deadline())
}

unsafe fn valid_key(key: Id) -> bool {
    if key.is_null() || !kind(key, "NSString") {
        return false;
    }
    let len: usize = msg1(key, "lengthOfBytesUsingEncoding:", 4usize);
    len > 0 && len <= MAX_KEY
}

unsafe fn request(pairs: &[(&str, Id)]) -> Id {
    let d: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    for (k, v) in pairs {
        if !v.is_null() {
            msg2::<Id, Id, ()>(d, "setObject:forKey:", *v, nsstring(k));
        }
    }
    d
}

unsafe fn ok(response: Id) -> bool {
    !response.is_null() && {
        let o = get(response, "ok");
        !o.is_null() && msg0::<bool>(o, "boolValue")
    }
}

/// true with *value=nil means confirmed missing; false means unavailable.
#[export_name = "_Z13PearHostKVGetP8NSStringPU15__autoreleasingP11objc_object"]
pub unsafe extern "C" fn kv_get(key: Id, value: *mut Id) -> bool {
    if !value.is_null() {
        *value = NIL;
    }
    if !valid_key(key) {
        return false;
    }
    let response = call(request(&[("op", nsstring("kv.get")), ("key", key)]));
    if !ok(response) {
        return false;
    }
    if !value.is_null() {
        *value = get(response, "value");
    }
    true
}

/// nil deletes by omitting the value (NSNull is no plist value). true needs a
/// host ACK; false may be an unknown commit outcome, never a replay.
#[export_name = "_Z13PearHostKVSetP8NSStringP11objc_object"]
pub unsafe extern "C" fn kv_set(key: Id, value: Id) -> bool {
    valid_key(key) && ok(call(request(&[("op", nsstring("kv.set")), ("key", key), ("value", value)])))
}

#[export_name = "_Z13PearHostKVAllv"]
pub unsafe extern "C" fn kv_all() -> Id {
    let response = call(request(&[("op", nsstring("kv.all"))]));
    if ok(response) { get(response, "value") } else { NIL }
}
