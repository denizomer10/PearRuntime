//! Clean-room: unified PearRuntime facade tying together `pear-macho`,
//! `pear-dyld`, `pear-libobjc`, `pear-foundation`, `pear-host` (+ OS backend),
//! and `pear-image` with zero external crates.io dependencies.

pub use pear_dyld;
pub use pear_foundation;
pub use pear_graphics;
pub use pear_host;
pub use pear_image;
pub use pear_libobjc;
pub use pear_macho;

#[cfg(target_os = "linux")]
pub use pear_host_linux;

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void};
use std::sync::Once;
use pear_libobjc::abi::{ClassT, Id, Imp, Sel};
use pear_libobjc::object::{alloc, EMPTY_CACHE, NSOBJECT_CLASS, NSOBJECT_META, StaticClass};
use pear_libobjc::runtime as objc_rt;

macro_rules! guest_class {
    ($cls:ident, $meta:ident) => {
        static $meta: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const NSOBJECT_META as *const ClassT,
            superclass: &raw const NSOBJECT_META as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
        static $cls: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const $meta as *const ClassT,
            superclass: &raw const NSOBJECT_CLASS as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
    };
}

guest_class!(CALAYER_CLASS, CALAYER_META);
guest_class!(CAMETALLAYER_CLASS, CAMETALLAYER_META);
guest_class!(CAEAGLLAYER_CLASS, CAEAGLLAYER_META);
guest_class!(CATRANSACTION_CLASS, CATRANSACTION_META);
guest_class!(CADISPLAYLINK_CLASS, CADISPLAYLINK_META);
guest_class!(UIAPPLICATION_CLASS, UIAPPLICATION_META);
guest_class!(UIWINDOW_CLASS, UIWINDOW_META);
guest_class!(UIVIEW_CLASS, UIVIEW_META);
guest_class!(UISCREEN_CLASS, UISCREEN_META);
guest_class!(UICOLOR_CLASS, UICOLOR_META);
guest_class!(UIIMAGE_CLASS, UIIMAGE_META);
guest_class!(UILABEL_CLASS, UILABEL_META);
guest_class!(UIFONT_CLASS, UIFONT_META);
guest_class!(UIDEVICE_CLASS, UIDEVICE_META);
guest_class!(UIRESPONDER_CLASS, UIRESPONDER_META);
guest_class!(UIVIEWCONTROLLER_CLASS, UIVIEWCONTROLLER_META);
guest_class!(UIGESTURERECOGNIZER_CLASS, UIGESTURERECOGNIZER_META);
guest_class!(UILONGPRESSGESTURERECOGNIZER_CLASS, UILONGPRESSGESTURERECOGNIZER_META);
guest_class!(UITAPGESTURERECOGNIZER_CLASS, UITAPGESTURERECOGNIZER_META);
guest_class!(UIPANGESTURERECOGNIZER_CLASS, UIPANGESTURERECOGNIZER_META);
guest_class!(UISTORYBOARD_CLASS, UISTORYBOARD_META);
guest_class!(UICONTROL_CLASS, UICONTROL_META);
guest_class!(UIBUTTON_CLASS, UIBUTTON_META);
guest_class!(GLKVIEW_CLASS, GLKVIEW_META);
guest_class!(GLKVIEWCONTROLLER_CLASS, GLKVIEWCONTROLLER_META);
guest_class!(MTKVIEW_CLASS, MTKVIEW_META);
guest_class!(EAGLCONTEXT_CLASS, EAGLCONTEXT_META);
guest_class!(UIDOCUMENT_CLASS, UIDOCUMENT_META);
guest_class!(UIACCESSIBILITYELEMENT_CLASS, UIACCESSIBILITYELEMENT_META);
guest_class!(UITEXTPOSITION_CLASS, UITEXTPOSITION_META);
guest_class!(UITEXTRANGE_CLASS, UITEXTRANGE_META);
guest_class!(UITEXTFIELD_CLASS, UITEXTFIELD_META);
guest_class!(UITEXTVIEW_CLASS, UITEXTVIEW_META);
guest_class!(UISCROLLVIEW_CLASS, UISCROLLVIEW_META);
guest_class!(UINAVIGATIONCONTROLLER_CLASS, UINAVIGATIONCONTROLLER_META);
guest_class!(UIALERTCONTROLLER_CLASS, UIALERTCONTROLLER_META);
guest_class!(SKREQUEST_CLASS, SKREQUEST_META);
guest_class!(SKPRODUCTSREQUEST_CLASS, SKPRODUCTSREQUEST_META);

