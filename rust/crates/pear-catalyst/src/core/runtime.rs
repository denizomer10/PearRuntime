//! The runtime core: logging, the bootstrap that wires the modules together,
//! window activation and opt-in diagnostics.
//!
//! The keymapper (keyboard/mouse → synthesized touches) lives in
//! `input::keymapper`, GameController discovery in `input::controller`. The
//! runtime adds no UI to the game: the keymapper UI is PearSilicon.app's own
//! overlay window.

use crate::core::{host, registry};
use crate::rt;
use pear_objc::{cls, msg0, msg1, nsstring, CGRect, Id, Imp, Sel};
use std::ffi::{c_char, c_void, CStr, CString, VaList};
use std::io::Write;
use std::sync::OnceLock;

const NIL: Id = std::ptr::null_mut();

// ---------------------------------------------------------------------------
// Logging

/// The log file used when the launcher did not pass PEAR_LOG_FILE: GUI
/// launches need the same trace as `--log` runs.
fn log_file_path() -> Option<&'static str> {
    static PATH: OnceLock<Option<String>> = OnceLock::new();
    PATH.get_or_init(|| unsafe {
        let bundle_id: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
        let bundle_id = if bundle_id.is_null() { String::new() } else { rt::ns_string(bundle_id) };
        if bundle_id.is_empty() {
            return None;
        }
        let directory = match std::env::var("PEAR_SUPPORT_ROOT").ok().filter(|r| !r.is_empty()) {
            Some(root) => rt::path_join(&root, "Logs"),
            None => {
                extern "C" {
                    fn getuid() -> u32;
                    fn getpwuid(uid: u32) -> *const Passwd;
                }
                #[repr(C)]
                struct Passwd {
                    name: *const c_char,
                    passwd: *const c_char,
                    uid: u32,
                    gid: u32,
                    change: isize,
                    class: *const c_char,
                    gecos: *const c_char,
                    dir: *const c_char,
                }
                let pw = getpwuid(getuid());
                if pw.is_null() || (*pw).dir.is_null() {
                    return None;
                }
                format!("{}/Library/Application Support/PearSilicon/Logs", CStr::from_ptr((*pw).dir).to_string_lossy())
            }
        };
        let _: bool = pear_objc::msg4(rt::file_manager(), "createDirectoryAtPath:withIntermediateDirectories:attributes:error:", nsstring(&directory), true, NIL, NIL);
        Some(rt::path_join(&directory, &format!("{bundle_id}.log")))
    })
    .as_deref()
}

#[export_name = "_Z15PearLogFilePathv"]
pub unsafe extern "C" fn log_file_path_ns() -> Id {
    match log_file_path() {
        Some(p) => nsstring(p),
        None => NIL,
    }
}

extern "C" {
    fn os_log_create(subsystem: *const c_char, category: *const c_char) -> *mut c_void;
    fn _os_log_impl(dso: *const c_void, log: *mut c_void, kind: u8, format: *const c_char, buf: *mut u8, size: u32);
    fn os_log_type_enabled(log: *mut c_void, kind: u8) -> bool;
    static __dso_handle: c_void;
}

/// `os_log_with_type(handle, DEFAULT, "%{public}s", message)`: the unified log
/// redacts dynamic arguments unless they are marked public.
unsafe fn os_log_public(message: &CStr) {
    static HANDLE: OnceLock<usize> = OnceLock::new();
    let handle = *HANDLE.get_or_init(|| unsafe { os_log_create(c"com.pearsilicon.runtime".as_ptr(), c"general".as_ptr()) as usize }) as *mut c_void;
    if handle.is_null() || !os_log_type_enabled(handle, 0) {
        return;
    }
    // The compiler's os_log buffer: summary (has non-scalar), one argument,
    // then a public string item (type 2, public flag) holding the pointer.
    let mut buf = [0u8; 12];
    buf[0] = 2;
    buf[1] = 1;
    buf[2] = 0x22;
    buf[3] = 8;
    buf[4..12].copy_from_slice(&(message.as_ptr() as usize).to_ne_bytes());
    static FORMAT: &CStr = c"%{public}s";
    _os_log_impl(&__dso_handle, handle, 0, FORMAT.as_ptr(), buf.as_mut_ptr(), buf.len() as u32);
}

