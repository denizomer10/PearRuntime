//! Makes UIScreen.main describe the app's window rather than the whole Mac
//! display: games that size their framebuffer from UIScreen otherwise render
//! a display-sized image into a smaller window. The geometry accessors follow
//! the key window live. PEAR_NO_SCREEN_SHIM disables it.
//!
//! scale/nativeScale report the display's backing scale times
//! PEAR_RENDER_SCALE (native | 0.25…1 | points), never below 1x; nativeBounds
//! is the window in pixels at that scale. Orientation accessors report the
//! window's shape instead of the scene wrapper's stale flag
//! (PEAR_NO_ORIENTATION_SHIM disables that part).

use crate::rt;
use pear_objc::{cls, msg0, msg1, nsstring, CGRect, CGSize, Id, Imp, Sel, NIL};
use std::cell::Cell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

extern "C" {
    fn clock_gettime_nsec_np(clock: u32) -> u64;
    fn NSStringFromCGSize(s: CGSize) -> Id;
}
const CLOCK_UPTIME_RAW: u32 = 8;

const ORIENTATION_UNKNOWN: isize = 0;
const ORIENTATION_PORTRAIT: isize = 1;
const ORIENTATION_LANDSCAPE_RIGHT: isize = 3;
const ORIENTATION_LANDSCAPE_LEFT: isize = 4;
const DEVICE_PORTRAIT: isize = 1;
const DEVICE_LANDSCAPE_LEFT: isize = 3;

struct Geometry {
    override_size: CGSize,
    cached: CGSize,
    cached_at: u64,
}

static GEOMETRY: Mutex<Geometry> = Mutex::new(Geometry {
    override_size: CGSize { width: 0.0, height: 0.0 },
    cached: CGSize { width: 0.0, height: 0.0 },
    cached_at: 0,
});
pub(crate) static REFRESH_QUEUED: AtomicBool = AtomicBool::new(false);
static ORIENTATION_READ: AtomicBool = AtomicBool::new(false);
/// Display backing scale as f64 bits (0 = unknown), cached for render threads.
static DISPLAY_SCALE: AtomicU64 = AtomicU64::new(0);
static LOGGED_SCALE: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_SCALE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_BOUNDS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_STATUS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_SCENE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_DEVICE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_CONTROLLER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

fn geometry() -> std::sync::MutexGuard<'static, Geometry> {
    GEOMETRY.lock().unwrap_or_else(|e| e.into_inner())
}

fn now() -> u64 {
    unsafe { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) }
}

fn is_main() -> bool {
    unsafe { msg0::<bool>(cls("NSThread"), "isMainThread") }
}

/// Size the GL shim publishes once a fixed drawable exists: authoritative,
/// so a Mac window resize does not move the engine's coordinate space.
#[export_name = "_Z21PearSetScreenOverride6CGSize"]
pub extern "C" fn set_screen_override(size: CGSize) {
    geometry().override_size = size;
}

#[export_name = "_Z23PearClearScreenOverridev"]
pub extern "C" fn clear_screen_override() {
    set_screen_override(CGSize::default());
}

thread_local! {
    static REFRESHING: Cell<bool> = const { Cell::new(false) };
}

/// Main thread only. Lazy app-delegate window getters can ask UIScreen for
/// their size; re-entering the producer consumes the existing snapshot.
unsafe fn refresh_window_bounds() {
    if REFRESHING.with(|r| r.replace(true)) {
        return;
    }
    let window = rt::primary_window();
    let size = if window.is_null() { CGSize::default() } else { msg0::<CGRect>(window, "bounds").size };
    if size.width > 0.0 && size.height > 0.0 {
        let changed = {
            let mut g = geometry();
            let changed = g.cached != size;
            g.cached = size;
            g.cached_at = now();
            changed
        };
        if changed && rt::env_set("PEAR_DEBUG_METAL") {
            rt::log(&format!("[PearRuntime][screen] published window geometry {}", rt::ns_string(NSStringFromCGSize(size))));
        }
    }
    REFRESHING.with(|r| r.set(false));
}

fn queue_screen_refresh() {
    if REFRESH_QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }
    rt::async_main(|| unsafe {
        refresh_window_bounds();
        REFRESH_QUEUED.store(false, Ordering::Release);
    });
}

/// Render threads read a main-thread snapshot, never the UIKit window tree;
/// a stale snapshot queues one coalesced refresh.
pub(crate) unsafe fn reported_bounds() -> CGRect {
    let (override_size, mut cached, age) = {
        let g = geometry();
        (g.override_size, g.cached, now().wrapping_sub(g.cached_at))
    };
    if override_size.width > 0.0 && override_size.height > 0.0 {
        return CGRect { origin: Default::default(), size: override_size };
    }
    if age >= 50_000_000 || cached.width <= 0.0 || cached.height <= 0.0 {
        if is_main() {
            refresh_window_bounds();
            cached = geometry().cached;
        } else {
            queue_screen_refresh();
        }
    }
    if cached.width > 0.0 && cached.height > 0.0 {
        return CGRect { origin: Default::default(), size: cached };
    }
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::AcqRel) {
        rt::log("[PearRuntime][screen] no window yet; reporting 1024x768 fallback");
    }
    CGRect { origin: Default::default(), size: CGSize { width: 1024.0, height: 768.0 } }
}