extern "C" fn shared_app_stub(_self: Id, _cmd: Sel) -> Id {
    static APP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *APP.get_or_init(|| unsafe { alloc(UIAPPLICATION_CLASS.ptr()) } as usize) as Id
}

extern "C" fn main_screen_stub(_self: Id, _cmd: Sel) -> Id {
    static SCR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *SCR.get_or_init(|| unsafe { alloc(UISCREEN_CLASS.ptr()) } as usize) as Id
}

extern "C" fn screen_scale_stub(_self: Id, _cmd: Sel) -> f64 {
    pear_host::host().screen().scale
}

extern "C" fn current_device_stub(_self: Id, _cmd: Sel) -> Id {
    static DEV: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *DEV.get_or_init(|| unsafe { alloc(UIDEVICE_CLASS.ptr()) } as usize) as Id
}

extern "C" fn device_system_name(_self: Id, _cmd: Sel) -> Id {
    pear_foundation::nsstring_from_rust("iOS")
}

extern "C" fn noop_method(_self: Id, _cmd: Sel) {}

fn init_guest_frameworks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let i = |p: *const ()| p as Imp;
        for (cls, meta, name) in [
            (&CALAYER_CLASS, &CALAYER_META, "CALayer"),
            (&CAMETALLAYER_CLASS, &CAMETALLAYER_META, "CAMetalLayer"),
            (&CAEAGLLAYER_CLASS, &CAEAGLLAYER_META, "CAEAGLLayer"),
            (&CATRANSACTION_CLASS, &CATRANSACTION_META, "CATransaction"),
            (&CADISPLAYLINK_CLASS, &CADISPLAYLINK_META, "CADisplayLink"),
            (&UIWINDOW_CLASS, &UIWINDOW_META, "UIWindow"),
            (&UIVIEW_CLASS, &UIVIEW_META, "UIView"),
            (&UICOLOR_CLASS, &UICOLOR_META, "UIColor"),
            (&UIIMAGE_CLASS, &UIIMAGE_META, "UIImage"),
            (&UILABEL_CLASS, &UILABEL_META, "UILabel"),
            (&UIFONT_CLASS, &UIFONT_META, "UIFont"),
            (&UIRESPONDER_CLASS, &UIRESPONDER_META, "UIResponder"),
            (&UIVIEWCONTROLLER_CLASS, &UIVIEWCONTROLLER_META, "UIViewController"),
            (&UIGESTURERECOGNIZER_CLASS, &UIGESTURERECOGNIZER_META, "UIGestureRecognizer"),
            (&UILONGPRESSGESTURERECOGNIZER_CLASS, &UILONGPRESSGESTURERECOGNIZER_META, "UILongPressGestureRecognizer"),
            (&UITAPGESTURERECOGNIZER_CLASS, &UITAPGESTURERECOGNIZER_META, "UITapGestureRecognizer"),
            (&UIPANGESTURERECOGNIZER_CLASS, &UIPANGESTURERECOGNIZER_META, "UIPanGestureRecognizer"),
            (&UISTORYBOARD_CLASS, &UISTORYBOARD_META, "UIStoryboard"),
            (&UICONTROL_CLASS, &UICONTROL_META, "UIControl"),
            (&UIBUTTON_CLASS, &UIBUTTON_META, "UIButton"),
            (&GLKVIEW_CLASS, &GLKVIEW_META, "GLKView"),
            (&GLKVIEWCONTROLLER_CLASS, &GLKVIEWCONTROLLER_META, "GLKViewController"),
            (&MTKVIEW_CLASS, &MTKVIEW_META, "MTKView"),
            (&EAGLCONTEXT_CLASS, &EAGLCONTEXT_META, "EAGLContext"),
            (&UIDOCUMENT_CLASS, &UIDOCUMENT_META, "UIDocument"),
            (&UIACCESSIBILITYELEMENT_CLASS, &UIACCESSIBILITYELEMENT_META, "UIAccessibilityElement"),
            (&UITEXTPOSITION_CLASS, &UITEXTPOSITION_META, "UITextPosition"),
            (&UITEXTRANGE_CLASS, &UITEXTRANGE_META, "UITextRange"),
            (&UITEXTFIELD_CLASS, &UITEXTFIELD_META, "UITextField"),
            (&UITEXTVIEW_CLASS, &UITEXTVIEW_META, "UITextView"),
            (&UISCROLLVIEW_CLASS, &UISCROLLVIEW_META, "UIScrollView"),
            (&UINAVIGATIONCONTROLLER_CLASS, &UINAVIGATIONCONTROLLER_META, "UINavigationController"),
            (&UIALERTCONTROLLER_CLASS, &UIALERTCONTROLLER_META, "UIAlertController"),
            (&SKREQUEST_CLASS, &SKREQUEST_META, "SKRequest"),
            (&SKPRODUCTSREQUEST_CLASS, &SKPRODUCTSREQUEST_META, "SKProductsRequest"),
        ] {
            objc_rt::register_builtin(cls.ptr(), meta.ptr(), name, 32, &[], &[]);
        }
        objc_rt::register_builtin(
            UIAPPLICATION_CLASS.ptr(),
            UIAPPLICATION_META.ptr(),
            "UIApplication",
            32,
            &[],
            &[("sharedApplication", i(shared_app_stub as *const ()))],
        );
        objc_rt::register_builtin(
            UISCREEN_CLASS.ptr(),
            UISCREEN_META.ptr(),
            "UIScreen",
            32,
            &[("scale", i(screen_scale_stub as *const ()))],
            &[("mainScreen", i(main_screen_stub as *const ()))],
        );
        objc_rt::register_builtin(
            UIDEVICE_CLASS.ptr(),
            UIDEVICE_META.ptr(),
            "UIDevice",
            32,
            &[
                ("systemName", i(device_system_name as *const ())),
                ("beginGeneratingDeviceOrientationNotifications", i(noop_method as *const ())),
            ],
            &[("currentDevice", i(current_device_stub as *const ()))],
        );
    });
}

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn realloc(p: *mut c_void, size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
    fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
    fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void;
    fn strlen(s: *const c_char) -> usize;
    fn strcmp(a: *const c_char, b: *const c_char) -> i32;
    fn getenv(name: *const c_char) -> *const c_char;
    fn abort() -> !;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

fn libc_dlsym(macho_sym: &str) -> Option<usize> {
    let cname = macho_sym.strip_prefix('_')?;
    let cstr = std::ffi::CString::new(cname).ok()?;
    #[cfg(target_os = "macos")]
    let default_handle = -2isize as *mut c_void;
    #[cfg(not(target_os = "macos"))]
    let default_handle = std::ptr::null_mut();
    let ptr = unsafe { dlsym(default_handle, cstr.as_ptr()) };
    if ptr.is_null() { None } else { Some(ptr as usize) }
}

extern "C" fn ca_current_media_time() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

static STACK_CHK_GUARD: usize = 0x5065_6172_5369_6c69;

extern "C" fn stack_chk_fail_trap() -> ! {
    eprintln!("[pear-runtime] ___stack_chk_fail");
    std::process::abort()
}

extern "C" fn tls_errno_ptr() -> *mut i32 {
    thread_local! {
        static ERRNO: std::cell::UnsafeCell<i32> = const { std::cell::UnsafeCell::new(0) };
    }
    ERRNO.with(|e| e.get())
}

extern "C" fn memcpy_chk(dst: *mut c_void, src: *const c_void, n: usize, _dstlen: usize) -> *mut c_void {
    unsafe { memcpy(dst, src, n) }
}

#[repr(C)]
struct SincosfStret {
    sinval: f32,
    cosval: f32,
}

extern "C" fn sincosf_stret(x: f32) -> SincosfStret {
    let (s, c) = x.sin_cos();
    SincosfStret { sinval: s, cosval: c }
}

#[repr(C)]
struct SincosStret {
    sinval: f64,
    cosval: f64,
}

extern "C" fn sincos_stret(x: f64) -> SincosStret {
    let (s, c) = x.sin_cos();
    SincosStret { sinval: s, cosval: c }
}

extern "C" fn acos_impl(x: f64) -> f64 { x.acos() }
extern "C" fn tolower_impl(c: i32) -> i32 {
    if (0..128).contains(&c) { (c as u8).to_ascii_lowercase() as i32 } else { c }
}
extern "C" fn toupper_impl(c: i32) -> i32 {
    if (0..128).contains(&c) { (c as u8).to_ascii_uppercase() as i32 } else { c }
}
extern "C" fn os_atomic_add32(val: i32, ptr: *mut i32) -> i32 {
    if ptr.is_null() { return 0; }
    unsafe {
        let a = &*(ptr as *const std::sync::atomic::AtomicI32);
        a.fetch_add(val, std::sync::atomic::Ordering::SeqCst) + val
    }
}
extern "C" fn arc4random_impl() -> u32 {
    static STATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x5065_6172_3132_3334);
    let mut x = STATE.load(std::sync::atomic::Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, std::sync::atomic::Ordering::Relaxed);
    x as u32
}
extern "C" fn availability_version_check(_count: u32, _versions: *const c_void) -> bool { true }

