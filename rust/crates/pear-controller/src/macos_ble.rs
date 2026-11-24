//! Pure-Rust CoreBluetooth BLE backend for Nintendo Switch 2 Joy-Cons & Pro Controller 2
//! (Buttons, 12-bit sticks, 6-axis Gyro/Accelerometer IMU, Player LEDs, and HD Rumble Haptics).

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use pear_objc::{
    add_method, autorelease_pool, cls, msg0, msg1, msg2, msg3, nsnumber_bool, nsstring,
    objc_release, objc_retain, Class, Id, Imp, Sel,
};

use crate::protocols::switch2::{
    classify_by_char_uuid, classify_by_name, is_switch2_ctrl_char, is_switch2_input_char,
    parse_manufacturer_data, parse_switch2_ble_report, switch2_configure_vibration_packet,
    switch2_imu_init_packets, switch2_pairing_vibration_packet, switch2_player_led_packet,
    switch2_vibration_block_packet, Switch2StickCal, NINTENDO_VID, PID_JOYCON2_LEFT,
    PID_JOYCON2_RIGHT, PID_PRO2, SWITCH2_COMMON_INPUT_UUID, SWITCH2_CTRL_CHAR_UUID,
    SWITCH2_RESPONSE_CHAR_UUID, SWITCH2_SERVICE_UUID,
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
}

const RTLD_LAZY: i32 = 1;
const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;
const CB_MANAGER_STATE_POWERED_ON: isize = 5;
const CB_CHAR_PROP_WRITE_WITHOUT_RESPONSE: usize = 0x04;
const CB_CHAR_PROP_WRITE: usize = 0x08;
const CB_CHAR_WRITE_WITH_RESPONSE: usize = 0;
const CB_CHAR_WRITE_WITHOUT_RESPONSE: usize = 1;

struct BlePeripheralEntry {
    device_id: String,
    peripheral: usize,
    cmd_char: Option<usize>,
    cmd_write_type: usize,
    vib_char: Option<usize>,
    vib_write_type: usize,
    kind: ControllerKind,
    pid: u16,
    name: String,
    last_report_us: u64,
    rumble_seq: u8,
    cal: Switch2StickCal,
}