/// PEAR_RENDER_SCALE multiplier; 0 means points (1.0).
fn render_scale_factor() -> f64 {
    static F: OnceLock<f64> = OnceLock::new();
    *F.get_or_init(|| match std::env::var("PEAR_RENDER_SCALE") {
        Ok(v) if !v.is_empty() && !v.eq_ignore_ascii_case("native") => {
            if v.eq_ignore_ascii_case("points") || v.eq_ignore_ascii_case("legacy") {
                0.0
            } else {
                let parsed = atof(&v);
                if (0.25..=1.0).contains(&parsed) { parsed } else { 1.0 }
            }
        }
        _ => 1.0,
    })
}

/// `atof`: the longest numeric prefix, 0 when there is none.
fn atof(s: &str) -> f64 {
    let t = s.trim_start();
    let end = t
        .char_indices()
        .take_while(|&(i, c)| c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && i == 0))
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    (1..=end).rev().find_map(|n| t[..n].parse::<f64>().ok()).unwrap_or(0.0)
}

unsafe fn display_scale(screen: Id) -> f64 {
    let cached = f64::from_bits(DISPLAY_SCALE.load(Ordering::Acquire));
    let main = is_main();
    if cached > 0.0 && !main {
        return cached;
    }
    let mut scale = 0.0;
    if main {
        let window = rt::primary_window();
        if !window.is_null() {
            scale = msg0(msg0::<Id>(window, "traitCollection"), "displayScale");
        }
    }
    // Before the window exists Catalyst's UIScreen reports a nominal 2.0 even
    // on a 1x display: ask AppKit for the real display.
    if scale <= 0.0 && main {
        let ns_screen = cls("NSScreen");
        if !ns_screen.is_null() && pear_objc::responds_to(ns_screen, "mainScreen") {
            let main_screen: Id = msg0(ns_screen, "mainScreen");
            if pear_objc::responds_to(main_screen, "backingScaleFactor") {
                scale = msg0(main_screen, "backingScaleFactor");
            }
        }
    }
    let original = ORIGINAL_SCALE.load(Ordering::Acquire);
    if scale <= 0.0 && !original.is_null() && !screen.is_null() {
        let f: unsafe extern "C" fn(Id, Sel) -> f64 = std::mem::transmute(original);
        scale = f(screen, pear_objc::sel("scale"));
    }
    if scale <= 0.0 {
        scale = if cached > 0.0 { cached } else { 2.0 };
    }
    if scale != cached {
        DISPLAY_SCALE.store(scale.to_bits(), Ordering::Release);
    }
    if scale.to_bits() != LOGGED_SCALE.swap(scale.to_bits(), Ordering::AcqRel) {
        let factor = render_scale_factor();
        rt::log(&format!(
            "[PearRuntime][screen] display backing scale {scale:.2}, render scale {:.2}",
            if factor == 0.0 { 1.0 } else { (scale * factor).max(1.0) }
        ));
    }
    scale
}

unsafe fn reported_scale(screen: Id) -> f64 {
    let factor = render_scale_factor();
    if factor == 0.0 { 1.0 } else { (display_scale(screen) * factor).max(1.0) }
}

/// The orientation the window's shape implies (a fixed GL drawable wins).
unsafe fn desired_orientation() -> isize {
    let shape = |s: CGSize| {
        if s.width > s.height + 1.0 {
            ORIENTATION_LANDSCAPE_LEFT
        } else if s.height > s.width + 1.0 {
            ORIENTATION_PORTRAIT
        } else {
            ORIENTATION_UNKNOWN
        }
    };
    let override_size = geometry().override_size;
    if override_size.width > 0.0 && override_size.height > 0.0 {
        return shape(override_size);
    }
    let window = rt::primary_window();
    if window.is_null() {
        return ORIENTATION_UNKNOWN;
    }
    let size = msg0::<CGRect>(window, "bounds").size;
    if size.width < 1.0 || size.height < 1.0 {
        return ORIENTATION_UNKNOWN;
    }
    shape(size)
}

unsafe extern "C" fn bounds(s: Id, c: Sel) -> CGRect {
    let r = reported_bounds();
    if r.size.width.is_finite() {
        return r;
    }
    let f: unsafe extern "C" fn(Id, Sel) -> CGRect = std::mem::transmute(ORIGINAL_BOUNDS.load(Ordering::Acquire));
    f(s, c)
}
unsafe extern "C" fn scale(s: Id, _c: Sel) -> f64 {
    reported_scale(s)
}
/// Pixels, as on a device: backbuffers sized from nativeBounds match the window.
unsafe extern "C" fn native_bounds(s: Id, _c: Sel) -> CGRect {
    let r = reported_bounds();
    let k = reported_scale(s);
    CGRect { origin: Default::default(), size: CGSize { width: (r.size.width * k).round(), height: (r.size.height * k).round() } }
}

