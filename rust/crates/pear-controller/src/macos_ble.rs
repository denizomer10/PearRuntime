//! Pure-Rust CoreBluetooth BLE backend for Nintendo Switch 2 Joy-Cons & Pro Controller 2.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use pear_objc::{
    add_method, cls, msg0, msg1, msg2, msg3, nsnumber_bool, nsstring, objc_release, objc_retain,
    Class, Id, Imp, Sel,
};

use crate::protocols::switch2::{
    classify_by_char_uuid, classify_by_name, is_switch2_ctrl_char, is_switch2_input_char,
    parse_manufacturer_data, parse_switch2_ble_report, switch2_keepalive_packet,
    switch2_optical_init_packets, switch2_player_led_packet, Switch2StickCal, NINTENDO_VID,
    PID_JOYCON2_LEFT, PID_JOYCON2_RIGHT, PID_PRO2, SWITCH2_SERVICE_UUID,
};
use crate::types::{ControllerDevice, ControllerKind, ControllerState, TransportKind};
use crate::{now_us, SharedStore};

extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra_bytes: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn dispatch_queue_attr_make_with_qos_class(
        attr: *const c_void,
        qos_class: u32,
        relative_priority: i32,
    ) -> *const c_void;
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *const c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint) -> i32;
    fn CGPreflightPostEventAccess() -> bool;
    fn CGEventCreateMouseEvent(
        source: *const c_void,
        mouse_type: u32,
        mouse_cursor_position: CGPoint,
        mouse_button: u32,
    ) -> *mut c_void;
    fn CGEventSetIntegerValueField(event: *mut c_void, field: u32, value: i64);
    fn CGEventPost(tap: u32, event: *mut c_void);
    fn CFRelease(cf: *const c_void);
}

const RTLD_LAZY: i32 = 1;
const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;
const CB_MANAGER_STATE_POWERED_ON: isize = 5;
const CB_CHAR_PROP_WRITE_WITHOUT_RESPONSE: usize = 0x04;
const CB_CHAR_WRITE_WITH_RESPONSE: usize = 0;
const CB_CHAR_WRITE_WITHOUT_RESPONSE: usize = 1;

const K_CG_HID_EVENT_TAP: u32 = 0;
const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const K_CG_MOUSE_EVENT_DELTA_X: u32 = 4;
const K_CG_MOUSE_EVENT_DELTA_Y: u32 = 5;

struct BlePeripheralEntry {
    device_id: String,
    peripheral: usize,
    cmd_char: Option<usize>,
    cmd_write_type: usize,
    kind: ControllerKind,
    pid: u16,
    name: String,
    last_report_us: u64,
    keepalive_seq: u8,
    raw_log_count: u8,
    cal: Switch2StickCal,
    opt_rem_x: f32,
    opt_rem_y: f32,
    prev_left_click: bool,
    prev_right_click: bool,
}

struct BleRuntimeState {
    store: Arc<SharedStore>,
    central: usize,
    delegate: usize,
    queue: usize,
    scanning_wanted: bool,
    peripherals: HashMap<usize, BlePeripheralEntry>,
}

static BLE_STATE: OnceLock<Mutex<Option<BleRuntimeState>>> = OnceLock::new();

fn ble_state() -> &'static Mutex<Option<BleRuntimeState>> {
    BLE_STATE.get_or_init(|| Mutex::new(None))
}

