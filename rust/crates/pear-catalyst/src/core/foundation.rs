//! Keeps two common Foundation contracts from terminating an app.
//!
//! 1. Serializers and image decoders raise on nil data (the nil result of a
//!    failed download); the documented behaviour for an unreadable payload is
//!    a nil result plus an error, which is what callers get here.
//! 2. Uncaught exceptions and crash signals are logged with their stacks
//!    before the process exits.
//!
//! Also `-[NSBundle classNamed:]` for system frameworks, whose iOS-layout
//! bundle path never matches the macOS image path under Catalyst (Unity loads
//! GCController this way). Disable with PEAR_NO_FOUNDATION_GUARDS=1,
//! PEAR_NO_SIGNAL_LOG, PEAR_NO_BUNDLE_CLASS_FIX.

use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, msg3, nsstring, sel, Class, Id, Imp, Sel, NIL};
use std::ffi::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};
use std::sync::Once;

extern "C" {
    #[link_name = "_Z15PearLogFilePathv"]
    fn PearLogFilePath() -> Id;
    fn NSSetUncaughtExceptionHandler(handler: unsafe extern "C" fn(Id));
    fn class_getImageName(c: Class) -> *const c_char;
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn write(fd: c_int, buf: *const c_void, len: usize) -> isize;
    fn backtrace(frames: *mut *mut c_void, size: c_int) -> c_int;
    fn backtrace_symbols_fd(frames: *const *mut c_void, size: c_int, fd: c_int);
    fn sigaction(sig: c_int, act: *const SigAction, old: *mut SigAction) -> c_int;
    fn raise(sig: c_int) -> c_int;
}

fn guards_disabled() -> bool {
    matches!(std::env::var("PEAR_NO_FOUNDATION_GUARDS"), Ok(v) if !v.is_empty() && v != "0")
}

unsafe fn nil_data_error(operation: &str) -> Id {
    let desc = nsstring(&format!("{operation} received nil data"));
    let info: Id = msg2(cls("NSDictionary"), "dictionaryWithObject:forKey:", desc, nsstring("NSLocalizedDescription"));
    msg3(cls("NSError"), "errorWithDomain:code:userInfo:", nsstring("NSCocoaErrorDomain"), 4864isize, info)
}

unsafe fn set_error(error: *mut Id, operation: &str) {
    if !error.is_null() {
        *error = nil_data_error(operation);
    }
}

macro_rules! original {
    ($slot:ident, $t:ty) => {{
        let f: $t = std::mem::transmute($slot.load(Ordering::Acquire));
        f
    }};
}

static JSON_DATA: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static JSON_STREAM: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static PLIST: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static UNARCHIVED: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static UNARCHIVER_INIT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static UNARCHIVER_LEGACY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static XML_INIT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static IMAGE_DATA: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static IMAGE_DATA_SCALE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static CLASS_NAMED: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "C" fn json_data(s: Id, c: Sel, data: Id, options: usize, error: *mut Id) -> Id {
    if data.is_null() {
        set_error(error, "JSONObjectWithData:");
        rt::log("[PearRuntime][data] JSON parse skipped: nil data");
        return NIL;
    }
    original!(JSON_DATA, unsafe extern "C" fn(Id, Sel, Id, usize, *mut Id) -> Id)(s, c, data, options, error)
}
unsafe extern "C" fn json_stream(s: Id, c: Sel, stream: Id, options: usize, error: *mut Id) -> Id {
    if stream.is_null() {
        set_error(error, "JSONObjectWithStream:");
        return NIL;
    }
    original!(JSON_STREAM, unsafe extern "C" fn(Id, Sel, Id, usize, *mut Id) -> Id)(s, c, stream, options, error)
}
unsafe extern "C" fn plist(s: Id, c: Sel, data: Id, options: usize, format: *mut usize, error: *mut Id) -> Id {
    if data.is_null() {
        set_error(error, "propertyListWithData:");
        return NIL;
    }
    original!(PLIST, unsafe extern "C" fn(Id, Sel, Id, usize, *mut usize, *mut Id) -> Id)(s, c, data, options, format, error)
}
unsafe extern "C" fn unarchived(s: Id, c: Sel, class: Class, data: Id, error: *mut Id) -> Id {
    if data.is_null() {
        set_error(error, "unarchivedObjectOfClass:fromData:error:");
        return NIL;
    }
    original!(UNARCHIVED, unsafe extern "C" fn(Id, Sel, Class, Id, *mut Id) -> Id)(s, c, class, data, error)
}
/// Initializers consume their receiver: a refused one is released.
unsafe extern "C" fn unarchiver_init(s: Id, c: Sel, data: Id, error: *mut Id) -> Id {
    if data.is_null() {
        set_error(error, "initForReadingFromData:error:");
        pear_objc::objc_release(s);
        return NIL;
    }
    original!(UNARCHIVER_INIT, unsafe extern "C" fn(Id, Sel, Id, *mut Id) -> Id)(s, c, data, error)
}
unsafe extern "C" fn unarchiver_legacy(s: Id, c: Sel, data: Id) -> Id {
    if data.is_null() {
        pear_objc::objc_release(s);
        return NIL;
    }
    original!(UNARCHIVER_LEGACY, unsafe extern "C" fn(Id, Sel, Id) -> Id)(s, c, data)
}
unsafe extern "C" fn xml_init(s: Id, c: Sel, data: Id) -> Id {
    if data.is_null() {
        pear_objc::objc_release(s);
        return NIL;
    }
    original!(XML_INIT, unsafe extern "C" fn(Id, Sel, Id) -> Id)(s, c, data)
}
unsafe extern "C" fn image_data(s: Id, c: Sel, data: Id) -> Id {
    if data.is_null() {
        rt::log("[PearRuntime][data] imageWithData: skipped: nil data");
        return NIL;
    }
    original!(IMAGE_DATA, unsafe extern "C" fn(Id, Sel, Id) -> Id)(s, c, data)
}
unsafe extern "C" fn image_data_scale(s: Id, c: Sel, data: Id, scale: f64) -> Id {
    if data.is_null() {
        return NIL;
    }
    original!(IMAGE_DATA_SCALE, unsafe extern "C" fn(Id, Sel, Id, f64) -> Id)(s, c, data, scale)
}

