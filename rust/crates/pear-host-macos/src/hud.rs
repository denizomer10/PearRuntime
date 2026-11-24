//! Apple Metal HUD (`MTL_HUD_ENABLED`) integration across PearHost OpenGL ES
//! and Metal games:
//!
//! - Configures `developerHUDProperties = @{ @"mode": @"default" }` on every
//!   app `CAMetalLayer` (including ANGLE's Metal sublayer and `MTKView` layers)
//!   when `nextDrawable` is requested.
//! - Provides a transparent top-level `PearMetalHUDCarrier` layer driven by
//!   `PearRecordFramePresented()` when OpenGL ES presents without an active
//!   external `CAMetalLayer nextDrawable`, ensuring the HUD stays visible.

use pear_objc::{
    assoc_get, assoc_set, autorelease_pool, cls, msg0, msg1, msg2, nsnumber_bool, nsstring,
    objc_retain, responds_to, sel, CGRect, CGSize, Id, Imp, Sel, NIL,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;

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

#[repr(C)]
struct GlobalBlockDescriptor {
    reserved: usize,
    size: usize,
}

#[repr(C)]
struct GlobalBlockLiteral {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
    descriptor: *const GlobalBlockDescriptor,
}

unsafe impl Sync for GlobalBlockLiteral {}
unsafe impl Sync for GlobalBlockDescriptor {}

extern "C" {
    static _dispatch_main_q: c_void;
    static _NSConcreteGlobalBlock: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn clock_gettime_nsec_np(clock_id: u32) -> u64;
    fn MTLCreateSystemDefaultDevice() -> Id;
    fn class_getInstanceMethod(cls: pear_objc::Class, name: Sel) -> *mut c_void;
    fn method_setImplementation(method: *mut c_void, imp: Imp) -> Imp;
}

static CARRIER: AtomicUsize = AtomicUsize::new(0);
static QUEUE: AtomicUsize = AtomicUsize::new(0);
static IN_FLIGHT: AtomicI32 = AtomicI32::new(0);
static INSTALLING: AtomicBool = AtomicBool::new(false);
static LAST_APP_DRAWABLE: AtomicU64 = AtomicU64::new(0);
static ORIG_NEXT_DRAWABLE: AtomicUsize = AtomicUsize::new(0);
static HUD_CONFIGURED_KEY: u8 = 0;

static HUD_COMPLETED_DESC: GlobalBlockDescriptor = GlobalBlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<GlobalBlockLiteral>(),
};

extern "C" fn hud_completed_invoke(_block: *const c_void, _cmd_buf: Id) {
    IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
}

static HUD_COMPLETED_BLOCK: GlobalBlockLiteral = GlobalBlockLiteral {
    isa: unsafe { &_NSConcreteGlobalBlock as *const c_void },
    flags: 1 << 28, // BLOCK_IS_GLOBAL
    reserved: 0,
    invoke: hud_completed_invoke as *const c_void,
    descriptor: &HUD_COMPLETED_DESC,
};

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MTL_HUD_ENABLED")
            .ok()
            .is_some_and(|v| !v.trim().is_empty() && v.trim() != "0")
    })
}

fn now_ns() -> u64 {
    unsafe { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) }
}

unsafe fn configure_layer_hud(layer: Id) {
    if layer.is_null() || !assoc_get(layer, &HUD_CONFIGURED_KEY).is_null() {
        return;
    }
    assoc_set(layer, &HUD_CONFIGURED_KEY, nsnumber_bool(true));
    if responds_to(layer, "setDeveloperHUDProperties:") {
        let dict: Id = msg2(
            cls("NSDictionary"),
            "dictionaryWithObject:forKey:",
            nsstring("default"),
            nsstring("mode"),
        );
        msg1::<Id, ()>(layer, "setDeveloperHUDProperties:", dict);
    }
}

unsafe extern "C" fn hooked_next_drawable(self_obj: Id, cmd: Sel) -> Id {
    if enabled() && (self_obj as usize) != CARRIER.load(Ordering::Relaxed) {
        LAST_APP_DRAWABLE.store(now_ns(), Ordering::Relaxed);
        configure_layer_hud(self_obj);
    }
    let orig = ORIG_NEXT_DRAWABLE.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(self_obj, cmd)
    } else {
        NIL
    }
}