unsafe fn orientation_or(slot: &AtomicPtr<c_void>, s: Id, c: Sel) -> isize {
    if is_main() {
        let d = desired_orientation();
        if d != ORIENTATION_UNKNOWN {
            return d;
        }
    }
    let f: unsafe extern "C" fn(Id, Sel) -> isize = std::mem::transmute(slot.load(Ordering::Acquire));
    f(s, c)
}
unsafe extern "C" fn status_orientation(s: Id, c: Sel) -> isize {
    orientation_or(&ORIGINAL_STATUS, s, c)
}
unsafe extern "C" fn scene_orientation(s: Id, c: Sel) -> isize {
    orientation_or(&ORIGINAL_SCENE, s, c)
}
/// Catalyst reports UIDeviceOrientationUnknown, which ports treat as portrait.
unsafe extern "C" fn device_orientation(s: Id, c: Sel) -> isize {
    if is_main() {
        match desired_orientation() {
            ORIENTATION_LANDSCAPE_LEFT | ORIENTATION_LANDSCAPE_RIGHT => return DEVICE_LANDSCAPE_LEFT,
            ORIENTATION_PORTRAIT => return DEVICE_PORTRAIT,
            _ => {}
        }
    }
    let f: unsafe extern "C" fn(Id, Sel) -> isize = std::mem::transmute(ORIGINAL_DEVICE.load(Ordering::Acquire));
    f(s, c)
}
/// The deprecated controller property (old UIKit code, MonoGame) stays at
/// its launch value otherwise.
unsafe extern "C" fn controller_orientation(s: Id, c: Sel) -> isize {
    ORIENTATION_READ.store(true, Ordering::Release);
    orientation_or(&ORIGINAL_CONTROLLER, s, c)
}

unsafe fn hook(class: &str, selector: &str, imp: Imp, slot: Option<&AtomicPtr<c_void>>) -> bool {
    let c = cls(class);
    if c.is_null() {
        return false;
    }
    let m = rt::class_getInstanceMethod(c, pear_objc::sel(selector));
    if m.is_null() {
        return false;
    }
    let old = rt::method_setImplementation(m, imp);
    if let Some(slot) = slot {
        slot.store(old as *mut c_void, Ordering::Release);
    }
    true
}

pub fn init() {
    if !rt::module_enabled("graphics") || rt::env_set("PEAR_NO_SCREEN_SHIM") {
        return;
    }
    unsafe {
        if cls("UIScreen").is_null() {
            return;
        }
        hook("UIScreen", "bounds", bounds as Imp, Some(&ORIGINAL_BOUNDS));
        hook("UIScreen", "scale", scale as Imp, Some(&ORIGINAL_SCALE));
        hook("UIScreen", "nativeScale", scale as Imp, None);
        hook("UIScreen", "nativeBounds", native_bounds as Imp, None);
        // AppKit notifications can precede UIKit's final bounds: refresh now
        // and after layout settles.
        for name in [
            "NSWindowDidResizeNotification",
            "NSWindowDidEnterFullScreenNotification",
            "NSWindowDidExitFullScreenNotification",
            "UIWindowDidBecomeKeyNotification",
            "NSWindowDidChangeBackingPropertiesNotification",
            "NSWindowDidChangeScreenNotification",
        ] {
            rt::observe_main(name, |_| {
                DISPLAY_SCALE.store(0, Ordering::Release);
                refresh_window_bounds();
                display_scale(NIL);
                rt::after_main(0.1, || refresh_window_bounds());
            });
        }
        rt::log("[PearRuntime][screen] shim installed (UIScreen follows the window at native backing scale)");
        if rt::env_set("PEAR_NO_ORIENTATION_SHIM") {
            rt::log("[PearRuntime][screen] orientation shim disabled");
            return;
        }
        hook("UIApplication", "statusBarOrientation", status_orientation as Imp, Some(&ORIGINAL_STATUS));
        hook("UIWindowScene", "interfaceOrientation", scene_orientation as Imp, Some(&ORIGINAL_SCENE));
        hook("UIDevice", "orientation", device_orientation as Imp, Some(&ORIGINAL_DEVICE));
        hook("UIViewController", "interfaceOrientation", controller_orientation as Imp, Some(&ORIGINAL_CONTROLLER));
        rt::log("[PearRuntime][screen] interface orientation follows the window");
    }
    // Apps that cached the launch orientation through the deprecated controller
    // property get one notification nudge; broadcasting to every app makes
    // UIKit rebuild navigation bars mid-transition.
    rt::after_main(2.0, || unsafe {
        if !ORIENTATION_READ.load(Ordering::Acquire) {
            rt::log("[PearRuntime][screen] orientation notification skipped (no cached read)");
            return;
        }
        let center: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        msg1::<Id, ()>(center, "postNotificationName:object:", nsstring("UIDeviceOrientationDidChangeNotification"));
        msg1::<Id, ()>(center, "postNotificationName:object:", nsstring("UIApplicationDidChangeStatusBarOrientationNotification"));
        rt::log("[PearRuntime][screen] posted orientation change");
    });
}