unsafe fn hook(class: Class, meta: bool, selector: &str, imp: Imp, slot: &AtomicPtr<c_void>) {
    if class.is_null() {
        return;
    }
    let s = sel(selector);
    let m = if meta { rt::class_getClassMethod(class, s) } else { rt::class_getInstanceMethod(class, s) };
    if !m.is_null() {
        slot.store(rt::method_getImplementation(m) as *mut c_void, Ordering::Release);
        rt::method_setImplementation(m, imp);
    }
}

unsafe fn install_serialization_guards() {
    let json = cls("NSJSONSerialization");
    hook(json, true, "JSONObjectWithData:options:error:", json_data as Imp, &JSON_DATA);
    hook(json, true, "JSONObjectWithStream:options:error:", json_stream as Imp, &JSON_STREAM);
    hook(cls("NSPropertyListSerialization"), true, "propertyListWithData:options:format:error:", plist as Imp, &PLIST);
    let unarchiver = cls("NSKeyedUnarchiver");
    hook(unarchiver, true, "unarchivedObjectOfClass:fromData:error:", unarchived as Imp, &UNARCHIVED);
    // The streaming initializer is the other common entry point (game SDKs
    // use it for cached download manifests).
    hook(unarchiver, false, "initForReadingFromData:error:", unarchiver_init as Imp, &UNARCHIVER_INIT);
    hook(unarchiver, false, "initForReadingWithData:", unarchiver_legacy as Imp, &UNARCHIVER_LEGACY);
    hook(cls("NSXMLParser"), false, "initWithData:", xml_init as Imp, &XML_INIT);
}

unsafe fn install_image_guards() {
    let image = cls("UIImage");
    hook(image, true, "imageWithData:", image_data as Imp, &IMAGE_DATA);
    hook(image, true, "imageWithData:scale:", image_data_scale as Imp, &IMAGE_DATA_SCALE);
}

unsafe extern "C" fn log_uncaught_exception(exception: Id) {
    let symbols: Id = msg0(exception, "callStackSymbols");
    let stack = rt::ns_string(msg1(symbols, "componentsJoinedByString:", nsstring("\n")));
    rt::log(&format!(
        "[PearRuntime][exception] {}: {}\n{stack}",
        rt::ns_string(msg0(exception, "name")),
        rt::ns_string(msg0(exception, "reason"))
    ));
}

// ---- crash signal log (async-signal-safe: write, backtrace, sigaction, raise)

#[repr(C)]
#[derive(Copy, Clone)]
struct SigAction {
    handler: usize,
    mask: u32,
    flags: c_int,
}

const NSIG: usize = 32;
const SIG_DFL: usize = 0;
const SIG_IGN: usize = 1;
const SA_RESTART: c_int = 0x2;
const SIGNALS: [c_int; 6] = [11, 6, 10, 4, 5, 8]; // SEGV ABRT BUS ILL TRAP FPE

static CRASH_FD: AtomicI32 = AtomicI32::new(-1);
static IN_HANDLER: AtomicBool = AtomicBool::new(false);
static mut PREVIOUS: [SigAction; NSIG] = [SigAction { handler: 0, mask: 0, flags: 0 }; NSIG];