pub unsafe fn update_carrier_layout() {
    let carrier = CARRIER.load(Ordering::Acquire) as Id;
    if carrier.is_null() {
        return;
    }
    let (host_layer, scale, bounds) = super::primary_backing_layer_info();
    if host_layer.is_null() {
        return;
    }
    if msg0::<Id>(carrier, "superlayer") != host_layer {
        msg0::<()>(carrier, "removeFromSuperlayer");
        msg1::<Id, ()>(host_layer, "addSublayer:", carrier);
    }
    let drawable = CGSize {
        width: (bounds.size.width * scale).round(),
        height: (bounds.size.height * scale).round(),
    };
    let cur_frame: CGRect = msg0(carrier, "frame");
    let cur_drawable: CGSize = msg0(carrier, "drawableSize");
    if cur_frame != bounds || cur_drawable != drawable {
        let ca = cls("CATransaction");
        msg0::<()>(ca, "begin");
        msg1::<bool, ()>(ca, "setDisableActions:", true);
        msg1::<CGRect, ()>(carrier, "setFrame:", bounds);
        msg1::<f64, ()>(carrier, "setContentsScale:", scale);
        if drawable.width >= 1.0 && drawable.height >= 1.0 {
            msg1::<CGSize, ()>(carrier, "setDrawableSize:", drawable);
        }
        msg0::<()>(ca, "commit");
    }
}

extern "C" fn install_carrier_main(_ctx: *mut c_void) {
    unsafe {
        autorelease_pool(|| {
            if CARRIER.load(Ordering::Acquire) != 0 {
                return;
            }
            let (host_layer, _, _) = super::primary_backing_layer_info();
            let device = MTLCreateSystemDefaultDevice();
            if host_layer.is_null() || device.is_null() {
                INSTALLING.store(false, Ordering::Release);
                return;
            }
            let queue: Id = msg0(device, "newCommandQueue");
            if queue.is_null() {
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
            if responds_to(carrier, "setAllowsNextDrawableTimeout:") {
                msg1::<bool, ()>(carrier, "setAllowsNextDrawableTimeout:", true);
            }
            configure_layer_hud(carrier);
            QUEUE.store(queue as usize, Ordering::Release);
            CARRIER.store(carrier as usize, Ordering::Release);
            update_carrier_layout();
            super::log_msg("hud", "Metal HUD carrier installed for OpenGL ES presents");
        });
    }
}

#[no_mangle]
pub extern "C" fn PearRecordFramePresented() {
    if !enabled() {
        return;
    }
    // If another CAMetalLayer (e.g., ANGLE's mMetalLayer or MTKView) is actively
    // requesting drawables, keep the carrier idle so only one Metal HUD renders.
    if now_ns().saturating_sub(LAST_APP_DRAWABLE.load(Ordering::Relaxed)) < 300_000_000 {
        return;
    }
    let carrier = CARRIER.load(Ordering::Acquire) as Id;
    if carrier.is_null() {
        if !INSTALLING.swap(true, Ordering::AcqRel) {
            unsafe {
                dispatch_async_f(&_dispatch_main_q as *const c_void, std::ptr::null_mut(), install_carrier_main);
            }
        }
        return;
    }
    if IN_FLIGHT.fetch_add(1, Ordering::AcqRel) >= 2 {
        IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
        return;
    }
    unsafe {
        autorelease_pool(|| {
            let drawable: Id = msg0(carrier, "nextDrawable");
            let queue = QUEUE.load(Ordering::Acquire) as Id;
            let commands: Id = if drawable.is_null() || queue.is_null() {
                NIL
            } else {
                msg0(queue, "commandBuffer")
            };
            if commands.is_null() {
                IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
                return;
            }
            let pass: Id = msg0(cls("MTLRenderPassDescriptor"), "renderPassDescriptor");
            let color: Id = msg1(msg0::<Id>(pass, "colorAttachments"), "objectAtIndexedSubscript:", 0usize);
            msg1::<Id, ()>(color, "setTexture:", msg0(drawable, "texture"));
            msg1::<usize, ()>(color, "setLoadAction:", 2); // MTLLoadActionClear
            msg1::<usize, ()>(color, "setStoreAction:", 1); // MTLStoreActionStore
            msg1::<ClearColor, ()>(color, "setClearColor:", ClearColor { r: 0.0, g: 0.0, b: 0.0, a: 0.0 });
            let enc: Id = msg1(commands, "renderCommandEncoderWithDescriptor:", pass);
            msg0::<()>(enc, "endEncoding");
            msg1::<*const c_void, ()>(
                commands,
                "addCompletedHandler:",
                &HUD_COMPLETED_BLOCK as *const GlobalBlockLiteral as *const c_void,
            );
            msg1::<Id, ()>(commands, "presentDrawable:", drawable);
            msg0::<()>(commands, "commit");
        });
    }
}

pub unsafe fn install() {
    if !enabled() {
        return;
    }
    let layer_cls = cls("CAMetalLayer");
    if !layer_cls.is_null() {
        let m = class_getInstanceMethod(layer_cls, sel("nextDrawable"));
        if !m.is_null() {
            let prev = method_setImplementation(m, hooked_next_drawable as Imp);
            ORIG_NEXT_DRAWABLE.store(prev as usize, Ordering::Release);
        }
    }
    super::log_msg("hud", "Metal HUD enabled (MTL_HUD_ENABLED=1)");
}
