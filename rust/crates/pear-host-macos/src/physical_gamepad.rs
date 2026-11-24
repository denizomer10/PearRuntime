//! Bridges `pear-controller` (Switch 2 Joy-Cons via BLE, DualSense, Xbox,
//! Switch Pro/Joy-Con, generic HID) into PearHost's `GCController` layer.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use pear_controller::types::{ControllerConfig, ControllerDevice, ControllerState};
use pear_objc::{
    assoc_get, assoc_set, autorelease_pool, cls, msg0, msg1, msg2, nsarray_count,
    nsarray_object_at, nsstring, objc_release, objc_retain, replace_method, responds_to, sel, Class,
    Id, Imp, Sel, NIL,
};

extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn class_getClassMethod(cls: Class, name: Sel) -> *mut c_void;
    fn class_getInstanceMethod(cls: Class, name: Sel) -> *mut c_void;
    fn method_getImplementation(m: *mut c_void) -> Imp;
    fn method_setImplementation(m: *mut c_void, imp: Imp) -> Imp;
    fn object_getClass(obj: Id) -> Class;
}

const RTLD_LAZY: i32 = 1;

#[repr(C)]
struct ObjCBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
}

struct BridgedPad {
    controller: usize,
    last_state: ControllerState,
    name: String,
    category: String,
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
static PADS: OnceLock<Mutex<HashMap<String, BridgedPad>>> = OnceLock::new();
static ORIG_CONTROLLERS: AtomicUsize = AtomicUsize::new(0);
static ORIG_CURRENT: AtomicUsize = AtomicUsize::new(0);
static ORIG_VENDOR_NAME: AtomicUsize = AtomicUsize::new(0);
static ORIG_PRODUCT_CAT: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_SNAPSHOT: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_ATTACHED: AtomicUsize = AtomicUsize::new(0);
static CONNECT_NOTIF: AtomicUsize = AtomicUsize::new(0);
static DISCONNECT_NOTIF: AtomicUsize = AtomicUsize::new(0);
static PHYS_VENDOR_KEY: u8 = 0;
static PHYS_CAT_KEY: u8 = 0;
static PHYS_FLAG_KEY: u8 = 0;

fn pads() -> &'static Mutex<HashMap<String, BridgedPad>> {
    PADS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn extra_controllers() -> Vec<Id> {
    pads()
        .lock()
        .map(|m| m.values().map(|p| p.controller as Id).collect())
        .unwrap_or_default()
}

pub fn install() {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    if std::env::var("PEAR_DIRECT_CONTROLLER").ok().as_deref() == Some("0") {
        return;
    }

    let cfg = match std::env::var("PEAR_CONTROLLER_CONFIG") {
        Ok(raw) if !raw.trim().is_empty() => ControllerConfig::from_json(&raw),
        _ => ControllerConfig::default(),
    };

    unsafe {
        let gc_handle = dlopen(
            c"/System/Library/Frameworks/GameController.framework/GameController".as_ptr(),
            RTLD_LAZY,
        );
        if !gc_handle.is_null() {
            let c_sym = dlsym(gc_handle, c"GCControllerDidConnectNotification".as_ptr());
            if !c_sym.is_null() {
                let ptr = *(c_sym as *const usize);
                CONNECT_NOTIF.store(ptr, Ordering::Release);
            }
            let d_sym = dlsym(gc_handle, c"GCControllerDidDisconnectNotification".as_ptr());
            if !d_sym.is_null() {
                let ptr = *(d_sym as *const usize);
                DISCONNECT_NOTIF.store(ptr, Ordering::Release);
            }
        }
        install_gc_swizzles();
    }

    let store = pear_controller::start_with_config(cfg);
    store.add_listener(Arc::new(|devices| {
        let snapshot = devices.to_vec();
        let boxed = Box::into_raw(Box::new(snapshot)) as *mut c_void;
        unsafe {
            dispatch_async_f(
                &_dispatch_main_q as *const c_void,
                boxed,
                sync_devices_on_main,
            );
        }
    }));

    unsafe {
        super::log_msg("controller", "pure-Rust HID + Switch 2 BLE controller engine active");
    }
}

unsafe fn install_gc_swizzles() {
    let gc_cls = cls("GCController");
    if gc_cls.is_null() {
        return;
    }

    // Only swizzle controllers / current if virtual_gamepad hasn't already claimed them.
    if std::env::var("PEAR_VIRTUAL_GAMEPAD").ok().as_deref() != Some("1") {
        let m_ctrls = class_getClassMethod(gc_cls, sel("controllers"));
        if !m_ctrls.is_null() {
            let prev = method_setImplementation(m_ctrls, phys_gc_controllers as Imp);
            ORIG_CONTROLLERS.store(prev as usize, Ordering::Release);
        }
        let m_cur = class_getClassMethod(gc_cls, sel("current"));
        if !m_cur.is_null() {
            let prev = method_setImplementation(m_cur, phys_gc_current as Imp);
            ORIG_CURRENT.store(prev as usize, Ordering::Release);
        }
    }

    let m_snap = class_getInstanceMethod(gc_cls, sel("isSnapshot"));
    if !m_snap.is_null() {
        let cur = method_getImplementation(m_snap);
        if cur != phys_is_snapshot as Imp {
            let prev = method_setImplementation(m_snap, phys_is_snapshot as Imp);
            ORIG_IS_SNAPSHOT.store(prev as usize, Ordering::Release);
        }
    }
    let m_att = class_getInstanceMethod(gc_cls, sel("isAttachedToDevice"));
    if !m_att.is_null() {
        let cur = method_getImplementation(m_att);
        if cur != phys_is_attached as Imp {
            let prev = method_setImplementation(m_att, phys_is_attached as Imp);
            ORIG_IS_ATTACHED.store(prev as usize, Ordering::Release);
        }
    }
    let m_vn = class_getInstanceMethod(gc_cls, sel("vendorName"));
    if !m_vn.is_null() {
        let cur = method_getImplementation(m_vn);
        if cur != phys_vendor_name as Imp {
            let prev = method_setImplementation(m_vn, phys_vendor_name as Imp);
            ORIG_VENDOR_NAME.store(prev as usize, Ordering::Release);
        }
    }
    let m_pc = class_getInstanceMethod(gc_cls, sel("productCategory"));
    if !m_pc.is_null() {
        let cur = method_getImplementation(m_pc);
        if cur != phys_product_cat as Imp {
            let prev = method_setImplementation(m_pc, phys_product_cat as Imp);
            ORIG_PRODUCT_CAT.store(prev as usize, Ordering::Release);
        }
    }
    let _ = replace_method;
    let _ = object_getClass;
}

unsafe extern "C" fn phys_gc_controllers(self_cls: Id, _cmd: Sel) -> Id {
    let orig = ORIG_CONTROLLERS.load(Ordering::Acquire);
    let arr: Id = if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(self_cls, sel("controllers"))
    } else {
        NIL
    };
    let extras = extra_controllers();
    if extras.is_empty() {
        return if arr.is_null() { msg0(cls("NSArray"), "array") } else { arr };
    }
    let out: Id = if arr.is_null() {
        msg0(cls("NSMutableArray"), "array")
    } else {
        msg1(cls("NSMutableArray"), "arrayWithArray:", arr)
    };
    for pad in extras {
        if !pad.is_null() && !msg1::<Id, bool>(out, "containsObject:", pad) {
            msg1::<Id, ()>(out, "addObject:", pad);
        }
    }
    msg1(cls("NSArray"), "arrayWithArray:", out)
}

