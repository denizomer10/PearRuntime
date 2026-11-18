//! One performance HUD for every iOS game: Apple's Metal HUD (MTL_HUD_ENABLED).
//!
//! Metal HUD attaches to the CAMetalLayer an app presents to; OpenGL ES games
//! present through EAGL instead. GL titles get a transparent CAMetalLayer on
//! top of the window that presents one cleared frame per game present (from
//! the EAGL present hook), so the HUD shows the game's own FPS. While the game
//! presents to another Metal layer, the carrier stays idle so only one HUD shows.

use crate::rt;
use pear_objc::{cls, msg0, msg1, nsstring, objc_retain, CGRect, CGSize, Id, NIL};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU64, Ordering};
use std::sync::OnceLock;

#[link(name = "Metal", kind = "framework")]
extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}
extern "C" {
    fn clock_gettime_nsec_np(clock: u32) -> u64;
}
const CLOCK_UPTIME_RAW: u32 = 8;
const BGRA8_UNORM: usize = 80;

#[repr(C)]
#[derive(Copy, Clone)]
struct ClearColor {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

static CARRIER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static QUEUE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static IN_FLIGHT: AtomicI32 = AtomicI32::new(0);
static INSTALLING: AtomicBool = AtomicBool::new(false);
/// Last time (CLOCK_UPTIME_RAW ns) any other Metal layer asked for a drawable.
static LAST_APP_DRAWABLE: AtomicU64 = AtomicU64::new(0);

fn hud_requested() -> bool {
    static R: OnceLock<bool> = OnceLock::new();
    *R.get_or_init(|| matches!(std::env::var("MTL_HUD_ENABLED"), Ok(v) if !v.is_empty() && v != "0"))
}

fn now() -> u64 {
    unsafe { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) }
}

/// Called from the CAMetalLayer nextDrawable hook for every layer.
#[export_name = "_Z24PearMetalHUDNoteDrawablePv"]
pub extern "C" fn note_drawable(layer: *mut c_void) {
    if layer != CARRIER.load(Ordering::Relaxed) {
        LAST_APP_DRAWABLE.store(now(), Ordering::Relaxed);
    }
}

unsafe fn size_carrier() {
    let carrier = CARRIER.load(Ordering::Acquire) as Id;
    let window = rt::primary_window();
    if carrier.is_null() || window.is_null() {
        return;
    }
    let host: Id = msg0(window, "layer");
    if msg0::<Id>(carrier, "superlayer") != host {
        msg0::<()>(carrier, "removeFromSuperlayer");
        msg1::<Id, ()>(host, "addSublayer:", carrier);
    }
    let bounds: CGRect = msg0(host, "bounds");
    let trait_scale: f64 = msg0(msg0::<Id>(window, "traitCollection"), "displayScale");
    let scale = if trait_scale > 0.0 { trait_scale } else { 1.0 };
    let drawable = CGSize { width: (bounds.size.width * scale).round(), height: (bounds.size.height * scale).round() };
    let frame: CGRect = msg0(carrier, "frame");
    let current: CGSize = msg0(carrier, "drawableSize");
    if frame != bounds || current != drawable {
        rt::without_actions(|| {
            msg1::<CGRect, ()>(carrier, "setFrame:", bounds);
            msg1::<f64, ()>(carrier, "setContentsScale:", scale);
            if drawable.width >= 1.0 && drawable.height >= 1.0 {
                msg1::<CGSize, ()>(carrier, "setDrawableSize:", drawable);
            }
        });
    }
}

extern "C" fn size_tick(_: *mut c_void) {
    unsafe { size_carrier() }
}