unsafe fn nsstring_to_rust(s: Id) -> String {
    if s.is_null() {
        return String::new();
    }
    let ptr: *const c_char = msg0(s, "UTF8String");
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

unsafe fn peripheral_uuid_string(peripheral: Id) -> String {
    let uuid: Id = msg0(peripheral, "identifier");
    if uuid.is_null() {
        return format!("{:x}", peripheral as usize);
    }
    let s: Id = msg0(uuid, "UUIDString");
    nsstring_to_rust(s)
}

unsafe fn write_ctrl_bytes(peripheral: Id, ch: Id, write_type: usize, bytes: &[u8]) {
    if peripheral.is_null() || ch.is_null() {
        return;
    }
    let data: Id = msg2(
        cls("NSData"),
        "dataWithBytes:length:",
        bytes.as_ptr() as *const c_void,
        bytes.len(),
    );
    let _: () = msg3(
        peripheral,
        "writeValue:forCharacteristic:type:",
        data,
        ch,
        write_type,
    );
}

unsafe fn emit_mouse_delta(dx: i32, dy: i32, left_down: bool) {
    let ev = CGEventCreate(std::ptr::null());
    if ev.is_null() {
        return;
    }
    let pos = CGEventGetLocation(ev);
    CFRelease(ev);
    let new_pos = CGPoint {
        x: (pos.x + dx as f64).max(0.0),
        y: (pos.y + dy as f64).max(0.0),
    };
    if CGPreflightPostEventAccess() {
        let ev_type = if left_down {
            K_CG_EVENT_LEFT_MOUSE_DRAGGED
        } else {
            K_CG_EVENT_MOUSE_MOVED
        };
        let move_ev = CGEventCreateMouseEvent(std::ptr::null(), ev_type, new_pos, 0);
        if !move_ev.is_null() {
            CGEventSetIntegerValueField(move_ev, K_CG_MOUSE_EVENT_DELTA_X, dx as i64);
            CGEventSetIntegerValueField(move_ev, K_CG_MOUSE_EVENT_DELTA_Y, dy as i64);
            CGEventPost(K_CG_HID_EVENT_TAP, move_ev);
            CFRelease(move_ev);
        }
    } else {
        // Without Accessibility permission, warp cursor cleanly without fighting CGEventPost.
        let _ = CGWarpMouseCursorPosition(new_pos);
    }
}

unsafe fn emit_mouse_button(button: u32, down: bool) {
    let ev = CGEventCreate(std::ptr::null());
    if ev.is_null() {
        return;
    }
    let pos = CGEventGetLocation(ev);
    CFRelease(ev);
    let ev_type = match (button, down) {
        (0, true) => K_CG_EVENT_LEFT_MOUSE_DOWN,
        (0, false) => K_CG_EVENT_LEFT_MOUSE_UP,
        (_, true) => K_CG_EVENT_RIGHT_MOUSE_DOWN,
        (_, false) => K_CG_EVENT_RIGHT_MOUSE_UP,
    };
    let btn_ev = CGEventCreateMouseEvent(std::ptr::null(), ev_type, pos, button);
    if !btn_ev.is_null() {
        CGEventPost(K_CG_HID_EVENT_TAP, btn_ev);
        CFRelease(btn_ev);
    }
}

unsafe fn register_ble_delegate_class() -> Class {
    static DELEGATE_CLS: OnceLock<usize> = OnceLock::new();
    let ptr = *DELEGATE_CLS.get_or_init(|| {
        let existing = cls("PearBleControllerDelegate");
        if !existing.is_null() {
            return existing as usize;
        }
        let nsobj = cls("NSObject");
        let c = objc_allocateClassPair(nsobj, c"PearBleControllerDelegate".as_ptr(), 0);
        add_method(c, "centralManagerDidUpdateState:", on_central_state as Imp, "v@:@");
        add_method(
            c,
            "centralManager:didDiscoverPeripheral:advertisementData:RSSI:",
            on_discover_peripheral as Imp,
            "v@:@@@@",
        );
        add_method(
            c,
            "centralManager:didConnectPeripheral:",
            on_connect_peripheral as Imp,
            "v@:@@",
        );
        add_method(
            c,
            "centralManager:didDisconnectPeripheral:error:",
            on_disconnect_peripheral as Imp,
            "v@:@@@",
        );
        add_method(c, "peripheral:didDiscoverServices:", on_discover_services as Imp, "v@:@@");
        add_method(
            c,
            "peripheral:didDiscoverCharacteristicsForService:error:",
            on_discover_characteristics as Imp,
            "v@:@@@",
        );
        add_method(
            c,
            "peripheral:didUpdateValueForCharacteristic:error:",
            on_characteristic_value as Imp,
            "v@:@@@",
        );
        objc_registerClassPair(c);
        c as usize
    });
    ptr as Class
}

extern "C" fn on_central_state(_self: Id, _cmd: Sel, central: Id) {
    unsafe {
        let state: isize = msg0(central, "state");
        if state == CB_MANAGER_STATE_POWERED_ON {
            let should_scan = ble_state()
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|s| s.scanning_wanted))
                .unwrap_or(false);
            if should_scan {
                start_scan_internal(central);
            }
        }
    }
}