unsafe extern "C" fn phys_gc_current(self_cls: Id, _cmd: Sel) -> Id {
    let orig = ORIG_CURRENT.load(Ordering::Acquire);
    let cur: Id = if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(self_cls, sel("current"))
    } else {
        NIL
    };
    if !cur.is_null() {
        return cur;
    }
    extra_controllers().into_iter().next().unwrap_or(NIL)
}

unsafe extern "C" fn phys_is_snapshot(this: Id, _cmd: Sel) -> i8 {
    if !assoc_get(this, &PHYS_FLAG_KEY).is_null() {
        return 0;
    }
    let orig = ORIG_IS_SNAPSHOT.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> i8 = std::mem::transmute(orig);
        f(this, sel("isSnapshot"))
    } else {
        0
    }
}

unsafe extern "C" fn phys_is_attached(this: Id, _cmd: Sel) -> i8 {
    if !assoc_get(this, &PHYS_FLAG_KEY).is_null() {
        return 1;
    }
    let orig = ORIG_IS_ATTACHED.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> i8 = std::mem::transmute(orig);
        f(this, sel("isAttachedToDevice"))
    } else {
        1
    }
}

unsafe extern "C" fn phys_vendor_name(this: Id, _cmd: Sel) -> Id {
    let custom = assoc_get(this, &PHYS_VENDOR_KEY);
    if !custom.is_null() {
        return custom;
    }
    let orig = ORIG_VENDOR_NAME.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(this, sel("vendorName"))
    } else {
        NIL
    }
}

