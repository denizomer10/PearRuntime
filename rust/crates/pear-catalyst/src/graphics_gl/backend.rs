//! ANGLE loader: one immutable Metal-backed EGL display per process. No
//! system-EAGL fallback; callers handle initialization failure explicitly.

use crate::rt;
use pear_objc::{msg0, Id};
use std::ffi::{c_char, c_int, c_void, CStr, CString, VaList};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

pub type EglDisplay = *mut c_void;
pub const EGL_NO_DISPLAY: EglDisplay = std::ptr::null_mut();

extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
    fn dlerror() -> *const c_char;
    fn vsnprintf(buf: *mut c_char, size: usize, format: *const c_char, args: VaList) -> c_int;
}
const RTLD_NOW: c_int = 0x2;
const RTLD_LOCAL: c_int = 0x4;

/// Writes `text` to stderr and appends it to PEAR_LOG_FILE (printf-style
/// lines already carry their own newline).
pub fn trace(text: &str) {
    let _ = std::io::stderr().write_all(text.as_bytes());
    if let Some(path) = std::env::var_os("PEAR_LOG_FILE") {
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
            let _ = f.write_all(text.as_bytes());
        }
    }
}

/// `PearANGLETrace(format, ...)`: printf formatting for C/ObjC callers.
#[export_name = "_Z14PearANGLETracePKcz"]
pub unsafe extern "C" fn pear_angle_trace(format: *const c_char, args: ...) {
    if format.is_null() {
        return;
    }
    let mut buf = vec![0u8; 4096];
    let n = vsnprintf(buf.as_mut_ptr() as *mut c_char, buf.len(), format, args.clone());
    if n < 0 {
        return;
    }
    if n as usize >= buf.len() {
        buf = vec![0u8; n as usize + 1];
        vsnprintf(buf.as_mut_ptr() as *mut c_char, buf.len(), format, args);
    }
    buf.truncate(n as usize);
    trace(&String::from_utf8_lossy(&buf));
}

struct Backend {
    directory: String,
}

static LOCK: Mutex<Option<Backend>> = Mutex::new(None);
static READY: AtomicBool = AtomicBool::new(false);
static LIBRARY: AtomicUsize = AtomicUsize::new(0);
static DISPLAY: AtomicUsize = AtomicUsize::new(0);
static GET_PROC: AtomicUsize = AtomicUsize::new(0);

unsafe fn fail(error: *mut Id, code: isize, reason: &str) -> bool {
    if !error.is_null() {
        *error = rt::error("PearRuntime.ANGLE", code, reason);
    }
    false
}

const EGL_PLATFORM_ANGLE_ANGLE: u32 = 0x3202;
const EGL_PLATFORM_ANGLE_TYPE_ANGLE: i32 = 0x3203;
const EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE: i32 = 0x3489;
const EGL_NONE: i32 = 0x3038;

type GetProc = unsafe extern "C" fn(*const c_char) -> *mut c_void;

#[export_name = "_Z19PearANGLEInitializeP8NSStringPU15__autoreleasingP7NSError"]
pub unsafe extern "C" fn initialize(directory: Id, error: *mut Id) -> bool {
    let mut guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if directory.is_null() || !msg0::<bool>(directory, "isAbsolutePath") {
        return fail(error, 1, "ANGLE requires an absolute library directory");
    }
    let selected = rt::ns_string(msg0(directory, "stringByStandardizingPath"));
    if READY.load(Ordering::Acquire) {
        let same = guard.as_ref().is_some_and(|b| b.directory == selected);
        return if same { true } else { fail(error, 2, "ANGLE backend cannot change within a running process") };
    }
    let path = CString::new(rt::path_join(&selected, "libEGL.dylib")).unwrap_or_default();
    let library = dlopen(path.as_ptr(), RTLD_NOW | RTLD_LOCAL);
    if library.is_null() {
        let reason = dlerror();
        let reason = if reason.is_null() { "unknown".to_string() } else { CStr::from_ptr(reason).to_string_lossy().into_owned() };
        return fail(error, 3, &format!("ANGLE load failed: {reason}"));
    }
    let proc_ptr = dlsym(library, c"eglGetProcAddress".as_ptr());
    let init_ptr = dlsym(library, c"eglInitialize".as_ptr());
    let platform_ptr = if proc_ptr.is_null() {
        std::ptr::null_mut()
    } else {
        std::mem::transmute::<_, GetProc>(proc_ptr)(c"eglGetPlatformDisplayEXT".as_ptr())
    };
    if proc_ptr.is_null() || init_ptr.is_null() || platform_ptr.is_null() {
        dlclose(library);
        return fail(error, 4, "ANGLE library lacks the required EGL entry points");
    }
    let platform: unsafe extern "C" fn(u32, *mut c_void, *const i32) -> EglDisplay = std::mem::transmute(platform_ptr);
    let init: unsafe extern "C" fn(EglDisplay, *mut i32, *mut i32) -> u32 = std::mem::transmute(init_ptr);
    let attributes = [EGL_PLATFORM_ANGLE_TYPE_ANGLE, EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE, EGL_NONE];
    let display = platform(EGL_PLATFORM_ANGLE_ANGLE, std::ptr::null_mut(), attributes.as_ptr());
    let (mut major, mut minor) = (0, 0);
    if display.is_null() || init(display, &mut major, &mut minor) == 0 {
        dlclose(library);
        return fail(error, 5, "ANGLE could not initialize its Metal display");
    }
    // The display/library stay alive for the process: existing EGL contexts
    // must never be invalidated by a reload or a settings change.
    LIBRARY.store(library as usize, Ordering::Release);
    DISPLAY.store(display as usize, Ordering::Release);
    GET_PROC.store(proc_ptr as usize, Ordering::Release);
    *guard = Some(Backend { directory: selected });
    READY.store(true, Ordering::Release);
    true
}

#[export_name = "_Z23PearANGLEGetProcAddressPKc"]
pub unsafe extern "C" fn get_proc_address(name: *const c_char) -> *mut c_void {
    if name.is_null() || *name == 0 || !READY.load(Ordering::Acquire) {
        return std::ptr::null_mut();
    }
    let symbol = dlsym(LIBRARY.load(Ordering::Acquire) as *mut c_void, name);
    if !symbol.is_null() {
        return symbol;
    }
    let get: GetProc = std::mem::transmute(GET_PROC.load(Ordering::Acquire));
    get(name)
}

/// Typed lookup helper for Rust callers.
pub unsafe fn proc<T: Copy>(name: &CStr) -> Option<T> {
    let p = get_proc_address(name.as_ptr());
    if p.is_null() { None } else { Some(std::mem::transmute_copy(&p)) }
}

#[export_name = "_Z16PearANGLEDisplayv"]
pub extern "C" fn display() -> EglDisplay {
    if READY.load(Ordering::Acquire) { DISPLAY.load(Ordering::Acquire) as EglDisplay } else { EGL_NO_DISPLAY }
}