struct BleRuntimeState {
    store: Arc<SharedStore>,
    central: usize,
    delegate: usize,
    _queue: usize,
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
            "centralManager:didFailToConnectPeripheral:error:",
            on_fail_to_connect_peripheral as Imp,
            "v@:@@@",
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
        autorelease_pool(|| {
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
        });
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
        autorelease_pool(|| {
            let mfg_key = nsstring("kCBAdvDataManufacturerData");
            let mfg_data: Id = msg1(adv, "objectForKey:", mfg_key);
            let mut detected: Option<(u16, ControllerKind)> = None;
            let mut matched_switch2 = false;
            if !mfg_data.is_null() {
                let len: usize = msg0(mfg_data, "length");
                let bytes_ptr: *const u8 = msg0(mfg_data, "bytes");
                if !bytes_ptr.is_null() && len > 0 {
                    let slice = std::slice::from_raw_parts(bytes_ptr, len);
                    detected = parse_manufacturer_data(slice);
                    if detected.is_some() || slice.windows(2).any(|w| w == [0x03, 0x7E]) {
                        matched_switch2 = true;
                    }
                }
            }

            if !matched_switch2 {
                let svc_key = nsstring("kCBAdvDataServiceUUIDs");
                let svc_uuids: Id = msg1(adv, "objectForKey:", svc_key);
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
            }

            let pname: Id = msg0(peripheral, "name");
            let mut name_str = nsstring_to_rust(pname);
            if name_str.is_empty() {
                let local_key = nsstring("kCBAdvDataLocalName");
                let local_name: Id = msg1(adv, "objectForKey:", local_key);
                name_str = nsstring_to_rust(local_name);
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

            let (pid, kind) =
                detected.unwrap_or((PID_JOYCON2_RIGHT, ControllerKind::Switch2JoyConRight));
            let p_key = peripheral as usize;
            let p_state: isize = msg0(peripheral, "state");
            let uuid_str = peripheral_uuid_string(peripheral);
            let display_name = if name_str.is_empty() {
                kind.display_name().to_string()
            } else {
                name_str
            };

            let should_connect = if let Ok(mut guard) = ble_state().lock() {
                if let Some(st) = guard.as_mut() {
                    if let Some(existing) = st.peripherals.get(&p_key) {
                        p_state == 0 && existing.cmd_char.is_none()
                    } else {
                        objc_retain(peripheral);
                        st.peripherals.insert(
                            p_key,
                            BlePeripheralEntry {
                                device_id: format!("ble:{}", uuid_str),
                                peripheral: p_key,
                                cmd_char: None,
                                cmd_write_type: CB_CHAR_WRITE_WITH_RESPONSE,
                                vib_char: None,
                                vib_write_type: CB_CHAR_WRITE_WITHOUT_RESPONSE,
                                kind,
                                pid,
                                name: display_name,
                                last_report_us: now_us(),
                                rumble_seq: 0,
                                cal: Switch2StickCal::default(),
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
        });
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
        autorelease_pool(|| {
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
                            raw_hex: String::new(),
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
            let _: () = msg1(
                peripheral,
                "discoverServices:",
                std::ptr::null::<c_void>() as Id,
            );
        });
    }
}

extern "C" fn on_fail_to_connect_peripheral(
    _self: Id,
    _cmd: Sel,
    central: Id,
    peripheral: Id,
    _error: Id,
) {
    if peripheral.is_null() {
        return;
    }
    unsafe {
        autorelease_pool(|| {
            let p_key = peripheral as usize;
            let mut rescan = false;
            if let Ok(mut guard) = ble_state().lock() {
                if let Some(st) = guard.as_mut() {
                    if let Some(entry) = st.peripherals.remove(&p_key) {
                        if let Some(c) = entry.cmd_char {
                            objc_release(c as Id);
                        }
                        if let Some(v) = entry.vib_char {
                            objc_release(v as Id);
                        }
                        st.store.remove_device(&entry.device_id);
                    }
                    rescan = st.scanning_wanted;
                }
            }
            objc_release(peripheral);
            if rescan && !central.is_null() {
                let _: () = msg0(central, "stopScan");
                start_scan_internal(central);
            }
        });
    }
}

extern "C" fn on_disconnect_peripheral(
    _self: Id,
    _cmd: Sel,
    central: Id,
    peripheral: Id,
    _error: Id,
) {
    if peripheral.is_null() {
        return;
    }
    unsafe {
        autorelease_pool(|| {
            let p_key = peripheral as usize;
            let mut rescan = false;
            if let Ok(mut guard) = ble_state().lock() {
                if let Some(st) = guard.as_mut() {
                    if let Some(entry) = st.peripherals.remove(&p_key) {
                        if let Some(c) = entry.cmd_char {
                            objc_release(c as Id);
                        }
                        if let Some(v) = entry.vib_char {
                            objc_release(v as Id);
                        }
                        st.store.remove_device(&entry.device_id);
                    }
                    rescan = st.scanning_wanted;
                }
            }
            objc_release(peripheral);
            if rescan && !central.is_null() {
                let _: () = msg0(central, "stopScan");
                start_scan_internal(central);
            }
        });
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
        autorelease_pool(|| {
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
        });
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
        autorelease_pool(|| {
            let chars: Id = msg0(service, "characteristics");
            if chars.is_null() {
                return;
            }
            let p_key = peripheral as usize;
            let count: usize = msg0(chars, "count");
            let mut found_cmd_index: Option<usize> = None;
            let mut char_list: Vec<(Id, String, usize)> = Vec::with_capacity(count);
            for i in 0..count {
                let ch: Id = msg1(chars, "objectAtIndex:", i);
                let cuuid: Id = msg0(ch, "UUID");
                let ustr_id: Id = msg0(cuuid, "UUIDString");
                let uuid = nsstring_to_rust(ustr_id);
                let props: usize = msg0(ch, "properties");
                char_list.push((ch, uuid.clone(), props));

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
                    // Subscribe only to `AB7DE9BE-89FE-49AD-828F-118F09DF7FD2` (matching JoyCon2Mac)
                    // so alternate compact report characteristics don't interleave incompatible frames.
                    if uuid.eq_ignore_ascii_case(SWITCH2_COMMON_INPUT_UUID) {
                        let _: () = msg2(peripheral, "setNotifyValue:forCharacteristic:", 1i8, ch);
                    }
                } else if uuid.eq_ignore_ascii_case(SWITCH2_RESPONSE_CHAR_UUID) {
                    let _: () = msg2(peripheral, "setNotifyValue:forCharacteristic:", 1i8, ch);
                } else if is_switch2_ctrl_char(&uuid) {
                    if uuid.eq_ignore_ascii_case(SWITCH2_CTRL_CHAR_UUID) {
                        found_cmd_index = Some(i);
                    }
                    // JoyCon2Mac prefers CBCharacteristicWriteWithResponse when supported.
                    let write_type = if (props & CB_CHAR_PROP_WRITE) != 0 {
                        CB_CHAR_WRITE_WITH_RESPONSE
                    } else {
                        CB_CHAR_WRITE_WITHOUT_RESPONSE
                    };
                    if let Ok(mut guard) = ble_state().lock() {
                        if let Some(st) = guard.as_mut() {
                            if let Some(entry) = st.peripherals.get_mut(&p_key) {
                                if entry.cmd_char.is_none()
                                    || uuid.eq_ignore_ascii_case(SWITCH2_CTRL_CHAR_UUID)
                                {
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

                    // Timed JoyCon2Mac init sequence:
                    //   t=500ms:  0C 91 01 02 (enable 6-axis IMU step 1)
                    //   t=1000ms: 0C 91 01 04 (enable 6-axis IMU step 2)
                    //   t=1200ms: 0A 91 01 08 (configure HD Rumble haptics) + 09 91 01 07 (Player 1 LED)
                    let imu_pkts = switch2_imu_init_packets();
                    thread::spawn(move || {
                        autorelease_pool(|| {
                            let send_if_alive = |bytes: &[u8]| {
                                if let Ok(guard) = ble_state().lock() {
                                    if let Some(st) = guard.as_ref() {
                                        if let Some(entry) = st.peripherals.get(&p_key) {
                                            if let Some(ch_ptr) = entry.cmd_char {
                                                write_ctrl_bytes(
                                                    entry.peripheral as Id,
                                                    ch_ptr as Id,
                                                    entry.cmd_write_type,
                                                    bytes,
                                                );
                                            }
                                        }
                                    }
                                }
                            };

                            thread::sleep(Duration::from_millis(500));
                            send_if_alive(&imu_pkts[0]);
                            thread::sleep(Duration::from_millis(500));
                            send_if_alive(&imu_pkts[1]);
                            thread::sleep(Duration::from_millis(200));
                            let vib_pkt = switch2_configure_vibration_packet();
                            send_if_alive(&vib_pkt);
                            thread::sleep(Duration::from_millis(80));
                            let led_pkt = switch2_player_led_packet(1);
                            send_if_alive(&led_pkt);
                        });
                    });
                }
            }

            // Discover vibrationCharacteristic (writable characteristic before UUID_COMMAND,
            // matching JoyCon2Mac/BLEManager.mm).
            if let Some(cmd_idx) = found_cmd_index {
                for i in (0..cmd_idx).rev() {
                    let (ch, ref uuid, props) = char_list[i];
                    if is_switch2_input_char(uuid)
                        || uuid.eq_ignore_ascii_case(SWITCH2_RESPONSE_CHAR_UUID)
                        || is_switch2_ctrl_char(uuid)
                    {
                        continue;
                    }
                    if (props & (CB_CHAR_PROP_WRITE_WITHOUT_RESPONSE | CB_CHAR_PROP_WRITE)) != 0 {
                        let vib_wt = if (props & CB_CHAR_PROP_WRITE_WITHOUT_RESPONSE) != 0 {
                            CB_CHAR_WRITE_WITHOUT_RESPONSE
                        } else {
                            CB_CHAR_WRITE_WITH_RESPONSE
                        };
                        if let Ok(mut guard) = ble_state().lock() {
                            if let Some(st) = guard.as_mut() {
                                if let Some(entry) = st.peripherals.get_mut(&p_key) {
                                    if let Some(old) = entry.vib_char.take() {
                                        objc_release(old as Id);
                                    }
                                    objc_retain(ch);
                                    entry.vib_char = Some(ch as usize);
                                    entry.vib_write_type = vib_wt;
                                }
                            }
                        }
                        break;
                    }
                }
            }
        });
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
        autorelease_pool(|| {
            let cuuid: Id = msg0(characteristic, "UUID");
            let ustr_id: Id = msg0(cuuid, "UUIDString");
            let uuid = nsstring_to_rust(ustr_id);
            if !is_switch2_input_char(&uuid) {
                return;
            }
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
                        if !cfg.is_device_enabled(&entry.device_id) {
                            None
                        } else if let Some(state) =
                            parse_switch2_ble_report(entry.kind, bytes, &cfg, &mut entry.cal)
                        {
                            let raw_hex: String = bytes
                                .iter()
                                .take(24)
                                .map(|b| format!("{:02X}", b))
                                .collect::<Vec<_>>()
                                .join(" ");
                            let dt = now.saturating_sub(entry.last_report_us);
                            entry.last_report_us = now;
                            let hz = if dt > 500 && dt < 250_000 {
                                (1_000_000 / dt) as u32
                            } else {
                                60
                            };
                            Some((entry.device_id.clone(), state, raw_hex, dt, hz))
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

            if let Some((id, state, raw_hex, dt, hz)) = update {
                if let Ok(guard) = ble_state().lock() {
                    if let Some(st) = guard.as_ref() {
                        st.store.update_raw_hex(&id, raw_hex);
                        st.store.update_state(&id, state, None, dt, hz);
                    }
                }
            }
        });
    }
}

pub struct BleBackend {
    _running: Arc<AtomicBool>,
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
                    _running: Arc::new(AtomicBool::new(false)),
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
                        _queue: queue as usize,
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

        Self {
            _running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn test_haptics(&self) {
        thread::spawn(|| unsafe {
            autorelease_pool(|| {
                let mut vib_targets: Vec<(usize, usize, usize, u8)> = Vec::new();
                if let Ok(mut guard) = ble_state().lock() {
                    if let Some(st) = guard.as_mut() {
                        let pairing_vib = switch2_pairing_vibration_packet();
                        for entry in st.peripherals.values_mut() {
                            if let Some(cmd_ch) = entry.cmd_char {
                                write_ctrl_bytes(
                                    entry.peripheral as Id,
                                    cmd_ch as Id,
                                    entry.cmd_write_type,
                                    &pairing_vib,
                                );
                            }
                            if let Some(vib_ch) = entry.vib_char {
                                entry.rumble_seq = entry.rumble_seq.wrapping_add(1);
                                let pkt = switch2_vibration_block_packet(entry.rumble_seq, 255);
                                write_ctrl_bytes(
                                    entry.peripheral as Id,
                                    vib_ch as Id,
                                    entry.vib_write_type,
                                    &pkt,
                                );
                                vib_targets.push((
                                    entry.peripheral,
                                    vib_ch,
                                    entry.vib_write_type,
                                    entry.rumble_seq,
                                ));
                            }
                        }
                    }
                }
                if vib_targets.is_empty() {
                    return;
                }
                thread::sleep(Duration::from_millis(220));
                for (p, ch, wt, seq) in vib_targets {
                    let off_pkt = switch2_vibration_block_packet(seq.wrapping_add(1), 0);
                    write_ctrl_bytes(p as Id, ch as Id, wt, &off_pkt);
                }
            });
        });
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
        self._running.store(false, Ordering::Release);
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
                            if let Some(v) = entry.vib_char {
                                objc_release(v as Id);
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