extern "C" fn dispatch_once_f_impl(pred: *mut usize, ctx: *mut c_void, func: extern "C" fn(*mut c_void)) {
    if pred.is_null() { return; }
    unsafe {
        let p = &*(pred as *const std::sync::atomic::AtomicUsize);
        if p.swap(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            func(ctx);
        }
    }
}

extern "C" fn mach_absolute_time_impl() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

extern "C" fn mach_timebase_info_impl(info: *mut MachTimebaseInfo) -> i32 {
    if !info.is_null() {
        unsafe {
            (*info).numer = 1;
            (*info).denom = 1;
        }
    }
    0
}

static MACH_TASK_SELF_STUB: u32 = 1;

unsafe extern "C" fn memset_pattern16_impl(b: *mut u8, pattern16: *const u8, len: usize) {
    if b.is_null() || pattern16.is_null() { return; }
    let pat = std::slice::from_raw_parts(pattern16, 16);
    for i in 0..len {
        *b.add(i) = pat[i & 15];
    }
}

extern "C" fn pthread_mach_thread_np_impl(_t: usize) -> u32 { 1 }
extern "C" fn sysctlbyname_impl(_n: *const c_char, _oldp: *mut c_void, _oldlenp: *mut usize, _newp: *const c_void, _newlen: usize) -> i32 { -1 }
extern "C" fn task_info_impl(_task: u32, _flavor: u32, _info: *mut c_void, _count: *mut u32) -> i32 { -1 }
extern "C" fn thread_policy_set_impl(_t: u32, _flavor: u32, _policy: *const c_void, _count: u32) -> i32 { 0 }

