//! Clean-room: unified PearRuntime facade tying together `pear-macho`,
//! `pear-dyld`, `pear-libobjc`, `pear-foundation`, `pear-host` (+ OS backend),
//! and `pear-image` with zero external crates.io dependencies.

pub use pear_dyld;
pub use pear_foundation;
pub use pear_graphics;
pub use pear_host;
pub use pear_image;
pub use pear_jit;
pub use pear_legacy_host;
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
guest_class!(SKPAYMENT_CLASS, SKPAYMENT_META);
guest_class!(SKPAYMENTQUEUE_CLASS, SKPAYMENTQUEUE_META);
guest_class!(SKSTOREREVIEWCONTROLLER_CLASS, SKSTOREREVIEWCONTROLLER_META);
guest_class!(UIACCELEROMETER_CLASS, UIACCELEROMETER_META);
guest_class!(UIACTIVITYVIEWCONTROLLER_CLASS, UIACTIVITYVIEWCONTROLLER_META);
guest_class!(UIALERTVIEW_CLASS, UIALERTVIEW_META);
guest_class!(UIPASTEBOARD_CLASS, UIPASTEBOARD_META);
guest_class!(UIWINDOWSCENE_CLASS, UIWINDOWSCENE_META);
guest_class!(UITOUCH_CLASS, UITOUCH_META);
guest_class!(UIEVENT_CLASS, UIEVENT_META);
guest_class!(AVAUDIOSESSION_CLASS, AVAUDIOSESSION_META);
guest_class!(AVAUDIOPLAYER_CLASS, AVAUDIOPLAYER_META);
guest_class!(AVPLAYER_CLASS, AVPLAYER_META);
guest_class!(AVPLAYERITEM_CLASS, AVPLAYERITEM_META);
guest_class!(AVURLASSET_CLASS, AVURLASSET_META);
guest_class!(AVASSETREADER_CLASS, AVASSETREADER_META);
guest_class!(AVASSETREADERTRACKOUTPUT_CLASS, AVASSETREADERTRACKOUTPUT_META);
guest_class!(AVPLAYERITEMLEGIBLEOUTPUT_CLASS, AVPLAYERITEMLEGIBLEOUTPUT_META);
guest_class!(AVPLAYERITEMVIDEOOUTPUT_CLASS, AVPLAYERITEMVIDEOOUTPUT_META);
guest_class!(CKCONTAINER_CLASS, CKCONTAINER_META);
guest_class!(CKDATABASE_CLASS, CKDATABASE_META);
guest_class!(CKMODIFYRECORDSOPERATION_CLASS, CKMODIFYRECORDSOPERATION_META);
guest_class!(CKQUERY_CLASS, CKQUERY_META);
guest_class!(CKQUERYOPERATION_CLASS, CKQUERYOPERATION_META);
guest_class!(CKRECORD_CLASS, CKRECORD_META);
guest_class!(CKRECORDID_CLASS, CKRECORDID_META);
guest_class!(GCCONTROLLER_CLASS, GCCONTROLLER_META);

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

extern "C" fn ui_application_main(
    _argc: i32,
    _argv: *const *const c_char,
    _principal: Id,
    delegate: Id,
) -> i32 {
    let delegate_name = if !delegate.is_null() {
        pear_foundation::rust_str_from_nsstring(delegate)
    } else {
        None
    };
    pear_host::host().run(Box::new(move || {
        if let Some(name) = delegate_name {
            if let Some(cls) = objc_rt::class_named(&name) {
                unsafe {
                    let app = shared_app_stub(std::ptr::null_mut(), objc_rt::sel("sharedApplication"));
                    let del = alloc(cls);
                    let sel_init = objc_rt::sel("init");
                    let d_init = pear_libobjc::message::lookup_for_send(del, sel_init);
                    let f_init: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_init.imp);
                    let del = f_init(d_init.receiver, sel_init);
                    let sel_did = objc_rt::sel("application:didFinishLaunchingWithOptions:");
                    let d_did = pear_libobjc::message::lookup_for_send(del, sel_did);
                    let f_did: extern "C" fn(Id, Sel, Id, Id) -> bool = std::mem::transmute(d_did.imp);
                    let _ = f_did(d_did.receiver, sel_did, app, std::ptr::null_mut());
                }
            }
        }
    }));
    0
}

static HOST_WINDOWS: std::sync::Mutex<Option<std::collections::HashMap<usize, (usize, usize)>>> =
    std::sync::Mutex::new(None);
static ACTIVE_EGL_CTX: std::sync::Mutex<Option<usize>> = std::sync::Mutex::new(None);