unsafe extern "C" fn phys_product_cat(this: Id, _cmd: Sel) -> Id {
    let custom = assoc_get(this, &PHYS_CAT_KEY);
    if !custom.is_null() {
        return custom;
    }
    let orig = ORIG_PRODUCT_CAT.load(Ordering::Acquire);
    if orig != 0 {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(orig);
        f(this, sel("productCategory"))
    } else {
        NIL
    }
}

extern "C" fn sync_devices_on_main(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let devices: Box<Vec<ControllerDevice>> = unsafe { Box::from_raw(ctx as *mut Vec<ControllerDevice>) };
    unsafe {
        autorelease_pool(|| {
            let gc_cls = cls("GCController");
            if gc_cls.is_null() {
                return;
            }
            let mut map = pads().lock().unwrap();
            let active_ids: std::collections::HashSet<&str> =
                devices.iter().map(|d| d.id.as_str()).collect();

            let removed: Vec<String> = map
                .keys()
                .filter(|k| !active_ids.contains(k.as_str()))
                .cloned()
                .collect();
            for id in removed {
                if let Some(old) = map.remove(&id) {
                    let ctrl = old.controller as Id;
                    post_notif(DISCONNECT_NOTIF.load(Ordering::Acquire) as Id, "GCControllerDidDisconnectNotification", ctrl);
                    objc_release(ctrl);
                }
            }

            for dev in devices.iter() {
                if !map.contains_key(&dev.id) {
                    let ctrl: Id = msg0(gc_cls, "controllerWithExtendedGamepad");
                    if ctrl.is_null() {
                        continue;
                    }
                    objc_retain(ctrl);
                    assoc_set(ctrl, &PHYS_FLAG_KEY, nsstring("1"));
                    assoc_set(ctrl, &PHYS_VENDOR_KEY, nsstring(&dev.name));
                    assoc_set(ctrl, &PHYS_CAT_KEY, nsstring(dev.kind.display_name()));
                    super::log_msg(
                        "controller",
                        &format!("connected {} ({} via {})", dev.name, dev.kind.display_name(), dev.transport.as_str()),
                    );
                    map.insert(
                        dev.id.clone(),
                        BridgedPad {
                            controller: ctrl as usize,
                            last_state: ControllerState::default(),
                            name: dev.name.clone(),
                            category: dev.kind.display_name().to_string(),
                        },
                    );
                    post_notif(CONNECT_NOTIF.load(Ordering::Acquire) as Id, "GCControllerDidConnectNotification", ctrl);
                }
                if let Some(entry) = map.get_mut(&dev.id) {
                    let ctrl = entry.controller as Id;
                    if entry.name != dev.name {
                        entry.name = dev.name.clone();
                        assoc_set(ctrl, &PHYS_VENDOR_KEY, nsstring(&dev.name));
                    }
                    if entry.category != dev.kind.display_name() {
                        entry.category = dev.kind.display_name().to_string();
                        assoc_set(ctrl, &PHYS_CAT_KEY, nsstring(dev.kind.display_name()));
                    }
                    apply_state(ctrl, &mut entry.last_state, &dev.state);
                }
            }
            let _ = nsarray_count;
            let _ = nsarray_object_at;
        });
    }
}

unsafe fn post_notif(notif_ptr: Id, fallback: &str, obj: Id) {
    if obj.is_null() {
        return;
    }
    let name = if notif_ptr.is_null() {
        nsstring(fallback)
    } else {
        notif_ptr
    };
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg2::<Id, Id, ()>(nc, "postNotificationName:object:", name, obj);
}

unsafe fn fire_btn_handlers(btn: Id, value: f32, pressed: bool) {
    let v_blk: Id = msg0(btn, "valueChangedHandler");
    if !v_blk.is_null() {
        let f: unsafe extern "C" fn(Id, Id, f32, i8) =
            std::mem::transmute((*(v_blk as *const ObjCBlock)).invoke);
        f(v_blk, btn, value, if pressed { 1 } else { 0 });
    }
    let p_blk: Id = msg0(btn, "pressedChangedHandler");
    if !p_blk.is_null() {
        let f: unsafe extern "C" fn(Id, Id, f32, i8) =
            std::mem::transmute((*(p_blk as *const ObjCBlock)).invoke);
        f(p_blk, btn, value, if pressed { 1 } else { 0 });
    }
}

