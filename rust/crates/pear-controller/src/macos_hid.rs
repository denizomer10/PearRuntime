//! Pure-Rust low-latency IOKit HID controller backend (DualSense, Xbox, Switch Pro/Joy-Con, Generic HID).

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::protocols::dualsense::{classify_sony_pid, parse_sony_report, SONY_VID};
use crate::protocols::switch1::{
    classify_switch1_pid, parse_switch1_report, switch1_full_mode_output_report,
};
use crate::protocols::switch2::NINTENDO_VID;
use crate::types::{
    apply_radial_deadzone, apply_trigger_deadzone, ControllerConfig, ControllerDevice,
    ControllerKind, ControllerState, TransportKind,
};
use crate::{now_us, SharedStore};

const MICROSOFT_VID: u16 = 0x045E;

type CFTypeRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFArrayRef = *const c_void;
type CFNumberRef = *const c_void;
type CFRunLoopRef = *const c_void;
type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOHIDElementRef = *mut c_void;
type IOHIDValueRef = *mut c_void;
type IOReturn = i32;
type CFIndex = isize;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT32_TYPE: CFIndex = 3;
const K_IO_RETURN_SUCCESS: IOReturn = 0;
const K_IO_HID_REPORT_TYPE_INPUT: u32 = 0;
const K_IO_HID_REPORT_TYPE_OUTPUT: u32 = 1;
const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFAllocatorDefault: CFAllocatorRef;
    static kCFRunLoopCommonModes: CFStringRef;
    static kCFRunLoopDefaultMode: CFStringRef;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    static kCFTypeArrayCallBacks: c_void;

    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCString(
        the_string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: u32,
    ) -> bool;
    fn CFNumberCreate(
        allocator: CFAllocatorRef,
        the_type: CFIndex,
        value_ptr: *const c_void,
    ) -> CFNumberRef;
    fn CFNumberGetValue(
        number: CFNumberRef,
        the_type: CFIndex,
        value_ptr: *mut c_void,
    ) -> bool;
    fn CFDictionaryCreate(
        allocator: CFAllocatorRef,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: CFIndex,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
    fn CFArrayCreate(
        allocator: CFAllocatorRef,
        values: *const *const c_void,
        num_values: CFIndex,
        callbacks: *const c_void,
    ) -> CFArrayRef;
    fn CFRelease(cf: CFTypeRef);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source_handled: bool)
        -> i32;
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: u32) -> IOHIDManagerRef;
    fn IOHIDManagerSetDeviceMatchingMultiple(manager: IOHIDManagerRef, multiple: CFArrayRef);
    fn IOHIDManagerRegisterDeviceMatchingCallback(
        manager: IOHIDManagerRef,
        callback: extern "C" fn(*mut c_void, IOReturn, *mut c_void, IOHIDDeviceRef),
        context: *mut c_void,
    );
    fn IOHIDManagerRegisterDeviceRemovalCallback(
        manager: IOHIDManagerRef,
        callback: extern "C" fn(*mut c_void, IOReturn, *mut c_void, IOHIDDeviceRef),
        context: *mut c_void,
    );
    fn IOHIDManagerRegisterInputValueCallback(
        manager: IOHIDManagerRef,
        callback: extern "C" fn(*mut c_void, IOReturn, *mut c_void, IOHIDValueRef),
        context: *mut c_void,
    );
    fn IOHIDManagerScheduleWithRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFStringRef,
    );
    fn IOHIDManagerUnscheduleFromRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFStringRef,
    );
    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: u32) -> IOReturn;
    fn IOHIDManagerClose(manager: IOHIDManagerRef, options: u32) -> IOReturn;
    fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;
    fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        report_length: CFIndex,
        callback: extern "C" fn(
            *mut c_void,
            IOReturn,
            *mut c_void,
            u32,
            u32,
            *mut u8,
            CFIndex,
        ),
        context: *mut c_void,
    );
    fn IOHIDDeviceSetReport(
        device: IOHIDDeviceRef,
        report_type: u32,
        report_id: CFIndex,
        report: *const u8,
        report_length: CFIndex,
    ) -> IOReturn;
    fn IOHIDValueGetElement(value: IOHIDValueRef) -> IOHIDElementRef;
    fn IOHIDValueGetIntegerValue(value: IOHIDValueRef) -> CFIndex;
    fn IOHIDElementGetDevice(element: IOHIDElementRef) -> IOHIDDeviceRef;
    fn IOHIDElementGetUsagePage(element: IOHIDElementRef) -> u32;
    fn IOHIDElementGetUsage(element: IOHIDElementRef) -> u32;
    fn IOHIDElementGetLogicalMin(element: IOHIDElementRef) -> CFIndex;
    fn IOHIDElementGetLogicalMax(element: IOHIDElementRef) -> CFIndex;
    fn IOHIDElementIsRelative(element: IOHIDElementRef) -> bool;
}