/// One log line: the unified log (public) and the PEAR_LOG_FILE mirror.
pub fn log_line(message: &str) {
    let c = CString::new(message.replace('\0', "")).unwrap_or_default();
    unsafe { os_log_public(&c) };
    let path = std::env::var_os("PEAR_LOG_FILE").map(std::path::PathBuf::from).or_else(|| log_file_path().map(std::path::PathBuf::from));
    if let Some(path) = path {
        if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
            let mut line = c.into_bytes();
            line.push(b'\n');
            let _ = file.write_all(&line);
        }
    }
}

/// `PearLog(format, ...)`: NSString formatting for the remaining ObjC++ code.
#[export_name = "_Z7PearLogP8NSStringz"]
pub unsafe extern "C" fn pear_log(format: Id, mut args: ...) {
    pear_objc::autorelease_pool(|| {
        let init: unsafe extern "C" fn(Id, Sel, Id, VaList) -> Id = std::mem::transmute(pear_objc::objc_msgSend as *const ());
        let message = init(msg0(cls("NSString"), "alloc"), pear_objc::sel("initWithFormat:arguments:"), format, args);
        let text = if message.is_null() { "(null)".to_string() } else { rt::ns_string(message) };
        if !message.is_null() {
            pear_objc::objc_release(message);
        }
        log_line(&text);
    });
}

// ---------------------------------------------------------------------------
// Window activation

extern "C" {
    fn NSStringFromCGRect(r: CGRect) -> Id;
}

unsafe fn rect_string(r: CGRect) -> String {
    rt::ns_string(NSStringFromCGRect(r))
}

unsafe fn ns_app() -> Id {
    let c = rt::class_named("NSApplication");
    if !c.is_null() && pear_objc::responds_to(c, "sharedApplication") { msg0(c, "sharedApplication") } else { NIL }
}

pub unsafe fn ui_app() -> Id {
    msg0(cls("UIApplication"), "sharedApplication")
}

unsafe fn class_of(obj: Id) -> String {
    if obj.is_null() { "nil".into() } else { pear_objc::class_name(obj).to_string() }
}

/// Every window of every connected UIWindowScene.
pub unsafe fn scene_windows() -> Vec<Id> {
    let mut out = Vec::new();
    let scenes: Id = msg0(msg0::<Id>(ui_app(), "connectedScenes"), "allObjects");
    for i in 0..pear_objc::nsarray_count(scenes) {
        let scene = pear_objc::nsarray_object_at(scenes, i);
        if !pear_objc::is_kind_of(scene, "UIWindowScene") {
            continue;
        }
        let windows: Id = msg0(scene, "windows");
        for j in 0..pear_objc::nsarray_count(windows) {
            out.push(pear_objc::nsarray_object_at(windows, j));
        }
    }
    out
}