unsafe fn fire_pad_handler(ext: Id, elem: Id) {
    let blk: Id = msg0(ext, "valueChangedHandler");
    if !blk.is_null() {
        let f: unsafe extern "C" fn(Id, Id, Id) =
            std::mem::transmute((*(blk as *const ObjCBlock)).invoke);
        f(blk, ext, elem);
    }
}

unsafe fn set_btn(ext: Id, sel_name: &str, prev: bool, next: bool) {
    if prev == next {
        return;
    }
    let btn: Id = msg0(ext, sel_name);
    if btn.is_null() {
        return;
    }
    let val = if next { 1.0f32 } else { 0.0f32 };
    if responds_to(btn, "setValue:") {
        msg1::<f32, ()>(btn, "setValue:", val);
    }
    fire_btn_handlers(btn, val, next);
    fire_pad_handler(ext, btn);
}

unsafe fn set_trigger(ext: Id, sel_name: &str, prev: f32, next: f32) {
    if (prev - next).abs() < 0.005 {
        return;
    }
    let btn: Id = msg0(ext, sel_name);
    if btn.is_null() {
        return;
    }
    if responds_to(btn, "setValue:") {
        msg1::<f32, ()>(btn, "setValue:", next);
    }
    fire_btn_handlers(btn, next, next > 0.1);
    fire_pad_handler(ext, btn);
}

unsafe fn set_stick(ext: Id, sel_name: &str, px: f32, py: f32, nx: f32, ny: f32) {
    if (px - nx).abs() < 0.004 && (py - ny).abs() < 0.004 {
        return;
    }
    let dpad: Id = msg0(ext, sel_name);
    if dpad.is_null() {
        return;
    }
    if responds_to(dpad, "setValueForXAxis:yAxis:") {
        msg2::<f32, f32, ()>(dpad, "setValueForXAxis:yAxis:", nx, ny);
    }
    let blk: Id = msg0(dpad, "valueChangedHandler");
    if !blk.is_null() {
        let f: unsafe extern "C" fn(Id, Id, f32, f32) =
            std::mem::transmute((*(blk as *const ObjCBlock)).invoke);
        f(blk, dpad, nx, ny);
    }
    fire_pad_handler(ext, dpad);
}

unsafe fn apply_state(ctrl: Id, prev: &mut ControllerState, next: &ControllerState) {
    let ext: Id = msg0(ctrl, "extendedGamepad");
    if ext.is_null() {
        return;
    }
    set_btn(ext, "buttonA", prev.a, next.a);
    set_btn(ext, "buttonB", prev.b, next.b);
    set_btn(ext, "buttonX", prev.x, next.x);
    set_btn(ext, "buttonY", prev.y, next.y);
    set_btn(ext, "leftShoulder", prev.l1, next.l1);
    set_btn(ext, "rightShoulder", prev.r1, next.r1);
    set_trigger(ext, "leftTrigger", prev.l2, next.l2);
    set_trigger(ext, "rightTrigger", prev.r2, next.r2);
    set_btn(ext, "leftThumbstickButton", prev.l3, next.l3);
    set_btn(ext, "rightThumbstickButton", prev.r3, next.r3);
    set_btn(ext, "buttonMenu", prev.menu, next.menu);
    set_btn(ext, "buttonOptions", prev.options, next.options);
    set_btn(ext, "buttonHome", prev.home, next.home);

    let pdx = (if prev.dpad_right { 1.0 } else { 0.0 }) - (if prev.dpad_left { 1.0 } else { 0.0 });
    let pdy = (if prev.dpad_up { 1.0 } else { 0.0 }) - (if prev.dpad_down { 1.0 } else { 0.0 });
    let ndx = (if next.dpad_right { 1.0 } else { 0.0 }) - (if next.dpad_left { 1.0 } else { 0.0 });
    let ndy = (if next.dpad_up { 1.0 } else { 0.0 }) - (if next.dpad_down { 1.0 } else { 0.0 });
    set_stick(ext, "dpad", pdx, pdy, ndx, ndy);
    set_stick(ext, "leftThumbstick", prev.left_x, prev.left_y, next.left_x, next.left_y);
    set_stick(ext, "rightThumbstick", prev.right_x, prev.right_y, next.right_x, next.right_y);

    *prev = *next;
    let _ = CStr::from_ptr;
}