unsafe fn install_carrier() {
    if !CARRIER.load(Ordering::Acquire).is_null() {
        return;
    }
    let device = MTLCreateSystemDefaultDevice();
    let window = rt::primary_window();
    if device.is_null() || window.is_null() {
        INSTALLING.store(false, Ordering::Release);
        return;
    }
    let carrier: Id = objc_retain(msg0(cls("CAMetalLayer"), "layer"));
    msg1::<Id, ()>(carrier, "setDevice:", device);
    msg1::<usize, ()>(carrier, "setPixelFormat:", BGRA8_UNORM);
    msg1::<bool, ()>(carrier, "setOpaque:", false);
    msg1::<bool, ()>(carrier, "setFramebufferOnly:", true);
    msg1::<f64, ()>(carrier, "setZPosition:", 10000.0);
    msg1::<Id, ()>(carrier, "setName:", nsstring("PearMetalHUDCarrier"));
    msg1::<bool, ()>(carrier, "setAllowsNextDrawableTimeout:", true);
    QUEUE.store(msg0::<Id>(device, "newCommandQueue") as *mut _, Ordering::Release);
    CARRIER.store(carrier as *mut _, Ordering::Release);
    size_carrier();
    for name in [
        "NSWindowDidResizeNotification",
        "NSWindowDidEnterFullScreenNotification",
        "NSWindowDidExitFullScreenNotification",
        "NSWindowDidChangeBackingPropertiesNotification",
        "UIWindowDidBecomeKeyNotification",
    ] {
        rt::observe_main(name, |_| {
            size_carrier();
            rt::async_main(|| size_carrier());
        });
    }
    // UIKit settles a new window size a little after AppKit reports it, and a
    // scene can swap its root window; a slow check covers both.
    rt::main_timer(1.0, 1.0, 0.25, size_tick);
    rt::log("[PearRuntime][hud] Metal HUD carrier installed for OpenGL ES presents");
}

/// Called from the EAGL `presentRenderbuffer:` hook after a successful present.
#[export_name = "_Z24PearRecordFramePresentedv"]
pub extern "C" fn record_frame_presented() {
    if !hud_requested() {
        return;
    }
    let carrier = CARRIER.load(Ordering::Acquire) as Id;
    if carrier.is_null() {
        if !INSTALLING.swap(true, Ordering::AcqRel) {
            rt::async_main(|| unsafe { pear_objc::autorelease_pool(|| install_carrier()) });
        }
        return;
    }
    // The game is drawing a Metal screen right now; Metal HUD is on that layer.
    if now().wrapping_sub(LAST_APP_DRAWABLE.load(Ordering::Relaxed)) < 300_000_000 {
        return;
    }
    // Never block the game's render thread: at most two carrier frames in flight.
    if IN_FLIGHT.fetch_add(1, Ordering::AcqRel) >= 2 {
        IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
        return;
    }
    unsafe {
        pear_objc::autorelease_pool(|| {
            let drawable: Id = msg0(carrier, "nextDrawable");
            let commands: Id = if drawable.is_null() { NIL } else { msg0(QUEUE.load(Ordering::Acquire) as Id, "commandBuffer") };
            if commands.is_null() {
                IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
                return;
            }
            let pass: Id = msg0(cls("MTLRenderPassDescriptor"), "renderPassDescriptor");
            let color: Id = msg1(msg0::<Id>(pass, "colorAttachments"), "objectAtIndexedSubscript:", 0usize);
            msg1::<Id, ()>(color, "setTexture:", msg0(drawable, "texture"));
            msg1::<usize, ()>(color, "setLoadAction:", 2); // clear
            msg1::<usize, ()>(color, "setStoreAction:", 1); // store
            msg1::<ClearColor, ()>(color, "setClearColor:", ClearColor { r: 0.0, g: 0.0, b: 0.0, a: 0.0 });
            msg0::<()>(msg1::<Id, Id>(commands, "renderCommandEncoderWithDescriptor:", pass), "endEncoding");
            let done = rt::block1(|_| {
                IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
            });
            msg1::<Id, ()>(commands, "addCompletedHandler:", done);
            rt::release_block(done);
            msg1::<Id, ()>(commands, "presentDrawable:", drawable);
            msg0::<()>(commands, "commit");
        });
    }
}

/// Kept for the bootstrap call; nothing to install until a GL frame arrives.
#[export_name = "_Z17PearInstallFPSHUDv"]
pub extern "C" fn install_fps_hud() {
    if hud_requested() {
        rt::log("[PearRuntime][hud] Metal HUD requested; OpenGL ES titles get a carrier layer");
    }
}