extern "C" fn exp10_impl(x: f64) -> f64 { 10.0f64.powf(x) }
extern "C" fn acosf_impl(x: f32) -> f32 { x.acos() }
extern "C" fn asinf_impl(x: f32) -> f32 { x.asin() }
extern "C" fn atan2f_impl(y: f32, x: f32) -> f32 { y.atan2(x) }
extern "C" fn cxa_atexit_noop(_f: *const c_void, _arg: *const c_void, _dso: *const c_void) -> i32 { 0 }
extern "C" fn dyld_image_count_stub() -> u32 { 1 }
extern "C" fn dyld_get_image_name_stub(_idx: u32) -> *const c_char { c"PearGuest".as_ptr() }

static DISPATCH_MAIN_Q: [usize; 16] = [0; 16];
static DEFAULT_RUNE_LOCALE: [u8; 4096] = [0; 4096];
static mut STDIN_STUB: usize = 0;
static mut STDOUT_STUB: usize = 0;
static mut STDERR_STUB: usize = 0;

extern "C" fn dispatch_once_impl(pred: *mut usize, block: *mut c_void) {
    if pred.is_null() || block.is_null() {
        return;
    }
    unsafe {
        let p = &*(pred as *const std::sync::atomic::AtomicUsize);
        if p.swap(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            let invoke = *((block as *const usize).add(2));
            let f: extern "C" fn(*mut c_void) = std::mem::transmute(invoke);
            f(block);
        }
    }
}

extern "C" fn dispatch_sync_impl(_q: *mut c_void, block: *mut c_void) {
    if !block.is_null() {
        unsafe {
            let invoke = *((block as *const usize).add(2));
            let f: extern "C" fn(*mut c_void) = std::mem::transmute(invoke);
            f(block);
        }
    }
}

extern "C" fn dispatch_get_global_queue_impl(_id: isize, _flags: usize) -> *const c_void {
    &raw const DISPATCH_MAIN_Q as *const c_void
}

extern "C" fn maskrune_impl(c: i32, f: usize) -> i32 {
    if (0..128).contains(&c) {
        let ch = c as u8 as char;
        let ok = match f {
            0x0100 => ch.is_ascii_alphabetic(),
            0x0400 => ch.is_ascii_digit(),
            0x4000 => ch.is_ascii_whitespace(),
            _ => ch.is_ascii_alphanumeric(),
        };
        if ok { 1 } else { 0 }
    } else {
        0
    }
}