extern "C" fn ui_window_init_with_frame(
    self_obj: Id,
    _cmd: Sel,
    frame: pear_graphics::CGRect,
) -> Id {
    let rect = pear_host::Rect {
        x: frame.origin.x,
        y: frame.origin.y,
        width: frame.size.width.max(64.0),
        height: frame.size.height.max(64.0),
    };
    let host = pear_host::host();
    let win = host.create_window(pear_host::GuestObject(self_obj as *mut c_void), rect);
    let layer = pear_host::LayerRef(self_obj as *mut c_void);
    host.set_root_layer(win, layer);
    let mut guard = HOST_WINDOWS.lock().unwrap();
    guard
        .get_or_insert_with(std::collections::HashMap::new)
        .insert(self_obj as usize, (win.0 as usize, layer.0 as usize));
    self_obj
}

extern "C" fn ui_window_make_key_and_visible(self_obj: Id, _cmd: Sel) {
    let pair = HOST_WINDOWS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).copied());
    let (win, _layer) = match pair {
        Some(p) => (pear_host::WindowHandle(p.0 as *mut c_void), p.1),
        None => {
            let _ = ui_window_init_with_frame(
                self_obj,
                _cmd,
                pear_graphics::CGRect {
                    origin: pear_graphics::CGPoint { x: 80.0, y: 60.0 },
                    size: pear_graphics::CGSize { width: 960.0, height: 600.0 },
                },
            );
            let p = HOST_WINDOWS
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|m| m.get(&(self_obj as usize)).copied())
                .unwrap_or((0, 0));
            (pear_host::WindowHandle(p.0 as *mut c_void), p.1)
        }
    };
    if !win.0.is_null() {
        pear_host::host().show_window(win, true);
    }
}

extern "C" fn eagl_init_with_api(self_obj: Id, _cmd: Sel, _api: usize) -> Id {
    self_obj
}

extern "C" fn eagl_set_current_context(_self_cls: Id, _cmd: Sel, ctx: Id) -> bool {
    *ACTIVE_EGL_CTX.lock().unwrap() = Some(ctx as usize);
    true
}

extern "C" fn eagl_renderbuffer_storage(
    _self_obj: Id,
    _cmd: Sel,
    _target: usize,
    drawable: Id,
) -> bool {
    if let Some(info) = pear_host::host().surface(pear_host::LayerRef(drawable as *mut c_void)) {
        if let pear_host::NativeSurface::Xlib { display, window } = info.surface {
            if let Some(egl) = egl_bridge::EglSurfaceCtx::create_from_x11(display, window) {
                let boxed = Box::into_raw(Box::new(egl)) as usize;
                *ACTIVE_EGL_CTX.lock().unwrap() = Some(boxed);
                return true;
            }
        }
    }
    true
}

extern "C" fn eagl_present_renderbuffer(_self_obj: Id, _cmd: Sel, _target: usize) -> bool {
    if let Some(ptr) = *ACTIVE_EGL_CTX.lock().unwrap() {
        if ptr > 0x10000 {
            let egl = unsafe { &*(ptr as *const egl_bridge::EglSurfaceCtx) };
            egl.swap_buffers();
        }
    }
    true
}

extern "C" fn ns_string_from_cgrect(r: pear_graphics::CGRect) -> Id {
    pear_foundation::nsstring_from_rust(&format!(
        "{{{{{}, {}}}, {{{}, {}}}}}",
        r.origin.x, r.origin.y, r.size.width, r.size.height
    ))
}

extern "C" fn ns_string_from_cgsize(s: pear_graphics::CGSize) -> Id {
    pear_foundation::nsstring_from_rust(&format!("{{{}, {}}}", s.width, s.height))
}