extern "C" {
    fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: c_int) -> c_int;
}

unsafe fn cf_str(s: &str) -> CFStringRef {
    let c = std::ffi::CString::new(s).unwrap();
    CFStringCreateWithCString(kCFAllocatorDefault, c.as_ptr(), K_CF_STRING_ENCODING_UTF8)
}

unsafe fn cf_num_i32(v: i32) -> CFNumberRef {
    CFNumberCreate(
        kCFAllocatorDefault,
        K_CF_NUMBER_SINT32_TYPE,
        &v as *const i32 as *const c_void,
    )
}

unsafe fn get_prop_i32(device: IOHIDDeviceRef, key: &str) -> Option<i32> {
    let k = cf_str(key);
    let val = IOHIDDeviceGetProperty(device, k);
    CFRelease(k);
    if val.is_null() {
        return None;
    }
    let mut out: i32 = 0;
    if CFNumberGetValue(val, K_CF_NUMBER_SINT32_TYPE, &mut out as *mut i32 as *mut c_void) {
        Some(out)
    } else {
        None
    }
}

unsafe fn get_prop_str(device: IOHIDDeviceRef, key: &str) -> Option<String> {
    let k = cf_str(key);
    let val = IOHIDDeviceGetProperty(device, k);
    CFRelease(k);
    if val.is_null() {
        return None;
    }
    let mut buf = [0i8; 256];
    if CFStringGetCString(val, buf.as_mut_ptr(), buf.len() as CFIndex, K_CF_STRING_ENCODING_UTF8) {
        Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    } else {
        None
    }
}

struct HidDeviceCtx {
    device_id: String,
    kind: ControllerKind,
    is_bluetooth: bool,
    raw_report_mode: bool,
    report_buf: Box<[u8; 256]>,
    last_report_us: u64,
    raw_lx: f32,
    raw_ly: f32,
    raw_rx: f32,
    raw_ry: f32,
    raw_l2: f32,
    raw_r2: f32,
    state: ControllerState,
}

struct HidContext {
    store: Arc<SharedStore>,
    devices: Mutex<HashMap<usize, Box<HidDeviceCtx>>>,
}