/// Directly launched Catalyst processes are not always given a key window:
/// bring the game window forward (never activating over another app).
unsafe fn activate_main_window_once() {
    if msg0::<isize>(ui_app(), "applicationState") != 0 {
        return;
    }
    let nsapp = ns_app();
    if nsapp.is_null() || !pear_objc::responds_to(nsapp, "isActive") || !msg0::<bool>(nsapp, "isActive") {
        return;
    }
    let mut best: Id = NIL;
    let mut best_area = 0.0;
    let windows: Id = msg0(nsapp, "windows");
    for i in 0..pear_objc::nsarray_count(windows) {
        let w = pear_objc::nsarray_object_at(windows, i);
        if pear_objc::responds_to(w, "isVisible") && !msg0::<bool>(w, "isVisible") {
            continue;
        }
        let frame: CGRect = if pear_objc::responds_to(w, "frame") { msg0(w, "frame") } else { CGRect::default() };
        let area = frame.size.width * frame.size.height;
        if area > best_area {
            best_area = area;
            best = w;
        }
    }
    if !best.is_null() && pear_objc::responds_to(best, "makeKeyAndOrderFront:") {
        msg1::<Id, ()>(best, "makeKeyAndOrderFront:", NIL);
        if pear_objc::responds_to(best, "makeKeyWindow") {
            msg0::<()>(best, "makeKeyWindow");
        }
    }
    let mut key: Id = NIL;
    let mut key_area = 0.0;
    for w in scene_windows() {
        let b: CGRect = msg0(w, "bounds");
        let area = b.size.width * b.size.height;
        if area > key_area {
            key_area = area;
            key = w;
        }
    }
    if !key.is_null() && !msg0::<bool>(key, "isKeyWindow") {
        msg0::<()>(key, "makeKeyAndVisible");
    }
    let delegate: Id = msg0(ui_app(), "delegate");
    let app_window: Id = if !delegate.is_null() && pear_objc::responds_to(delegate, "window") { msg0(delegate, "window") } else { NIL };
    let root = |w: Id| -> String { if w.is_null() { "nil".into() } else { class_of(msg0(w, "rootViewController")) } };
    let key_bounds: CGRect = if key.is_null() { CGRect::default() } else { msg0(key, "bounds") };
    rt::log(&format!(
        "[PearRuntime][window] activation state={} key={} size={:.0}x{:.0} root={}",
        msg0::<isize>(ui_app(), "applicationState"),
        class_of(key),
        if key_area > 0.0 { key_bounds.size.width } else { 0.0 },
        if key_area > 0.0 { key_bounds.size.height } else { 0.0 },
        root(key)
    ));
    rt::log(&format!(
        "[PearRuntime][window] app delegate={} window={} root={} hidden={}",
        if delegate.is_null() { "(null)".into() } else { class_of(delegate) },
        class_of(app_window),
        root(app_window),
        (!app_window.is_null() && msg0::<bool>(app_window, "isHidden")) as i32
    ));
    for w in scene_windows() {
        rt::log(&format!(
            "[PearRuntime][window] scene window={} root={} hidden={} key={} bounds={}",
            class_of(w),
            root(w),
            msg0::<bool>(w, "isHidden") as i32,
            msg0::<bool>(w, "isKeyWindow") as i32,
            rect_string(msg0(w, "bounds"))
        ));
    }
    if key.is_null() && !app_window.is_null() {
        msg1::<bool, ()>(app_window, "setHidden:", false);
        msg0::<()>(app_window, "makeKeyAndVisible");
        rt::log("[PearRuntime][window] no scene window; using app window");
    }
}

unsafe fn activate_main_window() {
    rt::observe_main("UIApplicationDidBecomeActiveNotification", |_| unsafe { activate_main_window_once() });
    // The window may not exist yet when the runtime loads: retry a few times.
    for delay in [1.0, 2.5, 5.0] {
        rt::after_main(delay, || unsafe { activate_main_window_once() });
    }
}

// ---------------------------------------------------------------------------
// iOS-on-Mac flags

unsafe extern "C" fn returns_no(_: Id, _: Sel) -> bool {
    false
}

/// Many iOS games disable device features when they see isiOSAppOnMac /
/// isMacCatalystApp; the runtime already presents an iPad-class device.
unsafe fn install_ios_on_mac_compatibility() {
    if rt::env_bool("PEAR_KEEP_IOS_ON_MAC") {
        return;
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let c = rt::class_named("NSProcessInfo");
        if c.is_null() {
            return;
        }
        for name in ["isiOSAppOnMac", "isMacCatalystApp"] {
            let m = rt::class_getInstanceMethod(c, pear_objc::sel(name));
            if !m.is_null() {
                rt::method_setImplementation(m, returns_no as Imp);
            }
        }
        rt::log("[PearRuntime][platform] iOS-on-Mac flags reported as false");
    });
}

// ---------------------------------------------------------------------------
// Bootstrap

unsafe extern "C" fn bootstrap_imp(_: Id, _: Sel) {
    bootstrap();
}