unsafe fn start_scan_internal(central: Id) {
    if central.is_null() {
        return;
    }
    let state: isize = msg0(central, "state");
    if state != CB_MANAGER_STATE_POWERED_ON {
        return;
    }
    let allow_dup = nsnumber_bool(false);
    let key = nsstring("kCBScanOptionAllowDuplicates");
    let opts: Id = msg2(cls("NSDictionary"), "dictionaryWithObject:forKey:", allow_dup, key);
    let _: () = msg2(
        central,
        "scanForPeripheralsWithServices:options:",
        std::ptr::null::<c_void>() as Id,
        opts,
    );
}

extern "C" fn on_discover_peripheral(
    _self: Id,
    _cmd: Sel,
    central: Id,
    peripheral: Id,
    adv: Id,
    _rssi: Id,
) {
    if peripheral.is_null() || adv.is_null() {
        return;
    }
    unsafe {
        let pname: Id = msg0(peripheral, "name");
        let name_str = nsstring_to_rust(pname);

        let svc_key = nsstring("kCBAdvDataServiceUUIDs");
        let svc_uuids: Id = msg1(adv, "objectForKey:", svc_key);
        let mut matched_switch2 = false;
        if !svc_uuids.is_null() {
            let count: usize = msg0(svc_uuids, "count");
            for i in 0..count {
                let u: Id = msg1(svc_uuids, "objectAtIndex:", i);
                let ustr: Id = msg0(u, "UUIDString");
                if nsstring_to_rust(ustr).eq_ignore_ascii_case(SWITCH2_SERVICE_UUID) {
                    matched_switch2 = true;
                    break;
                }
            }
        }

        let mfg_key = nsstring("kCBAdvDataManufacturerData");
        let mfg_data: Id = msg1(adv, "objectForKey:", mfg_key);
        let mut detected: Option<(u16, ControllerKind)> = None;
        if !mfg_data.is_null() {
            let len: usize = msg0(mfg_data, "length");
            let bytes_ptr: *const u8 = msg0(mfg_data, "bytes");
            if !bytes_ptr.is_null() && len > 0 {
                let slice = std::slice::from_raw_parts(bytes_ptr, len);
                detected = parse_manufacturer_data(slice);
                if detected.is_some() {
                    matched_switch2 = true;
                }
                if matched_switch2 {
                    let mhex: Vec<String> = slice.iter().map(|b| format!("{:02X}", b)).collect();
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/pear-jc2-raw.log") {
                        use std::io::Write;
                        let _ = writeln!(f, "ADV name={} detected={:?} mfg={}", name_str, detected, mhex.join(" "));
                    }
                }
            }
        }

        if detected.is_none() {
            if let Some(k) = classify_by_name(&name_str) {
                let pid = match k {
                    ControllerKind::Switch2JoyConRight => PID_JOYCON2_RIGHT,
                    ControllerKind::Switch2JoyConLeft => PID_JOYCON2_LEFT,
                    _ => PID_PRO2,
                };
                detected = Some((pid, k));
                matched_switch2 = true;
            }
        }

        if !matched_switch2 {
            return;
        }

        let (pid, kind) = detected.unwrap_or((PID_JOYCON2_RIGHT, ControllerKind::Switch2JoyConRight));
        let p_key = peripheral as usize;
        let uuid_str = peripheral_uuid_string(peripheral);
        let display_name = if name_str.is_empty() {
            kind.display_name().to_string()
        } else {
            name_str
        };

        let should_connect = if let Ok(mut guard) = ble_state().lock() {
            if let Some(st) = guard.as_mut() {
                if st.peripherals.contains_key(&p_key) {
                    false
                } else {
                    objc_retain(peripheral);
                    st.peripherals.insert(
                        p_key,
                        BlePeripheralEntry {
                            device_id: format!("ble:{}", uuid_str),
                            peripheral: p_key,
                            cmd_char: None,
                            cmd_write_type: CB_CHAR_WRITE_WITHOUT_RESPONSE,
                            kind,
                            pid,
                            name: display_name,
                            last_report_us: now_us(),
                            keepalive_seq: 0,
                            raw_log_count: 0,
                            cal: Switch2StickCal::default(),
                            opt_rem_x: 0.0,
                            opt_rem_y: 0.0,
                            prev_left_click: false,
                            prev_right_click: false,
                        },
                    );
                    true
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_connect {
            let _: () = msg2(
                central,
                "connectPeripheral:options:",
                peripheral,
                std::ptr::null::<c_void>() as Id,
            );
        }
    }
}

extern "C" fn on_connect_peripheral(
    _self: Id,
    _cmd: Sel,
    _central: Id,
    peripheral: Id,
) {
    if peripheral.is_null() {
        return;
    }
    unsafe {
        let p_key = peripheral as usize;
        if let Ok(guard) = ble_state().lock() {
            if let Some(st) = guard.as_ref() {
                if let Some(entry) = st.peripherals.get(&p_key) {
                    let enabled = st.store.config().is_device_enabled(&entry.device_id);
                    st.store.upsert_device(ControllerDevice {
                        id: entry.device_id.clone(),
                        name: entry.name.clone(),
                        kind: entry.kind,
                        transport: TransportKind::Ble,
                        vendor_id: NINTENDO_VID,
                        product_id: entry.pid,
                        battery_percent: None,
                        latency_us: 0,
                        report_rate_hz: 60,
                        connected: true,
                        enabled,
                        state: ControllerState::default(),
                    });
                }
            }
        }

        let delegate: Id = ble_state()
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.delegate as Id))
            .unwrap_or(std::ptr::null_mut());
        if !delegate.is_null() {
            let _: () = msg1(peripheral, "setDelegate:", delegate);
        }
        // Discover ALL services so we find both the input notify service AND the control write service.
        let _: () = msg1(
            peripheral,
            "discoverServices:",
            std::ptr::null::<c_void>() as Id,
        );
    }
}