extern "C" fn ns_get_executable_path(buf: *mut c_char, bufsize: *mut u32) -> i32 {
    let path = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/PearGuest".to_string());
    let bytes = path.as_bytes();
    let needed = (bytes.len() + 1) as u32;
    if buf.is_null() || bufsize.is_null() || unsafe { *bufsize } < needed {
        if !bufsize.is_null() {
            unsafe { *bufsize = needed };
        }
        return -1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
        *buf.add(bytes.len()) = 0;
    }
    0
}

extern "C" fn status_ok_stub() -> i32 { 0 }
extern "C" fn ptr_null_stub() -> *mut c_void { std::ptr::null_mut() }
extern "C" fn false_stub() -> bool { false }

fn framework_const(key: &'static str) -> usize {
    static MAP: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, usize>>> =
        std::sync::OnceLock::new();
    let m = MAP.get_or_init(Default::default);
    let mut g = m.lock().unwrap();
    *g.entry(key).or_insert_with(|| pear_foundation::nsstring_from_rust(key) as usize)
}

fn audiotoolbox_symbol(symbol: &str) -> Option<usize> {
    let ok = status_ok_stub as *const () as usize;
    let null = ptr_null_stub as *const () as usize;
    match symbol {
        "_AudioComponentFindNext" => Some(null),
        "_AudioComponentInstanceNew"
        | "_AudioComponentInstanceDispose"
        | "_AudioConverterNew"
        | "_AudioConverterDispose"
        | "_AudioConverterReset"
        | "_AudioConverterGetProperty"
        | "_AudioConverterSetProperty"
        | "_AudioConverterFillComplexBuffer"
        | "_AudioFileOpenWithCallbacks"
        | "_AudioFileClose"
        | "_AudioFileGetGlobalInfo"
        | "_AudioFileGetProperty"
        | "_AudioFileGetPropertyInfo"
        | "_AudioFileReadPackets"
        | "_AudioFormatGetProperty"
        | "_AudioOutputUnitStart"
        | "_AudioOutputUnitStop"
        | "_AudioSessionInitialize"
        | "_AudioSessionSetActive"
        | "_AudioSessionSetProperty"
        | "_AudioUnitAddPropertyListener"
        | "_AudioUnitGetProperty"
        | "_AudioUnitSetProperty"
        | "_AudioUnitInitialize"
        | "_AudioUnitUninitialize"
        | "_ExtAudioFileOpenURL"
        | "_ExtAudioFileDispose"
        | "_ExtAudioFileGetProperty"
        | "_ExtAudioFileSetProperty"
        | "_ExtAudioFileRead"
        | "_ExtAudioFileSeek" => Some(ok),
        _ => None,
    }
}

fn cfnetwork_symbol(symbol: &str) -> Option<usize> {
    let null = ptr_null_stub as *const () as usize;
    let ok = status_ok_stub as *const () as usize;
    match symbol {
        "_CFHTTPMessageCreateRequest"
        | "_CFHTTPMessageCopySerializedMessage"
        | "_CFNetworkCopySystemProxySettings"
        | "_CFReadStreamCreateForHTTPRequest"
        | "_CFReadStreamCreateWithFTPURL" => Some(null),
        "_CFHTTPMessageAddAuthentication"
        | "_CFHTTPMessageAppendBytes"
        | "_CFHTTPMessageSetHeaderFieldValue" => Some(ok),
        "_kCFHTTPAuthenticationSchemeBasic" => Some(framework_const("Basic")),
        "_kCFHTTPVersion1_0" => Some(framework_const("HTTP/1.0")),
        "_kCFHTTPVersion1_1" => Some(framework_const("HTTP/1.1")),
        "_kCFStreamPropertyHTTPProxy" => Some(framework_const("HTTPProxy")),
        "_kCFStreamPropertyHTTPResponseHeader" => Some(framework_const("HTTPResponseHeader")),
        "_kCFStreamPropertySSLSettings" => Some(framework_const("SSLSettings")),
        "_kCFStreamSSLValidatesCertificateChain" => Some(framework_const("SSLValidatesCertificateChain")),
        "_kCFStreamPropertyFTPProxy" => Some(framework_const("FTPProxy")),
        "_kCFStreamPropertyFTPFetchResourceInfo" => Some(framework_const("FTPFetchResourceInfo")),
        "_kCFStreamPropertyFTPFileTransferOffset" => Some(framework_const("FTPFileTransferOffset")),
        "_kCFStreamPropertyFTPResourceSize" => Some(framework_const("FTPResourceSize")),
        "_kCFStreamPropertyFTPUsePassiveMode" => Some(framework_const("FTPUsePassiveMode")),
        _ => None,
    }
}