extern "C" fn crash_signal_handler(signal: c_int) {
    unsafe {
        if IN_HANDLER.swap(true, Ordering::SeqCst) {
            let fallback = SigAction { handler: SIG_DFL, mask: 0, flags: 0 };
            sigaction(signal, &fallback, std::ptr::null_mut());
            raise(signal);
            return;
        }
        let fd = CRASH_FD.load(Ordering::Relaxed);
        if fd >= 0 {
            // "\n[PearRuntime][signal] received signal N\n" without allocation.
            let mut buf = [0u8; 64];
            let prefix = b"\n[PearRuntime][signal] received signal ";
            buf[..prefix.len()].copy_from_slice(prefix);
            let mut n = prefix.len();
            let mut digits = [0u8; 4];
            let mut d = 0;
            let mut v = signal.max(0) as u32;
            loop {
                digits[d] = b'0' + (v % 10) as u8;
                d += 1;
                v /= 10;
                if v == 0 || d == digits.len() {
                    break;
                }
            }
            while d > 0 {
                d -= 1;
                buf[n] = digits[d];
                n += 1;
            }
            buf[n] = b'\n';
            n += 1;
            write(fd, buf.as_ptr() as *const c_void, n);
            let mut frames = [std::ptr::null_mut::<c_void>(); 64];
            let count = backtrace(frames.as_mut_ptr(), 64);
            if count > 0 {
                backtrace_symbols_fd(frames.as_ptr(), count, fd);
            }
        }
        let mut previous = (*std::ptr::addr_of!(PREVIOUS))[signal as usize % NSIG];
        if previous.handler == SIG_IGN {
            previous.handler = SIG_DFL;
        }
        sigaction(signal, &previous, std::ptr::null_mut());
        raise(signal);
    }
}

unsafe fn install_crash_signal_log() {
    if rt::env_set("PEAR_NO_SIGNAL_LOG") {
        return;
    }
    let path = rt::ns_string(PearLogFilePath());
    if path.is_empty() {
        return;
    }
    let Ok(cpath) = std::ffi::CString::new(path) else { return };
    let fd = open(cpath.as_ptr(), 0x0001 | 0x0008 | 0x0200, 0o644 as c_int); // O_WRONLY|O_APPEND|O_CREAT
    if fd < 0 {
        return;
    }
    CRASH_FD.store(fd, Ordering::Relaxed);
    let previous = std::ptr::addr_of_mut!(PREVIOUS) as *mut SigAction;
    for sig in SIGNALS {
        let action = SigAction { handler: crash_signal_handler as usize, mask: 0, flags: SA_RESTART };
        sigaction(sig, &action, previous.add(sig as usize));
    }
}

unsafe fn install_foundation_guards() {
    if guards_disabled() {
        return;
    }
    install_serialization_guards();
    install_image_guards();
    NSSetUncaughtExceptionHandler(log_uncaught_exception);
    install_crash_signal_log();
    rt::log("[PearRuntime][data] Foundation nil-data guards installed");
}

unsafe extern "C" fn class_named(s: Id, c: Sel, name: Id) -> Class {
    let found = original!(CLASS_NAMED, unsafe extern "C" fn(Id, Sel, Id) -> Class)(s, c, name);
    if !found.is_null() || name.is_null() || msg0::<usize>(name, "length") == 0 {
        return found;
    }
    let bundle_path = rt::ns_string(msg0(s, "bundlePath"));
    if !bundle_path.starts_with("/System/") {
        return std::ptr::null_mut();
    }
    let candidate = cls(&rt::ns_string(name));
    if candidate.is_null() {
        return std::ptr::null_mut();
    }
    let image = rt::cstr(class_getImageName(candidate)).to_string();
    if image.is_empty() {
        return std::ptr::null_mut();
    }
    let last = bundle_path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if !image.starts_with(&format!("{bundle_path}/")) && !image.contains(&format!("/{last}/")) {
        return std::ptr::null_mut();
    }
    static LOGGED: Once = Once::new();
    LOGGED.call_once(|| {
        rt::log(&format!("[PearRuntime][bundle] {last} classNamed:{} resolved from {image}", rt::ns_string(name)));
    });
    candidate
}

unsafe fn install_system_bundle_class_lookup() {
    if rt::env_set("PEAR_NO_BUNDLE_CLASS_FIX") {
        return;
    }
    hook(cls("NSBundle"), false, "classNamed:", class_named as Imp, &CLASS_NAMED);
}

pub fn init() {
    if !rt::module_enabled("core") {
        return;
    }
    unsafe {
        install_system_bundle_class_lookup();
        install_foundation_guards();
    }
}