/// `+[PearRuntime bootstrap]`, kept as the runtime's public entry point.
unsafe fn register_class() {
    if !rt::class_named("PearRuntime").is_null() {
        return;
    }
    if let Some(b) = rt::ClassBuilder::new::<()>("PearRuntime", cls("NSObject")) {
        let c = b.register();
        rt::class_addMethod(rt::object_getClass(c), pear_objc::sel("bootstrap"), bootstrap_imp as Imp, c"v@:".as_ptr());
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

pub unsafe fn bootstrap() {
    // dyld has consumed launcher insertion: do not propagate it, or the host
    // capability, to helpers created after bootstrap.
    if rt::env_set("PEAR_LAUNCHER_INJECTION") {
        host::is_configured(); // capture this process's endpoint before clearing it
        for name in ["DYLD_INSERT_LIBRARIES", "PEAR_HOST_SOCKET", "PEAR_HOST_TOKEN", "PEAR_LAUNCHER_INJECTION"] {
            std::env::remove_var(name);
        }
    }
    let input_module = rt::module_enabled("input");
    let graphics_module = rt::module_enabled("graphics");
    rt::log(&format!(
        "[PearRuntime] version {} modules={}",
        rt::cstr(registry::PearRuntimeVersion()),
        rt::cstr(registry::PearRuntimeEnabledModules())
    ));
    if graphics_module {
        crate::graphics::legacy_window::install_legacy_window_fix();
    }
    if input_module && !crate::input::virtual_gamepad::enabled() {
        crate::input::mouse_touch::install_mouse_to_touch_fallback();
    }
    if graphics_module {
        crate::graphics::fps_hud::install_fps_hud();
    }
    activate_main_window();

    // Skips this bootstrap for input debugging (other modules still load).
    if rt::env_set("PEAR_DISABLE") {
        rt::log("[PearRuntime] disabled via PEAR_DISABLE");
        return;
    }
    install_ios_on_mac_compatibility();
    if !input_module {
        rt::log("[PearRuntime] input module disabled; keymapper and controller bridge off");
        return;
    }
    crate::input::keymapper::start();

    if env("PEAR_TEST_TAP_AT").is_some() {
        test_taps(&env("PEAR_TEST_TAP_AT").unwrap_or_default());
    }
    if rt::env_set("PEAR_TEST_LOOK") {
        rt::after_main(8.0, || {
            rt::log("[PearRuntime][look] TEST mouse-look (synthetic deltas)");
            for i in 0..40 {
                rt::after_main(i as f64 * 0.03, || crate::input::keymapper::update_mouse_look(25.0, 8.0));
            }
        });
    }
    if rt::env_set("PEAR_TEST_SWIPE") {
        rt::after_main(8.0, || {
            rt::log("[PearRuntime][swipe] TEST swipe (0.3,0.5) -> (0.7,0.5)");
            crate::input::keymapper::test_swipe();
        });
    }
    if rt::env_set("PEAR_DEBUG_SCREEN") {
        log_screen_metrics();
    }
    if rt::env_set("PEAR_DEBUG_VIEWS") {
        log_view_tree();
    }
    if rt::env_set("PEAR_PROBE_HID") {
        probe_hid();
    }
}

/// PEAR_TEST_TAP_AT="x,y[,delay];…": taps for verifying the coordinate space.
fn test_taps(spec: &str) {
    for (index, tap) in spec.split(';').enumerate() {
        let parts: Vec<&str> = tap.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let nx = rt::ns_string_double(parts[0]);
        let ny = rt::ns_string_double(parts[1]);
        let delay = if parts.len() >= 3 { rt::ns_string_double(parts[2]) } else { 8.0 };
        rt::after_main(delay, move || unsafe {
            rt::log(&format!(
                "[PearRuntime][touch] TEST tap {index} at ({nx:.3},{ny:.3}) after {delay:.1}s state={}",
                msg0::<isize>(ui_app(), "applicationState")
            ));
            if rt::env_set("PEAR_TOUCH_CGEVENT") {
                crate::input::touch_synth::synthesize_tap_cgevent(nx, ny);
            } else {
                let key = format!("pear-test-at-{index}");
                crate::input::touch_synth::synthesize_virtual_finger(true, nsstring(&key), nx, ny);
                rt::after_main(0.08, move || unsafe {
                    crate::input::touch_synth::synthesize_virtual_finger(false, nsstring(&key), nx, ny);
                });
            }
        });
    }
}

pub fn init() {
    unsafe {
        register_class();
        bootstrap();
    }
}

// ---------------------------------------------------------------------------
// Diagnostics (opt-in)

extern "C" {
    fn class_copyIvarList(c: Id, count: *mut u32) -> *mut *mut c_void;
    fn ivar_getName(v: *mut c_void) -> *const c_char;
    fn ivar_getTypeEncoding(v: *mut c_void) -> *const c_char;
    fn objc_copyClassList(count: *mut u32) -> *mut Id;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn NSStringFromCGAffineTransform(t: AffineTransform) -> Id;
    fn NSStringFromCGSize(s: pear_objc::CGSize) -> Id;
    fn NSStringFromCGPoint(p: pear_objc::CGPoint) -> Id;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Transform3D {
    m: [f64; 16],
}

fn icontains(haystack: &str, needle: &str) -> bool {
    haystack.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
}

/// Whether the pieces for native (real UITouch) synthesis are reachable.
fn probe_hid() {
    rt::after_main(3.0, || unsafe {
        let iokit = rt::dlopen_path("/System/Library/Frameworks/IOKit.framework/IOKit", rt::RTLD_LAZY);
        rt::log(&format!(
            "[PearRuntime][probe] IOKit={:p} DigitizerEvent={:p} DigitizerFingerEvent={:p} EventSystemClientCreate={:p}",
            iokit,
            dlsym(iokit, c"IOHIDEventCreateDigitizerEvent".as_ptr()),
            dlsym(iokit, c"IOHIDEventCreateDigitizerFingerEvent".as_ptr()),
            dlsym(iokit, c"IOHIDEventSystemClientCreate".as_ptr())
        ));
        let gs = rt::dlopen_path("/System/Library/PrivateFrameworks/GraphicsServices.framework/GraphicsServices", rt::RTLD_LAZY);
        rt::log(&format!(
            "[PearRuntime][probe] GraphicsServices={gs:p} GSSendEvent={:p}",
            if gs.is_null() { std::ptr::null_mut() } else { dlsym(gs, c"GSSendEvent".as_ptr()) }
        ));
        let app = ui_app();
        rt::log(&format!(
            "[PearRuntime][probe] app _enqueueHIDEvent: = {} | _enqueueHIDEventRecord: = {}",
            pear_objc::responds_to(app, "_enqueueHIDEvent:") as i32,
            pear_objc::responds_to(app, "_enqueueHIDEventRecord:") as i32
        ));
        let mut c = cls("UIApplication");
        while !c.is_null() {
            let mut n = 0u32;
            let ivars = class_copyIvarList(c, &mut n);
            for i in 0..n as usize {
                let name = rt::cstr(ivar_getName(*ivars.add(i)));
                if ["hid", "observer", "eventqueue"].iter().any(|k| icontains(name, k)) {
                    rt::log(&format!(
                        "[PearRuntime][probe]   ivar {} :: {name} {}",
                        rt::class_name(c),
                        rt::cstr(ivar_getTypeEncoding(*ivars.add(i)))
                    ));
                }
            }
            rt::free(ivars as *mut c_void);
            c = rt::class_getSuperclass(c);
        }
        // Touches arrive through the UIKitMac glue: look for its classes.
        let mut count = 0u32;
        let classes = objc_copyClassList(&mut count);
        for i in 0..count as usize {
            let k = *classes.add(i);
            let name = rt::class_name(k);
            if !["UINSVirtual", "VirtualDigitizer", "VirtualFinger"].iter().any(|n| icontains(name, n)) {
                continue;
            }
            let sup = rt::class_getSuperclass(k);
            rt::log(&format!("[PearRuntime][probe] CLASS {name} : {}", if sup.is_null() { "-" } else { rt::class_name(sup) }));
            let mut n = 0u32;
            let ivars = class_copyIvarList(k, &mut n);
            for j in 0..n as usize {
                rt::log(&format!(
                    "[PearRuntime][probe]   ivar {} {}",
                    rt::cstr(ivar_getName(*ivars.add(j))),
                    rt::cstr(ivar_getTypeEncoding(*ivars.add(j)))
                ));
            }
            rt::free(ivars as *mut c_void);
            let mut m = 0u32;
            let methods = rt::class_copyMethodList(k, &mut m);
            for j in 0..m as usize {
                rt::log(&format!("[PearRuntime][probe]   meth {}", rt::cstr(rt::sel_getName(rt::method_getName(*methods.add(j))))));
            }
            rt::free(methods as *mut c_void);
        }
        rt::free(classes as *mut c_void);
        let mut c = cls("UIApplication");
        while !c.is_null() {
            let mut n = 0u32;
            let methods = rt::class_copyMethodList(c, &mut n);
            for i in 0..n as usize {
                let name = rt::cstr(rt::sel_getName(rt::method_getName(*methods.add(i))));
                if (icontains(name, "hid") || icontains(name, "enqueue")) && name.contains(':') {
                    rt::log(&format!("[PearRuntime][probe]   {} :: {name}", rt::class_name(c)));
                }
            }
            rt::free(methods as *mut c_void);
            c = rt::class_getSuperclass(c);
        }
    });
}

unsafe fn dump_view(view: Id, depth: usize) {
    if depth > 40 {
        return;
    }
    let indent = "  ".repeat(depth);
    let t: AffineTransform = msg0(view, "transform");
    rt::log(&format!(
        "[PearRuntime][views] {indent}{} frame={} bounds={} transform={} scale={:.2} hidden={} alpha={:.2} layer={}",
        class_of(view),
        rect_string(msg0(view, "frame")),
        rect_string(msg0(view, "bounds")),
        rt::ns_string(NSStringFromCGAffineTransform(t)),
        msg0::<f64>(view, "contentScaleFactor"),
        msg0::<bool>(view, "isHidden") as i32,
        msg0::<f64>(view, "alpha"),
        class_of(msg0(view, "layer"))
    ));
    let subviews: Id = msg0(view, "subviews");
    for i in 0..pear_objc::nsarray_count(subviews) {
        dump_view(pear_objc::nsarray_object_at(subviews, i), depth + 1);
    }
}

unsafe fn dump_native(view: Id, depth: usize) {
    if view.is_null() || depth > 20 {
        return;
    }
    let frame: CGRect = msg0(view, "frame");
    let bounds: CGRect = msg0(view, "bounds");
    let hidden: bool = msg0(view, "isHidden");
    let layer: Id = if pear_objc::responds_to(view, "layer") { msg0(view, "layer") } else { NIL };
    let sublayers = if layer.is_null() { 0 } else { pear_objc::nsarray_count(msg0(layer, "sublayers")) };
    rt::log(&format!("[PearRuntime][native-view] layer={} sublayers={sublayers}", class_of(layer)));
    rt::log(&format!(
        "[PearRuntime][native-view] depth={depth} class={} frame={} bounds={} hidden={}",
        class_of(view),
        rect_string(frame),
        rect_string(bounds),
        hidden as i32
    ));
    let subviews: Id = msg0(view, "subviews");
    for i in 0..pear_objc::nsarray_count(subviews) {
        dump_native(pear_objc::nsarray_object_at(subviews, i), depth + 1);
    }
}

unsafe fn dump_layer(layer: Id, depth: usize) {
    if depth > 40 {
        return;
    }
    let indent = "  ".repeat(depth);
    let mut extra = String::new();
    if pear_objc::is_kind_of(layer, "CAMetalLayer") {
        extra = format!(
            " drawableSize={} device={} pixelFormat={} framebufferOnly={}",
            rt::ns_string(NSStringFromCGSize(msg0(layer, "drawableSize"))),
            if msg0::<Id>(layer, "device").is_null() { "nil" } else { "yes" },
            msg0::<usize>(layer, "pixelFormat") as i32,
            msg0::<bool>(layer, "framebufferOnly") as i32
        );
    }
    let t: Transform3D = msg0(layer, "transform");
    rt::log(&format!(
        "[PearRuntime][layers] {indent}{} bounds={} pos={} transform=[{:.3} {:.3} {:.3} {:.3}] opacity={:.2} hidden={}{extra}",
        class_of(layer),
        rect_string(msg0(layer, "bounds")),
        rt::ns_string(NSStringFromCGPoint(msg0(layer, "position"))),
        t.m[0],
        t.m[1],
        t.m[4],
        t.m[5],
        msg0::<f32>(layer, "opacity"),
        msg0::<bool>(layer, "isHidden") as i32
    ));
    let subs: Id = msg0(layer, "sublayers");
    for i in 0..pear_objc::nsarray_count(subs) {
        dump_layer(pear_objc::nsarray_object_at(subs, i), depth + 1);
    }
}

fn log_view_tree() {
    rt::after_main(6.0, || unsafe {
        for w in scene_windows() {
            rt::log(&format!(
                "[PearRuntime][views] window {} level={:.0} hidden={} key={} frame={}",
                class_of(w),
                msg0::<f64>(w, "windowLevel"),
                msg0::<bool>(w, "isHidden") as i32,
                msg0::<bool>(w, "isKeyWindow") as i32,
                rect_string(msg0(w, "frame"))
            ));
            let root: Id = msg0(w, "rootViewController");
            if !root.is_null() {
                let loaded: bool = msg0(root, "isViewLoaded");
                rt::log(&format!("[PearRuntime][views]   rootVC={} viewLoaded={}", class_of(root), loaded as i32));
                if loaded {
                    let v: Id = msg0(root, "view");
                    let sup: Id = msg0(v, "superview");
                    rt::log(&format!(
                        "[PearRuntime][views]   rootVC.view={} frame={} superview={} hidden={}",
                        class_of(v),
                        rect_string(msg0(v, "frame")),
                        class_of(sup),
                        msg0::<bool>(v, "isHidden") as i32
                    ));
                }
            }
            dump_view(w, 1);
        }
        // Modal legacy alerts can live on their own window outside the scene list.
        let windows: Id = msg0(ui_app(), "windows");
        for i in 0..pear_objc::nsarray_count(windows) {
            let w = pear_objc::nsarray_object_at(windows, i);
            rt::log(&format!(
                "[PearRuntime][views] app window {} level={:.0} hidden={} key={} alpha={:.2} frame={}",
                class_of(w),
                msg0::<f64>(w, "windowLevel"),
                msg0::<bool>(w, "isHidden") as i32,
                msg0::<bool>(w, "isKeyWindow") as i32,
                msg0::<f64>(w, "alpha"),
                rect_string(msg0(w, "frame"))
            ));
            dump_view(w, 1);
        }
        // The Catalyst scene bridge is an AppKit view hierarchy.
        let nsapp = ns_app();
        if !nsapp.is_null() {
            let windows: Id = msg0(nsapp, "windows");
            for i in 0..pear_objc::nsarray_count(windows) {
                let w = pear_objc::nsarray_object_at(windows, i);
                rt::log(&format!(
                    "[PearRuntime][native-view] window={} frame={} visible={}",
                    class_of(w),
                    rect_string(msg0(w, "frame")),
                    msg0::<bool>(w, "isVisible") as i32
                ));
                dump_native(msg0(w, "contentView"), 0);
            }
        }
        for w in scene_windows() {
            dump_layer(msg0(w, "layer"), 0);
        }
    });
}

fn log_screen_metrics() {
    rt::after_main(4.0, || unsafe {
        let screen: Id = msg0(cls("UIScreen"), "mainScreen");
        rt::log(&format!(
            "[PearRuntime][screen] bounds={} nativeBounds={} scale={:.2} nativeScale={:.2}",
            rect_string(msg0(screen, "bounds")),
            rect_string(msg0(screen, "nativeBounds")),
            msg0::<f64>(screen, "scale"),
            msg0::<f64>(screen, "nativeScale")
        ));
        let scenes: Id = msg0(msg0::<Id>(ui_app(), "connectedScenes"), "allObjects");
        for i in 0..pear_objc::nsarray_count(scenes) {
            let scene = pear_objc::nsarray_object_at(scenes, i);
            if !pear_objc::is_kind_of(scene, "UIWindowScene") {
                continue;
            }
            let space: Id = msg0(scene, "coordinateSpace");
            rt::log(&format!(
                "[PearRuntime][screen] scene={} coords={}",
                if msg0::<isize>(scene, "activationState") == 0 { "active" } else { "other" },
                rect_string(msg0(space, "bounds"))
            ));
            rt::log(&format!(
                "[PearRuntime][screen] orientation statusBar={} scene={} device={}",
                msg0::<isize>(ui_app(), "statusBarOrientation"),
                msg0::<isize>(scene, "interfaceOrientation"),
                msg0::<isize>(msg0::<Id>(cls("UIDevice"), "currentDevice"), "orientation")
            ));
            let windows: Id = msg0(scene, "windows");
            for j in 0..pear_objc::nsarray_count(windows) {
                let w = pear_objc::nsarray_object_at(windows, j);
                rt::log(&format!(
                    "[PearRuntime][screen] window frame={} bounds={} key={}",
                    rect_string(msg0(w, "frame")),
                    rect_string(msg0(w, "bounds")),
                    msg0::<bool>(w, "isKeyWindow") as i32
                ));
            }
        }
    });
}