fn quartz_const(name: &str) -> usize {
    static MAP: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, usize>>> =
        std::sync::OnceLock::new();
    let m = MAP.get_or_init(Default::default);
    let mut g = m.lock().unwrap();
    let key: &'static str = match name {
        "_kCAFilterLinear" => "linear",
        "_kCAGravityResize" => "resize",
        "_kCAMediaTimingFunctionEaseInEaseOut" => "easeInEaseOut",
        "_kCAMediaTimingFunctionLinear" => "linear",
        "_kCAMediaTimingFunctionEaseOut" => "easeOut",
        "_kCAFillModeForwards" => "forwards",
        _ => "",
    };
    *g.entry(key).or_insert_with(|| pear_foundation::nsstring_from_rust(key) as usize)
}

extern "C" fn unwind_resume_trap() -> ! {
    eprintln!("[pear-runtime] _Unwind_Resume reached");
    std::process::abort()
}

/// Initializes the portable PearRuntime subsystems (ObjC runtime, Foundation,
/// and the platform's host backend when running on Linux).
pub fn init() {
    pear_libobjc::object::init_runtime();
    pear_libobjc::blocks::init_blocks();
    pear_foundation::init_foundation();
    init_guest_frameworks();
    #[cfg(target_os = "linux")]
    {
        pear_host_linux::LinuxHost::install().initialize();
    }
}