extern "C" fn on_disconnect_peripheral(
    _self: Id,
    _cmd: Sel,
    _central: Id,
    peripheral: Id,
    _error: Id,
) {
    if peripheral.is_null() {
        return;
    }
    unsafe {
        let p_key = peripheral as usize;
        if let Ok(mut guard) = ble_state().lock() {
            if let Some(st) = guard.as_mut() {
                if let Some(entry) = st.peripherals.remove(&p_key) {
                    if let Some(c) = entry.cmd_char {
                        objc_release(c as Id);
                    }
                    st.store.remove_device(&entry.device_id);
                }
            }
        }
        objc_release(peripheral);
    }
}

extern "C" fn on_discover_services(
    _self: Id,
    _cmd: Sel,
    peripheral: Id,
    _error: Id,
) {
    if peripheral.is_null() {
        return;
    }
    unsafe {
        let services: Id = msg0(peripheral, "services");
        if services.is_null() {
            return;
        }
        let count: usize = msg0(services, "count");
        for i in 0..count {
            let svc: Id = msg1(services, "objectAtIndex:", i);
            let _: () = msg2(
                peripheral,
                "discoverCharacteristics:forService:",
                std::ptr::null::<c_void>() as Id,
                svc,
            );
        }
    }
}

extern "C" fn on_discover_characteristics(
    _self: Id,
    _cmd: Sel,
    peripheral: Id,
    service: Id,
    _error: Id,
) {
    if peripheral.is_null() || service.is_null() {
        return;
    }
    unsafe {
        let chars: Id = msg0(service, "characteristics");
        if chars.is_null() {
            return;
        }
        let p_key = peripheral as usize;
        let count: usize = msg0(chars, "count");
        for i in 0..count {
            let ch: Id = msg1(chars, "objectAtIndex:", i);
            let cuuid: Id = msg0(ch, "UUID");
            let ustr_id: Id = msg0(cuuid, "UUIDString");
            let uuid = nsstring_to_rust(ustr_id);

            if is_switch2_input_char(&uuid) {
                if let Some(exact_kind) = classify_by_char_uuid(&uuid) {
                    if let Ok(mut guard) = ble_state().lock() {
                        if let Some(st) = guard.as_mut() {
                            if let Some(entry) = st.peripherals.get_mut(&p_key) {
                                entry.kind = exact_kind;
                                st.store.update_kind(&entry.device_id, exact_kind);
                            }
                        }
                    }
                }
                let _: () = msg2(peripheral, "setNotifyValue:forCharacteristic:", 1i8, ch);
            } else if is_switch2_ctrl_char(&uuid) {
                let props: usize = msg0(ch, "properties");
                let write_type = if (props & CB_CHAR_PROP_WRITE_WITHOUT_RESPONSE) != 0 {
                    CB_CHAR_WRITE_WITHOUT_RESPONSE
                } else {
                    CB_CHAR_WRITE_WITH_RESPONSE
                };
                if let Ok(mut guard) = ble_state().lock() {
                    if let Some(st) = guard.as_mut() {
                        if let Some(entry) = st.peripherals.get_mut(&p_key) {
                            if entry.cmd_char.is_none() || uuid.eq_ignore_ascii_case("649D4AC9-8EB7-4E6C-AF44-1EA54FE5F005") {
                                if let Some(old) = entry.cmd_char.take() {
                                    objc_release(old as Id);
                                }
                                objc_retain(ch);
                                entry.cmd_char = Some(ch as usize);
                                entry.cmd_write_type = write_type;
                            }
                        }
                    }
                }
                // Stop blinking & lock Player 1 LED solid (BlueRetro 0x09 0x91 0x01 0x07).
                let led_pkt = switch2_player_led_packet(1);
                write_ctrl_bytes(peripheral, ch, write_type, &led_pkt);
                let mouse_on = ble_state()
                    .lock()
                    .ok()
                    .and_then(|g| g.as_ref().map(|s| s.store.config().mouse_mode_enabled))
                    .unwrap_or(false);
                if mouse_on {
                    for opt_pkt in switch2_optical_init_packets() {
                        write_ctrl_bytes(peripheral, ch, write_type, &opt_pkt);
                    }
                }
            }
        }
    }
}