extern "C" fn ns_string_from_cgpoint(p: pear_graphics::CGPoint) -> Id {
    pear_foundation::nsstring_from_rust(&format!("{{{}, {}}}", p.x, p.y))
}

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
            (&SKPAYMENT_CLASS, &SKPAYMENT_META, "SKPayment"),
            (&SKPAYMENTQUEUE_CLASS, &SKPAYMENTQUEUE_META, "SKPaymentQueue"),
            (&SKSTOREREVIEWCONTROLLER_CLASS, &SKSTOREREVIEWCONTROLLER_META, "SKStoreReviewController"),
            (&UIACCELEROMETER_CLASS, &UIACCELEROMETER_META, "UIAccelerometer"),
            (&UIACTIVITYVIEWCONTROLLER_CLASS, &UIACTIVITYVIEWCONTROLLER_META, "UIActivityViewController"),
            (&UIALERTVIEW_CLASS, &UIALERTVIEW_META, "UIAlertView"),
            (&UIPASTEBOARD_CLASS, &UIPASTEBOARD_META, "UIPasteboard"),
            (&UIWINDOWSCENE_CLASS, &UIWINDOWSCENE_META, "UIWindowScene"),
            (&UITOUCH_CLASS, &UITOUCH_META, "UITouch"),
            (&UIEVENT_CLASS, &UIEVENT_META, "UIEvent"),
            (&AVAUDIOSESSION_CLASS, &AVAUDIOSESSION_META, "AVAudioSession"),
            (&AVAUDIOPLAYER_CLASS, &AVAUDIOPLAYER_META, "AVAudioPlayer"),
            (&AVPLAYER_CLASS, &AVPLAYER_META, "AVPlayer"),
            (&AVPLAYERITEM_CLASS, &AVPLAYERITEM_META, "AVPlayerItem"),
            (&AVURLASSET_CLASS, &AVURLASSET_META, "AVURLAsset"),
            (&AVASSETREADER_CLASS, &AVASSETREADER_META, "AVAssetReader"),
            (&AVASSETREADERTRACKOUTPUT_CLASS, &AVASSETREADERTRACKOUTPUT_META, "AVAssetReaderTrackOutput"),
            (&AVPLAYERITEMLEGIBLEOUTPUT_CLASS, &AVPLAYERITEMLEGIBLEOUTPUT_META, "AVPlayerItemLegibleOutput"),
            (&AVPLAYERITEMVIDEOOUTPUT_CLASS, &AVPLAYERITEMVIDEOOUTPUT_META, "AVPlayerItemVideoOutput"),
            (&CKCONTAINER_CLASS, &CKCONTAINER_META, "CKContainer"),
            (&CKDATABASE_CLASS, &CKDATABASE_META, "CKDatabase"),
            (&CKMODIFYRECORDSOPERATION_CLASS, &CKMODIFYRECORDSOPERATION_META, "CKModifyRecordsOperation"),
            (&CKQUERY_CLASS, &CKQUERY_META, "CKQuery"),
            (&CKQUERYOPERATION_CLASS, &CKQUERYOPERATION_META, "CKQueryOperation"),
            (&CKRECORD_CLASS, &CKRECORD_META, "CKRecord"),
            (&CKRECORDID_CLASS, &CKRECORDID_META, "CKRecordID"),
            (&GCCONTROLLER_CLASS, &GCCONTROLLER_META, "GCController"),
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
        objc_rt::register_builtin(
            UIWINDOW_CLASS.ptr(),
            UIWINDOW_META.ptr(),
            "UIWindow",
            64,
            &[
                ("initWithFrame:", i(ui_window_init_with_frame as *const ())),
                ("makeKeyAndVisible", i(ui_window_make_key_and_visible as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            EAGLCONTEXT_CLASS.ptr(),
            EAGLCONTEXT_META.ptr(),
            "EAGLContext",
            64,
            &[
                ("initWithAPI:", i(eagl_init_with_api as *const ())),
                ("renderbufferStorage:fromDrawable:", i(eagl_renderbuffer_storage as *const ())),
                ("presentRenderbuffer:", i(eagl_present_renderbuffer as *const ())),
            ],
            &[("setCurrentContext:", i(eagl_set_current_context as *const ()))],
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
    fn dlopen(filename: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

fn gles_dlsym(macho_sym: &str) -> usize {
    static HANDLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let h = *HANDLE.get_or_init(|| unsafe {
        let mut p = std::ptr::null_mut();
        if let Ok(custom) = std::env::var("PEAR_GLES_LIB") {
            if let Ok(c) = std::ffi::CString::new(custom) {
                p = dlopen(c.as_ptr(), 1);
            }
        }
        if p.is_null() {
            #[cfg(target_os = "macos")]
            let candidates = [
                c"libGLESv2.dylib".as_ptr(),
                c"build/host/PearANGLE/libGLESv2.dylib".as_ptr(),
            ];
            #[cfg(not(target_os = "macos"))]
            let candidates = [c"libGLESv2.so.2".as_ptr(), c"libGLESv2.so".as_ptr()];
            for &cand in &candidates {
                p = dlopen(cand, 1);
                if !p.is_null() {
                    break;
                }
            }
        }
        p as usize
    }) as *mut c_void;
    if !h.is_null() {
        if let Some(cname) = macho_sym.strip_prefix('_') {
            if let Ok(cstr) = std::ffi::CString::new(cname) {
                let sym = unsafe { dlsym(h, cstr.as_ptr()) };
                if !sym.is_null() {
                    return sym as usize;
                }
            }
        }
    }
    status_ok_stub as *const () as usize
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

#[repr(C)]
struct SwiftOsVersion {
    major: isize,
    minor: isize,
    patch: isize,
}

extern "C" fn swift_stdlib_os_version() -> SwiftOsVersion {
    SwiftOsVersion { major: 17, minor: 0, patch: 0 }
}

extern "C" fn swift_once_impl(pred: *mut usize, f: extern "C" fn(*mut c_void), ctx: *mut c_void) {
    dispatch_once_f_impl(pred, ctx, f);
}

extern "C" fn swift_retain_impl(obj: Id) -> Id {
    let addr = obj as usize;
    if addr != 0 && (addr & 0x8000_0000_0000_0001) == 0 {
        pear_libobjc::object::retain(obj)
    } else {
        obj
    }
}

extern "C" fn swift_release_impl(obj: Id) {
    let addr = obj as usize;
    if addr != 0 && (addr & 0x8000_0000_0000_0001) == 0 {
        pear_libobjc::object::release(obj);
    }
}

extern "C" fn swift_identity_ptr(p: *const c_void) -> *const c_void { p }

extern "C" fn swift_alloc_object_impl(meta: *const ClassT, size: usize, _align_mask: usize) -> Id {
    if meta.is_null() {
        return std::ptr::null_mut();
    }
    let sz = size.max(16);
    unsafe {
        let p = malloc(sz) as Id;
        if !p.is_null() {
            std::ptr::write_bytes(p as *mut u8, 0, sz);
            *(p as *mut *const ClassT) = meta;
        }
        p
    }
}

extern "C" fn swift_dealloc_object_impl(obj: Id, _sz: usize, _align_mask: usize) {
    if !obj.is_null() {
        unsafe { free(obj as *mut c_void) };
    }
}

#[repr(C)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

static K_CM_TIME_ZERO: CMTime = CMTime { value: 0, timescale: 1, flags: 1, epoch: 0 };

extern "C" fn cm_time_make(value: i64, timescale: i32) -> CMTime {
    CMTime { value, timescale, flags: 1, epoch: 0 }
}

extern "C" fn cm_time_get_seconds(t: CMTime) -> f64 {
    if t.timescale == 0 { 0.0 } else { t.value as f64 / t.timescale as f64 }
}

fn swift_core_symbol(symbol: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match symbol {
        "_swift_retain"
        | "_swift_bridgeObjectRetain"
        | "_swift_unknownObjectRetain"
        | "_swift_errorRetain" => f(swift_retain_impl as *const ()),
        "_swift_release"
        | "_swift_bridgeObjectRelease"
        | "_swift_unknownObjectRelease"
        | "_swift_errorRelease" => f(swift_release_impl as *const ()),
        "_swift_once" => f(swift_once_impl as *const ()),
        "_swift_allocObject" => f(swift_alloc_object_impl as *const ()),
        "_swift_deallocObject" | "_swift_deallocClassInstance" => f(swift_dealloc_object_impl as *const ()),
        "_swift_getObjCClassMetadata"
        | "_swift_getObjCClassFromMetadata"
        | "_swift_getInitializedObjCClass" => f(swift_identity_ptr as *const ()),
        "__swift_stdlib_operatingSystemVersion" => f(swift_stdlib_os_version as *const ()),
        _ => None,
    }
}

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
        | "_ExtAudioFileSeek"
        | "_NewAUGraph"
        | "_DisposeAUGraph"
        | "_AUGraphAddNode"
        | "_AUGraphConnectNodeInput"
        | "_AUGraphInitialize"
        | "_AUGraphNodeInfo"
        | "_AUGraphOpen"
        | "_AUGraphSetNodeInputCallback"
        | "_AUGraphStart"
        | "_AUGraphStop"
        | "_AudioServicesPlaySystemSound"
        | "_AudioUnitSetParameter"
        | "_CAShow" => Some(ok),
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

struct DefaultGuestEvents;

impl pear_host::GuestEvents for DefaultGuestEvents {
    fn pointer(&self, _: pear_host::GuestObject, _: pear_host::TouchPhase, _: f64, _: f64, _: f64) {}
    fn key(&self, _: pear_host::KeyEvent) {}
    fn text(&self, _: &str) {}
    fn edit_command(&self, _: pear_host::EditCommand) {}
    fn text_input_active(&self) -> bool { false }
    fn window_resized(&self, _: pear_host::GuestObject, _: pear_host::Size, _: f64) {}
    fn window_became_key(&self, _: pear_host::GuestObject) {}
    fn lifecycle(&self, _: pear_host::Lifecycle) {}
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
        let _ = pear_host::install_events(Box::new(DefaultGuestEvents));
        pear_host_linux::LinuxHost::install().initialize();
    }
}

/// Unified symbol resolver for `pear-dyld`: resolves `libobjc`, `libSystem`
/// (libclosure + libc basics), `CoreFoundation`, and `Foundation` imports.
pub fn resolve_symbol(library: Option<&str>, symbol: &str) -> Option<usize> {
    init();
    match library {
        Some("/usr/lib/libobjc.A.dylib") => pear_libobjc::symbol(symbol),
        Some("/usr/lib/libc++.1.dylib") | Some("/usr/lib/libc++abi.dylib") => {
            libc_dlsym(symbol)
        }
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
        Some(path) if path.contains("AVFAudio") || path.contains("AVFoundation") => match symbol {
            "_OBJC_CLASS_$_AVAudioSession" => Some(AVAUDIOSESSION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVAudioPlayer" => Some(AVAUDIOPLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayer" => Some(AVPLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayerItem" => Some(AVPLAYERITEM_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVURLAsset" => Some(AVURLASSET_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVAssetReader" => Some(AVASSETREADER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVAssetReaderTrackOutput" => Some(AVASSETREADERTRACKOUTPUT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayerItemLegibleOutput" => Some(AVPLAYERITEMLEGIBLEOUTPUT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayerItemVideoOutput" => Some(AVPLAYERITEMVIDEOOUTPUT_CLASS.ptr() as usize),
            s if s.starts_with("_AV") => Some(framework_const(Box::leak(s.to_string().into_boxed_str()))),
            _ => None,
        },
        Some(path) if path.contains("CloudKit") => match symbol {
            "_OBJC_CLASS_$_CKContainer" => Some(CKCONTAINER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKDatabase" => Some(CKDATABASE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKModifyRecordsOperation" => Some(CKMODIFYRECORDSOPERATION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKQuery" => Some(CKQUERY_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKQueryOperation" => Some(CKQUERYOPERATION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKRecord" => Some(CKRECORD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKRecordID" => Some(CKRECORDID_CLASS.ptr() as usize),
            "_CKQueryOperationMaximumResults" => Some(framework_const("CKQueryOperationMaximumResults")),
            _ => None,
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
            "_OBJC_CLASS_$_UIAccelerometer" => Some(UIACCELEROMETER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIActivityViewController" => Some(UIACTIVITYVIEWCONTROLLER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIAlertView" => Some(UIALERTVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIPasteboard" => Some(UIPASTEBOARD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIWindowScene" => Some(UIWINDOWSCENE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITouch" => Some(UITOUCH_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIEvent" => Some(UIEVENT_CLASS.ptr() as usize),
            "_NSStringFromCGRect" => Some(ns_string_from_cgrect as *const () as usize),
            "_NSStringFromCGSize" => Some(ns_string_from_cgsize as *const () as usize),
            "_NSStringFromCGPoint" => Some(ns_string_from_cgpoint as *const () as usize),
            "_OBJC_METACLASS_$_UIDocument" => Some(UIDOCUMENT_META.ptr() as usize),
            "_OBJC_METACLASS_$_UIView" => Some(UIVIEW_META.ptr() as usize),
            "_OBJC_METACLASS_$_UIViewController" => Some(UIVIEWCONTROLLER_META.ptr() as usize),
            "_UIApplicationMain" => Some(ui_application_main as *const () as usize),
            s if s.starts_with("_UI") => Some(framework_const(Box::leak(s.to_string().into_boxed_str()))),
            _ => None,
        },
        Some(path) if path.contains("StoreKit") => match symbol {
            "_OBJC_CLASS_$_SKRequest" => Some(SKREQUEST_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKProductsRequest" => Some(SKPRODUCTSREQUEST_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKPayment" => Some(SKPAYMENT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKPaymentQueue" => Some(SKPAYMENTQUEUE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKStoreReviewController" => Some(SKSTOREREVIEWCONTROLLER_CLASS.ptr() as usize),
            "_SKErrorDomain" => Some(framework_const("SKErrorDomain")),
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
        Some(path) if path.contains("libswiftCore") => swift_core_symbol(symbol).or_else(|| libc_dlsym(symbol)),
        Some(path) if path.contains("libz") => libc_dlsym(symbol),
        Some(path) if path.contains("Metal") && !path.contains("MetalKit") => match symbol {
            "_MTLCreateSystemDefaultDevice" | "_MTLCopyAllDevices" => Some(ptr_null_stub as *const () as usize),
            _ => None,
        },
        Some(path) if path.contains("GameController") => match symbol {
            "_OBJC_CLASS_$_GCController" => Some(GCCONTROLLER_CLASS.ptr() as usize),
            "_GCControllerDidConnectNotification" => Some(framework_const("GCControllerDidConnectNotification")),
            "_GCControllerDidDisconnectNotification" => Some(framework_const("GCControllerDidDisconnectNotification")),
            _ => None,
        },
        Some(path) if path.contains("CoreMedia") => match symbol {
            "_CMTimeMake" => Some(cm_time_make as *const () as usize),
            "_CMTimeGetSeconds" => Some(cm_time_get_seconds as *const () as usize),
            "_kCMTimeZero" => Some(&raw const K_CM_TIME_ZERO as usize),
            _ => None,
        },
        Some(path) if path.contains("CoreVideo") => match symbol {
            "_CVPixelBufferLockBaseAddress"
            | "_CVPixelBufferUnlockBaseAddress"
            | "_CVPixelBufferGetWidth"
            | "_CVPixelBufferGetHeight" => Some(status_ok_stub as *const () as usize),
            "_CVPixelBufferGetBaseAddress" => Some(ptr_null_stub as *const () as usize),
            _ => None,
        },
        Some(path) if path.contains("OpenGLES") => match symbol {
            "_OBJC_CLASS_$_EAGLContext" => Some(EAGLCONTEXT_CLASS.ptr() as usize),
            "_kEAGLColorFormatRGBA8" => Some(framework_const("kEAGLColorFormatRGBA8")),
            "_kEAGLDrawablePropertyColorFormat" => Some(framework_const("kEAGLDrawablePropertyColorFormat")),
            "_kEAGLDrawablePropertyRetainedBacking" => Some(framework_const("kEAGLDrawablePropertyRetainedBacking")),
            s if s.starts_with("_gl") => Some(gles_dlsym(s)),
            _ => None,
        },
        None => pear_libobjc::symbol(symbol)
            .or_else(|| pear_foundation::symbol(symbol))
            .or_else(|| pear_graphics::symbol(symbol))
            .or_else(|| libc_dlsym(symbol)),
        _ => None,
    }
}

/// Portable EGL + OpenGL ES 2.0 surface bridge (`libEGL.so.1` + `libGLESv2.so.2`)
/// for Linux/X11 windows (including Mesa `llvmpipe` inside headless/VNC containers).
pub mod egl_bridge {
    use std::ffi::{c_char, c_float, c_int, c_uint, c_void};

    const EGL_OPENGL_ES_API: c_uint = 0x30A0;
    const EGL_SURFACE_TYPE: c_int = 0x3033;
    const EGL_WINDOW_BIT: c_int = 0x0004;
    const EGL_RENDERABLE_TYPE: c_int = 0x3040;
    const EGL_OPENGL_ES2_BIT: c_int = 0x0004;
    const EGL_RED_SIZE: c_int = 0x3024;
    const EGL_GREEN_SIZE: c_int = 0x3023;
    const EGL_BLUE_SIZE: c_int = 0x3022;
    const EGL_ALPHA_SIZE: c_int = 0x3021;
    const EGL_CONTEXT_CLIENT_VERSION: c_int = 0x3098;
    const EGL_NONE: c_int = 0x3038;

    const GL_COLOR_BUFFER_BIT: c_uint = 0x0000_4000;
    const GL_TRIANGLES: c_uint = 0x0004;
    const GL_FLOAT: c_uint = 0x1406;
    const GL_FRAGMENT_SHADER: c_uint = 0x8B30;
    const GL_VERTEX_SHADER: c_uint = 0x8B31;

    pub struct EglSurfaceCtx {
        dpy: *mut c_void,
        surf: *mut c_void,
        ctx: *mut c_void,
        program: c_uint,
        u_angle: c_int,
        egl_swap_buffers: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint,
        gl_viewport: unsafe extern "C" fn(c_int, c_int, c_int, c_int),
        gl_clear_color: unsafe extern "C" fn(c_float, c_float, c_float, c_float),
        gl_clear: unsafe extern "C" fn(c_uint),
        gl_use_program: unsafe extern "C" fn(c_uint),
        gl_uniform1f: unsafe extern "C" fn(c_int, c_float),
        gl_vertex_attrib_pointer: unsafe extern "C" fn(c_uint, c_int, c_uint, u8, c_int, *const c_void),
        gl_enable_vertex_attrib_array: unsafe extern "C" fn(c_uint),
        gl_draw_arrays: unsafe extern "C" fn(c_uint, c_int, c_int),
    }

    unsafe impl Send for EglSurfaceCtx {}
    unsafe impl Sync for EglSurfaceCtx {}

    impl EglSurfaceCtx {
        pub fn create_from_x11(x11_display: *mut c_void, x11_window: u64) -> Option<Self> {
            unsafe {
                let mut egl_lib = super::dlopen(c"libEGL.so.1".as_ptr(), 1);
                if egl_lib.is_null() {
                    egl_lib = super::dlopen(c"libEGL.so".as_ptr(), 1);
                }
                let mut gles_lib = super::dlopen(c"libGLESv2.so.2".as_ptr(), 1);
                if gles_lib.is_null() {
                    gles_lib = super::dlopen(c"libGLESv2.so".as_ptr(), 1);
                }
                if egl_lib.is_null() || gles_lib.is_null() {
                    eprintln!("[pear-egl] failed to dlopen libEGL / libGLESv2");
                    return None;
                }

                let get_proc: unsafe extern "C" fn(*const c_char) -> *mut c_void =
                    std::mem::transmute(super::dlsym(egl_lib, c"eglGetProcAddress".as_ptr()));
                let load_egl = |name: &std::ffi::CStr| -> *mut c_void {
                    let p = super::dlsym(egl_lib, name.as_ptr());
                    if !p.is_null() { p } else { get_proc(name.as_ptr()) }
                };
                let load_gl = |name: &std::ffi::CStr| -> *mut c_void {
                    let p = super::dlsym(gles_lib, name.as_ptr());
                    if !p.is_null() { p } else { get_proc(name.as_ptr()) }
                };

                let egl_get_display: unsafe extern "C" fn(*mut c_void) -> *mut c_void =
                    std::mem::transmute(load_egl(c"eglGetDisplay"));
                let egl_initialize: unsafe extern "C" fn(*mut c_void, *mut c_int, *mut c_int) -> c_uint =
                    std::mem::transmute(load_egl(c"eglInitialize"));
                let egl_bind_api: unsafe extern "C" fn(c_uint) -> c_uint =
                    std::mem::transmute(load_egl(c"eglBindAPI"));
                let egl_choose_config: unsafe extern "C" fn(*mut c_void, *const c_int, *mut *mut c_void, c_int, *mut c_int) -> c_uint =
                    std::mem::transmute(load_egl(c"eglChooseConfig"));
                let egl_create_window_surface: unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *const c_int) -> *mut c_void =
                    std::mem::transmute(load_egl(c"eglCreateWindowSurface"));
                let egl_create_context: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const c_int) -> *mut c_void =
                    std::mem::transmute(load_egl(c"eglCreateContext"));
                let egl_make_current: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> c_uint =
                    std::mem::transmute(load_egl(c"eglMakeCurrent"));
                let egl_swap_buffers: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint =
                    std::mem::transmute(load_egl(c"eglSwapBuffers"));
                let egl_get_error: unsafe extern "C" fn() -> c_int =
                    std::mem::transmute(load_egl(c"eglGetError"));

                let dpy = egl_get_display(x11_display);
                if dpy.is_null() {
                    eprintln!("[pear-egl] eglGetDisplay returned null");
                    return None;
                }
                let mut maj = 0;
                let mut min = 0;
                if egl_initialize(dpy, &mut maj, &mut min) == 0 {
                    eprintln!("[pear-egl] eglInitialize failed err=0x{:x}", egl_get_error());
                    return None;
                }
                let _ = egl_bind_api(EGL_OPENGL_ES_API);

                let cfg_attrs = [
                    EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
                    EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
                    EGL_RED_SIZE, 8,
                    EGL_GREEN_SIZE, 8,
                    EGL_BLUE_SIZE, 8,
                    EGL_ALPHA_SIZE, 8,
                    EGL_NONE,
                ];
                let mut cfg: *mut c_void = std::ptr::null_mut();
                let mut num_cfg = 0;
                if egl_choose_config(dpy, cfg_attrs.as_ptr(), &mut cfg, 1, &mut num_cfg) == 0 || num_cfg == 0 {
                    eprintln!("[pear-egl] eglChooseConfig failed err=0x{:x}", egl_get_error());
                    return None;
                }

                let surf = egl_create_window_surface(dpy, cfg, x11_window as usize, std::ptr::null());
                if surf.is_null() {
                    eprintln!("[pear-egl] eglCreateWindowSurface failed err=0x{:x}", egl_get_error());
                    return None;
                }

                let ctx_attrs = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
                let ctx = egl_create_context(dpy, cfg, std::ptr::null_mut(), ctx_attrs.as_ptr());
                if ctx.is_null() {
                    eprintln!("[pear-egl] eglCreateContext failed err=0x{:x}", egl_get_error());
                    return None;
                }

                if egl_make_current(dpy, surf, surf, ctx) == 0 {
                    eprintln!("[pear-egl] eglMakeCurrent failed err=0x{:x}", egl_get_error());
                    return None;
                }

                let gl_create_shader: unsafe extern "C" fn(c_uint) -> c_uint = std::mem::transmute(load_gl(c"glCreateShader"));
                let gl_shader_source: unsafe extern "C" fn(c_uint, c_int, *const *const c_char, *const c_int) = std::mem::transmute(load_gl(c"glShaderSource"));
                let gl_compile_shader: unsafe extern "C" fn(c_uint) = std::mem::transmute(load_gl(c"glCompileShader"));
                let gl_create_program: unsafe extern "C" fn() -> c_uint = std::mem::transmute(load_gl(c"glCreateProgram"));
                let gl_attach_shader: unsafe extern "C" fn(c_uint, c_uint) = std::mem::transmute(load_gl(c"glAttachShader"));
                let gl_bind_attrib_location: unsafe extern "C" fn(c_uint, c_uint, *const c_char) = std::mem::transmute(load_gl(c"glBindAttribLocation"));
                let gl_link_program: unsafe extern "C" fn(c_uint) = std::mem::transmute(load_gl(c"glLinkProgram"));
                let gl_get_uniform_location: unsafe extern "C" fn(c_uint, *const c_char) -> c_int = std::mem::transmute(load_gl(c"glGetUniformLocation"));

                let vs_src = c"attribute vec2 aPos; attribute vec3 aCol; varying vec3 vCol; uniform float uAngle; void main() { float c = cos(uAngle); float s = sin(uAngle); gl_Position = vec4(aPos.x * c - aPos.y * s, aPos.x * s + aPos.y * c, 0.0, 1.0); vCol = aCol; }";
                let fs_src = c"precision mediump float; varying vec3 vCol; void main() { gl_FragColor = vec4(vCol, 1.0); }";

                let vs = gl_create_shader(GL_VERTEX_SHADER);
                let vs_ptr = vs_src.as_ptr();
                gl_shader_source(vs, 1, &vs_ptr, std::ptr::null());
                gl_compile_shader(vs);

                let fs = gl_create_shader(GL_FRAGMENT_SHADER);
                let fs_ptr = fs_src.as_ptr();
                gl_shader_source(fs, 1, &fs_ptr, std::ptr::null());
                gl_compile_shader(fs);

                let program = gl_create_program();
                gl_attach_shader(program, vs);
                gl_attach_shader(program, fs);
                gl_bind_attrib_location(program, 0, c"aPos".as_ptr());
                gl_bind_attrib_location(program, 1, c"aCol".as_ptr());
                gl_link_program(program);
                let u_angle = gl_get_uniform_location(program, c"uAngle".as_ptr());

                eprintln!("[pear-egl] EGL {maj}.{min} + GLESv2 initialized (surf={surf:?} ctx={ctx:?})");

                Some(Self {
                    dpy,
                    surf,
                    ctx,
                    program,
                    u_angle,
                    egl_swap_buffers,
                    gl_viewport: std::mem::transmute(load_gl(c"glViewport")),
                    gl_clear_color: std::mem::transmute(load_gl(c"glClearColor")),
                    gl_clear: std::mem::transmute(load_gl(c"glClear")),
                    gl_use_program: std::mem::transmute(load_gl(c"glUseProgram")),
                    gl_uniform1f: std::mem::transmute(load_gl(c"glUniform1f")),
                    gl_vertex_attrib_pointer: std::mem::transmute(load_gl(c"glVertexAttribPointer")),
                    gl_enable_vertex_attrib_array: std::mem::transmute(load_gl(c"glEnableVertexAttribArray")),
                    gl_draw_arrays: std::mem::transmute(load_gl(c"glDrawArrays")),
                })
            }
        }

        pub fn swap_buffers(&self) {
            unsafe {
                let _ = (self.egl_swap_buffers)(self.dpy, self.surf);
            }
        }

        pub fn render_demo_frame(&self, width: i32, height: i32, angle: f32) {
            let _ = self.ctx;
            #[rustfmt::skip]
            let verts: [f32; 15] = [
                 0.0,   0.65,  0.30, 0.85, 0.95,
                -0.60, -0.45,  0.95, 0.35, 0.55,
                 0.60, -0.45,  0.35, 0.95, 0.65,
            ];
            unsafe {
                (self.gl_viewport)(0, 0, width, height);
                let bg = 0.08 + 0.04 * angle.sin();
                (self.gl_clear_color)(bg, 0.11, 0.17, 1.0);
                (self.gl_clear)(GL_COLOR_BUFFER_BIT);
                (self.gl_use_program)(self.program);
                if self.u_angle >= 0 {
                    (self.gl_uniform1f)(self.u_angle, angle);
                }
                let stride = (5 * std::mem::size_of::<f32>()) as c_int;
                (self.gl_vertex_attrib_pointer)(0, 2, GL_FLOAT, 0, stride, verts.as_ptr() as *const c_void);
                (self.gl_enable_vertex_attrib_array)(0);
                (self.gl_vertex_attrib_pointer)(1, 3, GL_FLOAT, 0, stride, verts.as_ptr().add(2) as *const c_void);
                (self.gl_enable_vertex_attrib_array)(1);
                (self.gl_draw_arrays)(GL_TRIANGLES, 0, 3);
                let _ = (self.egl_swap_buffers)(self.dpy, self.surf);
            }
        }
    }
}

/// 32-bit iOS API & `PearLegacyEngine` + `PearLegacyJIT` integration layer.
pub mod api32 {
    use pear_jit::{ExecutionMode, ExecutionPlan};

    /// Initializes the OS-independent 32-bit host bridge (`pear-legacy-host`)
    /// and prepares `PearLegacyJIT` (A32) for 32-bit iOS execution.
    pub fn init(plan: &ExecutionPlan) {
        debug_assert_eq!(plan.mode, ExecutionMode::Legacy32BitJit);
        pear_legacy_host::init_legacy_host();
    }
}

/// 64-bit iOS API layer (`pear-dyld` + `pear-libobjc` + `pear-foundation` + `pear-graphics` + `pear-host`),
/// executing natively on ARM64 hosts or via `PearLegacyJIT` (A64 -> x86_64) on x86_64 hosts.
pub mod api64 {
    use pear_jit::{ExecutionMode, ExecutionPlan};

    pub fn init(plan: &ExecutionPlan) {
        match plan.mode {
            ExecutionMode::NativeDirect | ExecutionMode::Arm64ToX86_64Jit => super::init(),
            ExecutionMode::Legacy32BitJit => super::api32::init(plan),
        }
    }
}

/// Inspects a Mach-O or FAT binary and selects the optimal OS/arch/API/JIT plan.
pub fn plan_execution(data: &[u8]) -> Option<pear_jit::ExecutionPlan> {
    let arch = pear_macho::detect_guest_arch(data)?;
    Some(pear_jit::ExecutionPlan::current_host(arch))
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
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_swift_once").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_swift_bridgeObjectRetain").is_some());

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

        let app_bin = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-ios-app")).unwrap();
        let (app_img, _) = load_and_boot(&app_bin, pear_dyld::Options::default()).unwrap();
        let app_main: extern "C" fn(i32, *const *const c_char) -> i32 =
            unsafe { std::mem::transmute(app_img.symbol("_main").unwrap()) };
        assert_eq!(app_main(0, std::ptr::null()), 63);
    }
}