/// Unified symbol resolver for `pear-dyld`: resolves `libobjc`, `libSystem`
/// (libclosure + libc basics), `CoreFoundation`, and `Foundation` imports.
pub fn resolve_symbol(library: Option<&str>, symbol: &str) -> Option<usize> {
    init();
    match library {
        Some("/usr/lib/libobjc.A.dylib") => pear_libobjc::symbol(symbol),
        Some("/usr/lib/libSystem.B.dylib") => match symbol {
            "__Unwind_Resume" => Some(unwind_resume_trap as *const () as usize),
            "___stack_chk_guard" => Some(&raw const STACK_CHK_GUARD as usize),
            "___stack_chk_fail" => Some(stack_chk_fail_trap as *const () as usize),
            "___error" => Some(tls_errno_ptr as *const () as usize),
            "___memcpy_chk" => Some(memcpy_chk as *const () as usize),
            "___sincosf_stret" => Some(sincosf_stret as *const () as usize),
            "___sincos_stret" => Some(sincos_stret as *const () as usize),
            "___exp10" => Some(exp10_impl as *const () as usize),
            "_acos" => Some(acos_impl as *const () as usize),
            "_acosf" => Some(acosf_impl as *const () as usize),
            "_asinf" => Some(asinf_impl as *const () as usize),
            "_atan2f" => Some(atan2f_impl as *const () as usize),
            "___tolower" => Some(tolower_impl as *const () as usize),
            "___toupper" => Some(toupper_impl as *const () as usize),
            "_OSAtomicAdd32" => Some(os_atomic_add32 as *const () as usize),
            "_arc4random" => Some(arc4random_impl as *const () as usize),
            "__availability_version_check" => Some(availability_version_check as *const () as usize),
            "___cxa_atexit" => Some(cxa_atexit_noop as *const () as usize),
            "__dyld_image_count" => Some(dyld_image_count_stub as *const () as usize),
            "__dyld_get_image_name" => Some(dyld_get_image_name_stub as *const () as usize),
            "__dispatch_main_q" => Some(&raw const DISPATCH_MAIN_Q as usize),
            "_dispatch_once" => Some(dispatch_once_impl as *const () as usize),
            "_dispatch_once_f" => Some(dispatch_once_f_impl as *const () as usize),
            "_mach_absolute_time" => Some(mach_absolute_time_impl as *const () as usize),
            "_mach_timebase_info" => Some(mach_timebase_info_impl as *const () as usize),
            "_mach_task_self_" => Some(&raw const MACH_TASK_SELF_STUB as usize),
            "_memset_pattern16" => Some(memset_pattern16_impl as *const () as usize),
            "_pthread_mach_thread_np" => Some(pthread_mach_thread_np_impl as *const () as usize),
            "_sysctlbyname" => Some(sysctlbyname_impl as *const () as usize),
            "_task_info" => Some(task_info_impl as *const () as usize),
            "_thread_policy_set" => Some(thread_policy_set_impl as *const () as usize),
            "_dispatch_sync" | "_dispatch_async" => Some(dispatch_sync_impl as *const () as usize),
            "_dispatch_get_global_queue" => Some(dispatch_get_global_queue_impl as *const () as usize),
            "__DefaultRuneLocale" => Some(&raw const DEFAULT_RUNE_LOCALE as usize),
            "___maskrune" => Some(maskrune_impl as *const () as usize),
            "___stdinp" => Some(&raw const STDIN_STUB as usize),
            "___stdoutp" => Some(&raw const STDOUT_STUB as usize),
            "___stderrp" => Some(&raw const STDERR_STUB as usize),
            "__NSGetExecutablePath" => Some(ns_get_executable_path as *const () as usize),
            "_malloc" => Some(malloc as *const () as usize),
            "_calloc" => Some(calloc as *const () as usize),
            "_realloc" => Some(realloc as *const () as usize),
            "_free" => Some(free as *const () as usize),
            "_memcpy" => Some(memcpy as *const () as usize),
            "_memset" => Some(memset as *const () as usize),
            "_strlen" => Some(strlen as *const () as usize),
            "_strcmp" => Some(strcmp as *const () as usize),
            "_getenv" => Some(getenv as *const () as usize),
            "_abort" => Some(abort as *const () as usize),
            _ => pear_libobjc::symbol(symbol).or_else(|| libc_dlsym(symbol)),
        },
        Some(path) if path.contains("CoreFoundation") || path.contains("Foundation") => {
            pear_foundation::symbol(symbol)
        }
        Some(path) if path.contains("CoreGraphics") => pear_graphics::symbol(symbol),
        Some(path) if path.contains("QuartzCore") => match symbol {
            "_CACurrentMediaTime" => Some(ca_current_media_time as *const () as usize),
            "_OBJC_CLASS_$_CALayer" => Some(CALAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CAMetalLayer" => Some(CAMETALLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CAEAGLLayer" => Some(CAEAGLLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CATransaction" => Some(CATRANSACTION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CADisplayLink" => Some(CADISPLAYLINK_CLASS.ptr() as usize),
            "_kCAFilterLinear"
            | "_kCAGravityResize"
            | "_kCAMediaTimingFunctionEaseInEaseOut"
            | "_kCAMediaTimingFunctionLinear"
            | "_kCAMediaTimingFunctionEaseOut"
            | "_kCAFillModeForwards" => Some(quartz_const(symbol)),
            _ => pear_graphics::quartz_symbol(symbol),
        },
        Some(path) if path.contains("UIKit") => match symbol {
            "_OBJC_CLASS_$_UIApplication" => Some(UIAPPLICATION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIWindow" => Some(UIWINDOW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIView" => Some(UIVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIScreen" => Some(UISCREEN_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIColor" => Some(UICOLOR_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIImage" => Some(UIIMAGE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UILabel" => Some(UILABEL_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIFont" => Some(UIFONT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIDevice" => Some(UIDEVICE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIResponder" => Some(UIRESPONDER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIViewController" => Some(UIVIEWCONTROLLER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIGestureRecognizer" => Some(UIGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UILongPressGestureRecognizer" => Some(UILONGPRESSGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITapGestureRecognizer" => Some(UITAPGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIPanGestureRecognizer" => Some(UIPANGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIStoryboard" => Some(UISTORYBOARD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIControl" => Some(UICONTROL_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIButton" => Some(UIBUTTON_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIDocument" => Some(UIDOCUMENT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIAccessibilityElement" => Some(UIACCESSIBILITYELEMENT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextPosition" => Some(UITEXTPOSITION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextRange" => Some(UITEXTRANGE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextField" => Some(UITEXTFIELD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextView" => Some(UITEXTVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIScrollView" => Some(UISCROLLVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UINavigationController" => Some(UINAVIGATIONCONTROLLER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIAlertController" => Some(UIALERTCONTROLLER_CLASS.ptr() as usize),
            _ => None,
        },
        Some(path) if path.contains("StoreKit") => match symbol {
            "_OBJC_CLASS_$_SKRequest" => Some(SKREQUEST_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKProductsRequest" => Some(SKPRODUCTSREQUEST_CLASS.ptr() as usize),
            _ => None,
        },
        Some(path) if path.contains("AudioToolbox") => audiotoolbox_symbol(symbol),
        Some(path) if path.contains("CFNetwork") => cfnetwork_symbol(symbol),
        Some(path) if path.contains("Security") => match symbol {
            "_SecItemCopyMatching" | "_SecItemAdd" | "_SecItemUpdate" | "_SecItemDelete" => {
                Some(status_ok_stub as *const () as usize)
            }
            "_SecRandomCopyBytes" => Some(status_ok_stub as *const () as usize),
            _ => None,
        },
        Some(path) if path.contains("SystemConfiguration") => match symbol {
            "_SCNetworkReachabilityCreateWithName"
            | "_SCNetworkReachabilityCreateWithAddress" => Some(ptr_null_stub as *const () as usize),
            "_SCNetworkReachabilityGetFlags" => Some(false_stub as *const () as usize),
            _ => None,
        },
        Some(path) if path.contains("GLKit") => match symbol {
            "_OBJC_CLASS_$_GLKView" => Some(GLKVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_GLKViewController" => Some(GLKVIEWCONTROLLER_CLASS.ptr() as usize),
            _ => None,
        },
        Some(path) if path.contains("MetalKit") => match symbol {
            "_OBJC_CLASS_$_MTKView" => Some(MTKVIEW_CLASS.ptr() as usize),
            _ => None,
        },
        Some(path) if path.contains("OpenGLES") => match symbol {
            "_OBJC_CLASS_$_EAGLContext" => Some(EAGLCONTEXT_CLASS.ptr() as usize),
            _ => None,
        },
        None => pear_libobjc::symbol(symbol)
            .or_else(|| pear_foundation::symbol(symbol))
            .or_else(|| pear_graphics::symbol(symbol)),
        _ => None,
    }
}

/// Maps, fixes up, registers ObjC/Foundation metadata, calls `+load`, and runs
/// C/C++ initializers for an arm64 Mach-O image.
pub fn load_and_boot(
    data: &[u8],
    options: pear_dyld::Options,
) -> Result<(pear_dyld::LoadedImage, pear_libobjc::ImageReport), pear_dyld::Error> {
    init();
    let image = pear_dyld::load(data, &resolve_symbol, options)?;
    let report = unsafe { pear_libobjc::register_image(&image) };
    unsafe {
        pear_libobjc::call_load_methods(&image);
        image.run_initializers();
    }
    Ok((image, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_runtime_boots_both_objc_fixtures_and_foundation() {
        init();
        let f1 = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../pear-libobjc/fixtures/fixture-objc.dylib")).unwrap();
        let (img1, _) = load_and_boot(&f1, pear_dyld::Options::default()).unwrap();
        let run1: extern "C" fn() -> i32 = unsafe { std::mem::transmute(img1.symbol("_fixture_objc").unwrap()) };
        assert_eq!(run1(), 255);

        let f2 = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../pear-libobjc/fixtures/fixture2-objc.dylib")).unwrap();
        let (img2, _) = load_and_boot(&f2, pear_dyld::Options::default()).unwrap();
        let run2: extern "C" fn() -> i32 = unsafe { std::mem::transmute(img2.symbol("_fixture2_objc").unwrap()) };
        assert_eq!(run2(), 511);

        // Verify Foundation, CoreGraphics, QuartzCore & CFConstantString resolution
        assert!(resolve_symbol(Some("/System/Library/Frameworks/Foundation.framework/Foundation"), "_OBJC_CLASS_$_NSString").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"), "_CFStringCreateWithCString").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"), "___CFConstantStringClassReference").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics"), "_CGRectInset").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/QuartzCore.framework/QuartzCore"), "_CACurrentMediaTime").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/QuartzCore.framework/QuartzCore"), "_OBJC_CLASS_$_CAMetalLayer").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIApplication").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIScreen").is_some());

        unsafe {
            let app_cls = resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIApplication").unwrap() as Id;
            let sel_shared = objc_rt::sel("sharedApplication");
            let d1 = pear_libobjc::message::lookup_for_send(app_cls, sel_shared);
            let f1: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d1.imp);
            assert!(!f1(d1.receiver, sel_shared).is_null());

            let dev_cls = resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIDevice").unwrap() as Id;
            let sel_cur = objc_rt::sel("currentDevice");
            let d2 = pear_libobjc::message::lookup_for_send(dev_cls, sel_cur);
            let f2: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d2.imp);
            let dev = f2(d2.receiver, sel_cur);
            let sel_sys = objc_rt::sel("systemName");
            let d3 = pear_libobjc::message::lookup_for_send(dev, sel_sys);
            let f3: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d3.imp);
            let name = f3(d3.receiver, sel_sys);
            assert_eq!(pear_foundation::rust_str_from_nsstring(name).as_deref(), Some("iOS"));
        }
    }
}