extern "C" fn on_characteristic_value(
    _self: Id,
    _cmd: Sel,
    peripheral: Id,
    characteristic: Id,
    error: Id,
) {
    if peripheral.is_null() || characteristic.is_null() || !error.is_null() {
        return;
    }
    unsafe {
        let val: Id = msg0(characteristic, "value");
        if val.is_null() {
            return;
        }
        let len: usize = msg0(val, "length");
        let ptr: *const u8 = msg0(val, "bytes");
        if ptr.is_null() || len == 0 {
            return;
        }
        let bytes = std::slice::from_raw_parts(ptr, len);
        let p_key = peripheral as usize;
        let now = now_us();

        let update = if let Ok(mut guard) = ble_state().lock() {
            if let Some(st) = guard.as_mut() {
                let cfg = st.store.config();
                if let Some(entry) = st.peripherals.get_mut(&p_key) {
                    if entry.raw_log_count < 15 {
                        entry.raw_log_count += 1;
                        let hex: Vec<String> = bytes.iter().take(24).map(|b| format!("{:02X}", b)).collect();
                        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/pear-jc2-raw.log") {
                            use std::io::Write;
                            let _ = writeln!(f, "kind={:?} len={} hex={}", entry.kind, bytes.len(), hex.join(" "));
                        }
                    }
                    if !cfg.is_device_enabled(&entry.device_id) {
                        None
                    } else if let Some((state, (opt_dx, opt_dy))) =
                        parse_switch2_ble_report(entry.kind, bytes, &cfg, &mut entry.cal)
                    {
                        let dt = now.saturating_sub(entry.last_report_us);
                        entry.last_report_us = now;
                        let hz = if dt > 500 && dt < 250_000 {
                            (1_000_000 / dt) as u32
                        } else {
                            60
                        };

                        if cfg.mouse_mode_enabled {
                            let sens = cfg.mouse_sensitivity;
                            let left_click = state.r1 || state.l1;
                            let right_click = state.r2 > 0.5 || state.l2 > 0.5;
                            if left_click != entry.prev_left_click {
                                entry.prev_left_click = left_click;
                                emit_mouse_button(0, left_click);
                            }
                            if right_click != entry.prev_right_click {
                                entry.prev_right_click = right_click;
                                emit_mouse_button(1, right_click);
                            }
                            if opt_dx != 0 || opt_dy != 0 {
                                entry.opt_rem_x += (opt_dx as f32) * sens * 0.6;
                                entry.opt_rem_y += (opt_dy as f32) * sens * 0.6;
                                let mx = entry.opt_rem_x.trunc() as i32;
                                let my = entry.opt_rem_y.trunc() as i32;
                                entry.opt_rem_x -= mx as f32;
                                entry.opt_rem_y -= my as f32;
                                if mx != 0 || my != 0 {
                                    emit_mouse_delta(mx, my, left_click);
                                }
                            }
                        }

                        Some((entry.device_id.clone(), state, dt, hz))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((id, state, dt, hz)) = update {
            if let Ok(guard) = ble_state().lock() {
                if let Some(st) = guard.as_ref() {
                    st.store.update_state(&id, state, None, dt, hz);
                }
            }
        }
    }
}

extern "C" fn optical_init_on_queue(_ctx: *mut c_void) {
    unsafe {
        if let Ok(guard) = ble_state().lock() {
            if let Some(st) = guard.as_ref() {
                for entry in st.peripherals.values() {
                    if let Some(ch_ptr) = entry.cmd_char {
                        for opt_pkt in switch2_optical_init_packets() {
                            write_ctrl_bytes(
                                entry.peripheral as Id,
                                ch_ptr as Id,
                                entry.cmd_write_type,
                                &opt_pkt,
                            );
                        }
                    }
                }
            }
        }
    }
}

extern "C" fn keepalive_tick_on_queue(_ctx: *mut c_void) {
    unsafe {
        if let Ok(mut guard) = ble_state().lock() {
            if let Some(st) = guard.as_mut() {
                for entry in st.peripherals.values_mut() {
                    if let Some(ch_ptr) = entry.cmd_char {
                        entry.keepalive_seq = entry.keepalive_seq.wrapping_add(1);
                        let pkt = switch2_keepalive_packet(entry.keepalive_seq);
                        write_ctrl_bytes(
                            entry.peripheral as Id,
                            ch_ptr as Id,
                            entry.cmd_write_type,
                            &pkt,
                        );
                    }
                }
            }
        }
    }
}

pub struct BleBackend {
    running: Arc<AtomicBool>,
    keepalive_thread: Option<thread::JoinHandle<()>>,
}

unsafe fn bundle_has_bluetooth_usage_description() -> bool {
    let bundle: Id = msg0(cls("NSBundle"), "mainBundle");
    if bundle.is_null() {
        return false;
    }
    let info: Id = msg0(bundle, "infoDictionary");
    if info.is_null() {
        return false;
    }
    let key = nsstring("NSBluetoothAlwaysUsageDescription");
    let val: Id = msg1(info, "objectForKey:", key);
    !val.is_null()
}

impl BleBackend {
    pub fn start(store: Arc<SharedStore>, scan_enabled: bool) -> Self {
        unsafe {
            if !bundle_has_bluetooth_usage_description() {
                return Self {
                    running: Arc::new(AtomicBool::new(false)),
                    keepalive_thread: None,
                };
            }
            dlopen(
                c"/System/Library/Frameworks/CoreBluetooth.framework/CoreBluetooth".as_ptr(),
                RTLD_LAZY,
            );

            let cb_cls = cls("CBCentralManager");
            if !cb_cls.is_null() {
                let mut guard = ble_state().lock().unwrap();
                if guard.is_none() {
                    let attr = dispatch_queue_attr_make_with_qos_class(
                        std::ptr::null(),
                        QOS_CLASS_USER_INTERACTIVE,
                        0,
                    );
                    let queue =
                        dispatch_queue_create(c"com.pearruntime.ble.controller".as_ptr(), attr);
                    let del_cls = register_ble_delegate_class();
                    let del_alloc: Id = msg0(del_cls, "alloc");
                    let delegate: Id = msg0(del_alloc, "init");
                    let central_alloc: Id = msg0(cb_cls, "alloc");
                    let central: Id =
                        msg2(central_alloc, "initWithDelegate:queue:", delegate, queue as Id);
                    *guard = Some(BleRuntimeState {
                        store,
                        central: central as usize,
                        delegate: delegate as usize,
                        queue: queue as usize,
                        scanning_wanted: scan_enabled,
                        peripherals: HashMap::new(),
                    });
                } else if let Some(st) = guard.as_mut() {
                    st.scanning_wanted = scan_enabled;
                    if scan_enabled {
                        start_scan_internal(st.central as Id);
                    }
                }
            }
        }

        // Note: Switch 2 BLE uses 0x30 (LED) / 0x0C (optical) commands; do not send Switch 1 0x00 rumble packets.
        let _ = keepalive_tick_on_queue;
        let _ = switch2_keepalive_packet;
        let running = Arc::new(AtomicBool::new(false));
        let keepalive_thread = None;
        let _ = thread::Builder::new;
        let _ = Duration::from_secs;
        let _ = dispatch_async_f;

        Self {
            running,
            keepalive_thread,
        }
    }

    pub fn enable_optical_mode(&self) {
        if let Ok(guard) = ble_state().lock() {
            if let Some(st) = guard.as_ref() {
                if st.queue != 0 {
                    unsafe {
                        dispatch_async_f(
                            st.queue as *const c_void,
                            std::ptr::null_mut(),
                            optical_init_on_queue,
                        );
                    }
                }
            }
        }
    }

    pub fn set_scanning(&self, enabled: bool) {
        if let Ok(mut guard) = ble_state().lock() {
            if let Some(st) = guard.as_mut() {
                st.scanning_wanted = enabled;
                unsafe {
                    if enabled {
                        start_scan_internal(st.central as Id);
                    } else if st.central != 0 {
                        let _: () = msg0(st.central as Id, "stopScan");
                    }
                }
            }
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(h) = self.keepalive_thread.take() {
            let _ = h.join();
        }
        if let Ok(mut guard) = ble_state().lock() {
            if let Some(mut st) = guard.take() {
                unsafe {
                    if st.central != 0 {
                        let _: () = msg0(st.central as Id, "stopScan");
                        for (_, entry) in st.peripherals.drain() {
                            let _: () = msg1(
                                st.central as Id,
                                "cancelPeripheralConnection:",
                                entry.peripheral as Id,
                            );
                            if let Some(c) = entry.cmd_char {
                                objc_release(c as Id);
                            }
                            objc_release(entry.peripheral as Id);
                        }
                        objc_release(st.central as Id);
                    }
                    if st.delegate != 0 {
                        objc_release(st.delegate as Id);
                    }
                }
            }
        }
    }
}

impl Drop for BleBackend {
    fn drop(&mut self) {
        self.stop();
    }
}