pub struct HidBackend {
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl HidBackend {
    pub fn start(store: Arc<SharedStore>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        let handle = thread::Builder::new()
            .name("pear-controller-hid".to_string())
            .spawn(move || unsafe {
                // Request USER_INTERACTIVE QoS for minimal HID delivery latency.
                let _ = pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
                run_hid_loop(store, flag);
            })
            .ok();
        Self {
            running,
            thread: handle,
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

impl Drop for HidBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe fn make_matching_dict(page: i32, usage: i32) -> CFDictionaryRef {
    let k1 = cf_str("DeviceUsagePage");
    let k2 = cf_str("DeviceUsage");
    let v1 = cf_num_i32(page);
    let v2 = cf_num_i32(usage);
    let keys = [k1 as *const c_void, k2 as *const c_void];
    let vals = [v1 as *const c_void, v2 as *const c_void];
    let dict = CFDictionaryCreate(
        kCFAllocatorDefault,
        keys.as_ptr(),
        vals.as_ptr(),
        2,
        &kCFTypeDictionaryKeyCallBacks,
        &kCFTypeDictionaryValueCallBacks,
    );
    CFRelease(k1);
    CFRelease(k2);
    CFRelease(v1);
    CFRelease(v2);
    dict
}

unsafe fn run_hid_loop(store: Arc<SharedStore>, running: Arc<AtomicBool>) {
    let manager = IOHIDManagerCreate(kCFAllocatorDefault, 0);
    if manager.is_null() {
        return;
    }

    let d1 = make_matching_dict(0x01, 0x05); // GenericDesktop / GamePad
    let d2 = make_matching_dict(0x01, 0x04); // GenericDesktop / Joystick
    let arr_vals = [d1 as *const c_void, d2 as *const c_void];
    let matching = CFArrayCreate(
        kCFAllocatorDefault,
        arr_vals.as_ptr(),
        2,
        &kCFTypeArrayCallBacks,
    );
    CFRelease(d1);
    CFRelease(d2);

    IOHIDManagerSetDeviceMatchingMultiple(manager, matching);
    CFRelease(matching);

    let ctx = Box::into_raw(Box::new(HidContext {
        store,
        devices: Mutex::new(HashMap::new()),
    }));

    IOHIDManagerRegisterDeviceMatchingCallback(manager, on_device_added, ctx as *mut c_void);
    IOHIDManagerRegisterDeviceRemovalCallback(manager, on_device_removed, ctx as *mut c_void);
    IOHIDManagerRegisterInputValueCallback(manager, on_input_value, ctx as *mut c_void);
    IOHIDManagerScheduleWithRunLoop(manager, CFRunLoopGetCurrent(), kCFRunLoopCommonModes);

    if IOHIDManagerOpen(manager, 0) == K_IO_RETURN_SUCCESS {
        while running.load(Ordering::Acquire) {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false);
        }
    }

    IOHIDManagerUnscheduleFromRunLoop(manager, CFRunLoopGetCurrent(), kCFRunLoopCommonModes);
    IOHIDManagerClose(manager, 0);
    CFRelease(manager);
    let _ = Box::from_raw(ctx);
}

fn classify_hid(vid: u16, pid: u16) -> ControllerKind {
    if vid == SONY_VID {
        if let Some(k) = classify_sony_pid(pid) {
            return k;
        }
    }
    if vid == NINTENDO_VID {
        if let Some(k) = classify_switch1_pid(pid) {
            return k;
        }
    }
    if vid == MICROSOFT_VID {
        return ControllerKind::Xbox;
    }
    ControllerKind::GenericHID
}

extern "C" fn on_device_added(
    context: *mut c_void,
    result: IOReturn,
    _sender: *mut c_void,
    device: IOHIDDeviceRef,
) {
    if result != K_IO_RETURN_SUCCESS || context.is_null() || device.is_null() {
        return;
    }
    unsafe {
        // Ignore Apple's synthetic GCController HID mirrors.
        if get_prop_i32(device, "GCSyntheticDevice").unwrap_or(0) != 0 {
            return;
        }
        let vid = get_prop_i32(device, "VendorID").unwrap_or(0) as u16;
        let pid = get_prop_i32(device, "ProductID").unwrap_or(0) as u16;
        let transport_str = get_prop_str(device, "Transport").unwrap_or_default();
        let is_bt = transport_str.to_ascii_lowercase().contains("bluetooth");
        let transport = if is_bt {
            TransportKind::BluetoothHid
        } else {
            TransportKind::UsbHid
        };

        let kind = classify_hid(vid, pid);
        let name = get_prop_str(device, "Product")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kind.display_name().to_string());

        let key = device as usize;
        let device_id = format!("hid:{:04x}:{:04x}:{:x}", vid, pid, key);

        let ctx = &*(context as *const HidContext);
        let prefer_raw = matches!(
            kind,
            ControllerKind::DualSense
                | ControllerKind::DualShock4
                | ControllerKind::Switch1Pro
                | ControllerKind::Switch1JoyConLeft
                | ControllerKind::Switch1JoyConRight
        );

        let mut dev_ctx = Box::new(HidDeviceCtx {
            device_id: device_id.clone(),
            kind,
            is_bluetooth: is_bt,
            raw_report_mode: false,
            report_buf: Box::new([0u8; 256]),
            last_report_us: now_us(),
            raw_lx: 0.0,
            raw_ly: 0.0,
            raw_rx: 0.0,
            raw_ry: 0.0,
            raw_l2: 0.0,
            raw_r2: 0.0,
            state: ControllerState::default(),
        });

        ctx.store.upsert_device(ControllerDevice {
            id: device_id,
            name,
            kind,
            transport,
            vendor_id: vid,
            product_id: pid,
            battery_percent: None,
            latency_us: 0,
            report_rate_hz: 0,
            connected: true,
            state: ControllerState::default(),
        });

        if prefer_raw {
            let ptr = &mut *dev_ctx as *mut HidDeviceCtx;
            let buf_ptr = dev_ctx.report_buf.as_mut_ptr();
            let buf_len = dev_ctx.report_buf.len() as CFIndex;
            IOHIDDeviceRegisterInputReportCallback(
                device,
                buf_ptr,
                buf_len,
                on_raw_report,
                context,
            );
            let _ = ptr;

            // Unlock Switch 1 60-120Hz full report mode (0x30).
            if matches!(
                kind,
                ControllerKind::Switch1Pro
                    | ControllerKind::Switch1JoyConLeft
                    | ControllerKind::Switch1JoyConRight
            ) {
                let cmd = switch1_full_mode_output_report(0);
                let _ = IOHIDDeviceSetReport(
                    device,
                    K_IO_HID_REPORT_TYPE_OUTPUT,
                    0x01,
                    cmd.as_ptr(),
                    cmd.len() as CFIndex,
                );
            }
        }

        if let Ok(mut map) = ctx.devices.lock() {
            map.insert(key, dev_ctx);
        }
    }
}

extern "C" fn on_device_removed(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    device: IOHIDDeviceRef,
) {
    if context.is_null() || device.is_null() {
        return;
    }
    unsafe {
        let ctx = &*(context as *const HidContext);
        let key = device as usize;
        if let Ok(mut map) = ctx.devices.lock() {
            if let Some(removed) = map.remove(&key) {
                ctx.store.remove_device(&removed.device_id);
            }
        }
    }
}

extern "C" fn on_raw_report(
    context: *mut c_void,
    result: IOReturn,
    sender: *mut c_void,
    report_type: u32,
    _report_id: u32,
    report: *mut u8,
    report_length: CFIndex,
) {
    if result != K_IO_RETURN_SUCCESS
        || report_type != K_IO_HID_REPORT_TYPE_INPUT
        || context.is_null()
        || sender.is_null()
        || report.is_null()
        || report_length <= 0
    {
        return;
    }
    unsafe {
        let bytes = std::slice::from_raw_parts(report, report_length as usize);
        let ctx = &*(context as *const HidContext);
        let cfg = ctx.store.config();
        let key = sender as usize;
        let now = now_us();

        let mut update: Option<(String, ControllerState, Option<u8>, u64, u32)> = None;
        if let Ok(mut map) = ctx.devices.lock() {
            if let Some(dev) = map.get_mut(&key) {
                let parsed = match dev.kind {
                    ControllerKind::DualSense | ControllerKind::DualShock4 => {
                        parse_sony_report(dev.kind, bytes, &cfg).map(|s| (s, None))
                    }
                    ControllerKind::Switch1Pro
                    | ControllerKind::Switch1JoyConLeft
                    | ControllerKind::Switch1JoyConRight => {
                        parse_switch1_report(dev.kind, bytes, &cfg)
                    }
                    _ => None,
                };
                if let Some((state, battery)) = parsed {
                    dev.raw_report_mode = true;
                    dev.state = state;
                    let dt = now.saturating_sub(dev.last_report_us);
                    dev.last_report_us = now;
                    let hz = if dt > 500 && dt < 250_000 {
                        (1_000_000 / dt) as u32
                    } else {
                        0
                    };
                    let _ = dev.is_bluetooth;
                    update = Some((dev.device_id.clone(), state, battery, dt, hz));
                }
            }
        }
        if let Some((id, state, battery, dt, hz)) = update {
            ctx.store.update_state(&id, state, battery, dt, hz);
        }
    }
}

extern "C" fn on_input_value(
    context: *mut c_void,
    result: IOReturn,
    _sender: *mut c_void,
    value: IOHIDValueRef,
) {
    if result != K_IO_RETURN_SUCCESS || context.is_null() || value.is_null() {
        return;
    }
    unsafe {
        let elem = IOHIDValueGetElement(value);
        if elem.is_null() || IOHIDElementIsRelative(elem) {
            return;
        }
        let device = IOHIDElementGetDevice(elem);
        if device.is_null() {
            return;
        }

        let ctx = &*(context as *const HidContext);
        let cfg = ctx.store.config();
        let key = device as usize;
        let page = IOHIDElementGetUsagePage(elem);
        let usage = IOHIDElementGetUsage(elem);
        let raw_val = IOHIDValueGetIntegerValue(value);
        let min_val = IOHIDElementGetLogicalMin(elem);
        let max_val = IOHIDElementGetLogicalMax(elem);
        let now = now_us();

        let mut update: Option<(String, ControllerState, u64, u32)> = None;
        if let Ok(mut map) = ctx.devices.lock() {
            if let Some(dev) = map.get_mut(&key) {
                if dev.raw_report_mode {
                    return;
                }
                let changed = apply_hid_element(dev, page, usage, raw_val, min_val, max_val, &cfg);
                if changed {
                    let dt = now.saturating_sub(dev.last_report_us);
                    dev.last_report_us = now;
                    let hz = if dt > 500 && dt < 250_000 {
                        (1_000_000 / dt) as u32
                    } else {
                        0
                    };
                    let mut st = dev.state;
                    if dev.kind.is_nintendo() {
                        st.apply_nintendo_layout(cfg.nintendo_button_layout);
                    }
                    update = Some((dev.device_id.clone(), st, dt, hz));
                }
            }
        }
        if let Some((id, st, dt, hz)) = update {
            ctx.store.update_state(&id, st, None, dt, hz);
        }
    }
}

fn norm_01(val: isize, min: isize, max: isize) -> f32 {
    if max <= min {
        return 0.0;
    }
    (((val - min) as f32) / ((max - min) as f32)).clamp(0.0, 1.0)
}

fn norm_signed(val: isize, min: isize, max: isize) -> f32 {
    (2.0 * norm_01(val, min, max) - 1.0).clamp(-1.0, 1.0)
}

fn apply_hid_element(
    dev: &mut HidDeviceCtx,
    page: u32,
    usage: u32,
    val: isize,
    min: isize,
    max: isize,
    cfg: &ControllerConfig,
) -> bool {
    if page == 0x09 {
        // Button page
        let pressed = val != 0;
        match usage {
            1 => dev.state.a = pressed,
            2 => dev.state.b = pressed,
            3 => dev.state.x = pressed,
            4 => dev.state.y = pressed,
            5 => dev.state.l1 = pressed,
            6 => dev.state.r1 = pressed,
            7 => dev.state.l2 = if pressed { 1.0 } else { 0.0 },
            8 => dev.state.r2 = if pressed { 1.0 } else { 0.0 },
            9 => dev.state.options = pressed,
            10 => dev.state.menu = pressed,
            11 => dev.state.l3 = pressed,
            12 => dev.state.r3 = pressed,
            13 => dev.state.home = pressed,
            14 => dev.state.capture = pressed,
            _ => return false,
        }
        return true;
    }

    if page == 0x01 {
        // GenericDesktop
        match usage {
            0x39 => {
                // Hat switch
                let pos = val - min;
                let (u, d, l, r) = if pos >= 0 && pos < 8 && val <= max {
                    match pos {
                        0 => (true, false, false, false),
                        1 => (true, false, false, true),
                        2 => (false, false, false, true),
                        3 => (false, true, false, true),
                        4 => (false, true, false, false),
                        5 => (false, true, true, false),
                        6 => (false, false, true, false),
                        7 => (true, false, true, false),
                        _ => (false, false, false, false),
                    }
                } else {
                    (false, false, false, false)
                };
                dev.state.dpad_up = u;
                dev.state.dpad_down = d;
                dev.state.dpad_left = l;
                dev.state.dpad_right = r;
                return true;
            }
            0x30 => {
                dev.raw_lx = norm_signed(val, min, max);
                let (lx, ly) = apply_radial_deadzone(dev.raw_lx, dev.raw_ly, cfg.left_deadzone);
                dev.state.left_x = lx;
                dev.state.left_y = ly;
                return true;
            }
            0x31 => {
                dev.raw_ly = -norm_signed(val, min, max);
                let (lx, ly) = apply_radial_deadzone(dev.raw_lx, dev.raw_ly, cfg.left_deadzone);
                dev.state.left_x = lx;
                dev.state.left_y = ly;
                return true;
            }
            0x32 => {
                if dev.kind == ControllerKind::Xbox {
                    dev.raw_rx = norm_signed(val, min, max);
                    let (rx, ry) =
                        apply_radial_deadzone(dev.raw_rx, dev.raw_ry, cfg.right_deadzone);
                    dev.state.right_x = rx;
                    dev.state.right_y = ry;
                } else {
                    dev.raw_l2 = norm_01(val, min, max);
                    dev.state.l2 = apply_trigger_deadzone(dev.raw_l2, cfg.trigger_deadzone);
                }
                return true;
            }
            0x33 => {
                if dev.kind == ControllerKind::Xbox {
                    dev.raw_l2 = norm_01(val, min, max);
                    dev.state.l2 = apply_trigger_deadzone(dev.raw_l2, cfg.trigger_deadzone);
                } else {
                    dev.raw_rx = norm_signed(val, min, max);
                    let (rx, ry) =
                        apply_radial_deadzone(dev.raw_rx, dev.raw_ry, cfg.right_deadzone);
                    dev.state.right_x = rx;
                    dev.state.right_y = ry;
                }
                return true;
            }
            0x34 => {
                if dev.kind == ControllerKind::Xbox {
                    dev.raw_r2 = norm_01(val, min, max);
                    dev.state.r2 = apply_trigger_deadzone(dev.raw_r2, cfg.trigger_deadzone);
                } else {
                    dev.raw_ry = -norm_signed(val, min, max);
                    let (rx, ry) =
                        apply_radial_deadzone(dev.raw_rx, dev.raw_ry, cfg.right_deadzone);
                    dev.state.right_x = rx;
                    dev.state.right_y = ry;
                }
                return true;
            }
            0x35 => {
                if dev.kind == ControllerKind::Xbox {
                    dev.raw_ry = -norm_signed(val, min, max);
                    let (rx, ry) =
                        apply_radial_deadzone(dev.raw_rx, dev.raw_ry, cfg.right_deadzone);
                    dev.state.right_x = rx;
                    dev.state.right_y = ry;
                } else {
                    dev.raw_r2 = norm_01(val, min, max);
                    dev.state.r2 = apply_trigger_deadzone(dev.raw_r2, cfg.trigger_deadzone);
                }
                return true;
            }
            0x90 => {
                dev.state.dpad_up = val != 0;
                return true;
            }
            0x91 => {
                dev.state.dpad_down = val != 0;
                return true;
            }
            0x92 => {
                dev.state.dpad_right = val != 0;
                return true;
            }
            0x93 => {
                dev.state.dpad_left = val != 0;
                return true;
            }
            _ => {}
        }
    }

    if page == 0x02 && dev.kind == ControllerKind::Xbox {
        // Simulation Controls page: Brake (0xC5) = L2, Accelerator (0xC4) = R2 on Xbox BT
        if usage == 0xC5 {
            dev.raw_l2 = norm_01(val, min, max);
            dev.state.l2 = apply_trigger_deadzone(dev.raw_l2, cfg.trigger_deadzone);
            return true;
        }
        if usage == 0xC4 {
            dev.raw_r2 = norm_01(val, min, max);
            dev.state.r2 = apply_trigger_deadzone(dev.raw_r2, cfg.trigger_deadzone);
            return true;
        }
    }

    let _ = Duration::ZERO;
    let _ = Instant::now;
    false
}
