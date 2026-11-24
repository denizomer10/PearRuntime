//! Pure-Rust low-latency controller engine for PearRuntime & PearSilicon.
//!
//! Supports Nintendo Switch 2 Joy-Cons / Pro Controller 2 (BLE GATT),
//! Sony DualSense / DualShock 4, Microsoft Xbox controllers, Nintendo Switch 1
//! Pro / Joy-Cons, and generic HID gamepads.

pub mod macos_ble;
pub mod macos_hid;
pub mod protocols;
pub mod types;

use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use macos_ble::BleBackend;
use macos_hid::HidBackend;
use protocols::combiner::build_effective_devices;
use types::{ControllerConfig, ControllerDevice, ControllerKind, ControllerState};

pub fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

pub type StateListener = Arc<dyn Fn(&[ControllerDevice]) + Send + Sync>;

pub struct SharedStore {
    config: RwLock<ControllerConfig>,
    raw_devices: Mutex<HashMap<String, ControllerDevice>>,
    listeners: Mutex<Vec<StateListener>>,
}

impl SharedStore {
    pub fn new(config: ControllerConfig) -> Self {
        Self {
            config: RwLock::new(config),
            raw_devices: Mutex::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
        }
    }

    pub fn config(&self) -> ControllerConfig {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_config(&self, cfg: ControllerConfig) {
        if let Ok(mut w) = self.config.write() {
            *w = cfg;
        }
        self.notify_listeners();
    }

    pub fn add_listener(&self, listener: StateListener) {
        if let Ok(mut l) = self.listeners.lock() {
            l.push(listener);
        }
    }

    pub fn upsert_device(&self, dev: ControllerDevice) {
        if let Ok(mut map) = self.raw_devices.lock() {
            map.insert(dev.id.clone(), dev);
        }
        self.notify_listeners();
    }

    pub fn remove_device(&self, id: &str) {
        if let Ok(mut map) = self.raw_devices.lock() {
            map.remove(id);
        }
        self.notify_listeners();
    }

    pub fn update_kind(&self, id: &str, kind: ControllerKind) {
        if let Ok(mut map) = self.raw_devices.lock() {
            if let Some(d) = map.get_mut(id) {
                d.kind = kind;
            }
        }
        self.notify_listeners();
    }

    pub fn update_state(
        &self,
        id: &str,
        state: ControllerState,
        battery: Option<u8>,
        latency_us: u64,
        report_rate_hz: u32,
    ) {
        if let Ok(mut map) = self.raw_devices.lock() {
            if let Some(d) = map.get_mut(id) {
                d.state = state;
                if battery.is_some() {
                    d.battery_percent = battery;
                }
                if latency_us > 0 {
                    d.latency_us = latency_us;
                }
                if report_rate_hz > 0 {
                    d.report_rate_hz = report_rate_hz;
                }
            }
        }
        self.notify_listeners();
    }

    pub fn effective_devices(&self) -> Vec<ControllerDevice> {
        let cfg = self.config();
        let raw: Vec<ControllerDevice> = self
            .raw_devices
            .lock()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        let mut eff = build_effective_devices(&raw, &cfg);
        eff.sort_by(|a, b| a.id.cmp(&b.id));
        eff
    }

    pub fn snapshot_json(&self) -> String {
        let cfg = self.config();
        let devices = self.effective_devices();
        let dev_json: Vec<String> = devices.iter().map(|d| d.to_json()).collect();
        format!(
            "{{\"lowLatencyMode\":{},\"pollingHz\":{},\"joyconCombineMode\":\"{}\",\"devices\":[{}]}}",
            cfg.low_latency_mode,
            cfg.polling_hz,
            cfg.joycon_combine_mode.as_str(),
            dev_json.join(","),
        )
    }

    fn notify_listeners(&self) {
        let has_listeners = self
            .listeners
            .lock()
            .map(|l| !l.is_empty())
            .unwrap_or(false);
        if !has_listeners {
            return;
        }
        let devices = self.effective_devices();
        if let Ok(listeners) = self.listeners.lock() {
            for cb in listeners.iter() {
                cb(&devices);
            }
        }
    }
}

struct RuntimeInstance {
    store: Arc<SharedStore>,
    hid: Option<HidBackend>,
    ble: Option<BleBackend>,
}

static RUNTIME: OnceLock<Mutex<Option<RuntimeInstance>>> = OnceLock::new();

fn runtime_slot() -> &'static Mutex<Option<RuntimeInstance>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}

pub fn start_with_config(cfg: ControllerConfig) -> Arc<SharedStore> {
    let mut guard = runtime_slot().lock().unwrap();
    if let Some(inst) = guard.as_ref() {
        inst.store.set_config(cfg.clone());
        if let Some(ble) = inst.ble.as_ref() {
            ble.set_scanning(cfg.ble_switch2_enabled);
        }
        return inst.store.clone();
    }
    let store = Arc::new(SharedStore::new(cfg.clone()));
    let hid = Some(HidBackend::start(store.clone()));
    let ble = Some(BleBackend::start(store.clone(), cfg.ble_switch2_enabled));
    *guard = Some(RuntimeInstance {
        store: store.clone(),
        hid,
        ble,
    });
    store
}

pub fn shared_store() -> Option<Arc<SharedStore>> {
    runtime_slot()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|i| i.store.clone()))
}

pub fn stop_runtime() {
    let mut guard = runtime_slot().lock().unwrap();
    if let Some(mut inst) = guard.take() {
        if let Some(mut ble) = inst.ble.take() {
            ble.stop();
        }
        if let Some(mut hid) = inst.hid.take() {
            hid.stop();
        }
    }
}

// --- C ABI Exports for PearSilicon SwiftUI UI & Host ---

#[no_mangle]
pub extern "C" fn pear_controller_start(config_json: *const c_char) -> bool {
    let cfg = if config_json.is_null() {
        ControllerConfig::default()
    } else {
        unsafe {
            CStr::from_ptr(config_json)
                .to_str()
                .map(ControllerConfig::from_json)
                .unwrap_or_default()
        }
    };
    let _ = start_with_config(cfg);
    true
}

#[no_mangle]
pub extern "C" fn pear_controller_stop() {
    stop_runtime();
}

#[no_mangle]
pub extern "C" fn pear_controller_set_config_json(config_json: *const c_char) {
    if config_json.is_null() {
        return;
    }
    let cfg = unsafe {
        CStr::from_ptr(config_json)
            .to_str()
            .map(ControllerConfig::from_json)
            .unwrap_or_default()
    };
    if let Ok(guard) = runtime_slot().lock() {
        if let Some(inst) = guard.as_ref() {
            inst.store.set_config(cfg.clone());
            if let Some(ble) = inst.ble.as_ref() {
                ble.set_scanning(cfg.ble_switch2_enabled);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn pear_controller_set_ble_scan(enabled: bool) {
    if let Ok(guard) = runtime_slot().lock() {
        if let Some(inst) = guard.as_ref() {
            if let Some(ble) = inst.ble.as_ref() {
                ble.set_scanning(enabled);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn pear_controller_snapshot_json() -> *mut c_char {
    let json = shared_store()
        .map(|s| s.snapshot_json())
        .unwrap_or_else(|| "{\"lowLatencyMode\":true,\"pollingHz\":250,\"joyconCombineMode\":\"auto\",\"devices\":[]}".to_string());
    CString::new(json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn pear_controller_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::dualsense::{classify_sony_pid, parse_sony_report};
    use protocols::switch1::parse_switch1_report;
    use protocols::switch2::{
        classify_by_char_uuid, parse_manufacturer_data, parse_switch2_ble_report,
        PID_JOYCON2_LEFT, PID_JOYCON2_RIGHT, SWITCH2_JC2_L_INPUT_UUID, SWITCH2_JC2_R_INPUT_UUID,
    };
    use types::{JoyConCombineMode, TransportKind};

    #[test]
    fn parses_switch2_manufacturer_data() {
        // Little-endian VID 0x057E + PID 0x2066
        let adv = [0x7E, 0x05, 0x01, 0x66, 0x20, 0x00];
        let parsed = parse_manufacturer_data(&adv);
        assert_eq!(parsed, Some((PID_JOYCON2_LEFT, ControllerKind::Switch2JoyConLeft)));

        let adv_r = [0x7E, 0x05, 0x67, 0x20];
        assert_eq!(
            parse_manufacturer_data(&adv_r),
            Some((PID_JOYCON2_RIGHT, ControllerKind::Switch2JoyConRight))
        );
    }

    #[test]
    fn classifies_switch2_char_uuids() {
        assert_eq!(
            classify_by_char_uuid(SWITCH2_JC2_L_INPUT_UUID),
            Some(ControllerKind::Switch2JoyConLeft)
        );
        assert_eq!(
            classify_by_char_uuid(SWITCH2_JC2_R_INPUT_UUID),
            Some(ControllerKind::Switch2JoyConRight)
        );
    }

    #[test]
    fn parses_switch2_ble_report_and_nintendo_swap() {
        let mut pkt = [0u8; 64];
        // Set bit 3 (A on Nintendo) and bit 6 (R1)
        let btn: u32 = (1 << 3) | (1 << 6);
        pkt[4..8].copy_from_slice(&btn.to_le_bytes());
        // Neutral sticks (2048, 2048 -> 0x00, 0x08, 0x80)
        pkt[10] = 0x00;
        pkt[11] = 0x08;
        pkt[12] = 0x80;
        pkt[13] = 0x00;
        pkt[14] = 0x08;
        pkt[15] = 0x80;

        let mut cfg = ControllerConfig::default();
        cfg.nintendo_button_layout = false;
        let st = parse_switch2_ble_report(ControllerKind::Switch2Pro, &pkt, &cfg).unwrap();
        // With Xbox/iOS layout, Nintendo A becomes B!
        assert!(st.b);
        assert!(!st.a);
        assert!(st.r1);
        assert_eq!(st.left_x, 0.0);

        cfg.nintendo_button_layout = true;
        let st_n = parse_switch2_ble_report(ControllerKind::Switch2Pro, &pkt, &cfg).unwrap();
        assert!(st_n.a);
    }

    #[test]
    fn parses_dualsense_usb_report() {
        let mut pkt = [0u8; 64];
        pkt[0] = 0x01; // USB report ID
        pkt[1] = 128;  // LX center
        pkt[2] = 0;    // LY up -> +1.0
        pkt[3] = 255;  // RX right -> +1.0
        pkt[4] = 128;  // RY center
        pkt[5] = 255;  // L2 full
        pkt[6] = 0;    // R2 zero
        pkt[8] = 0x20 | 0x00; // Cross + D-pad Up (hat 0)
        pkt[9] = 0x02;        // R1
        let cfg = ControllerConfig::default();
        let st = parse_sony_report(ControllerKind::DualSense, &pkt, &cfg).unwrap();
        assert_eq!(classify_sony_pid(0x0CE6), Some(ControllerKind::DualSense));
        assert!(st.a);
        assert!(st.dpad_up);
        assert!(st.r1);
        assert!(st.left_y > 0.9);
        assert!(st.right_x > 0.9);
        assert!(st.l2 > 0.95);
    }

    #[test]
    fn parses_switch1_full_report_and_combines_joycon2_pair() {
        let mut pkt = [0u8; 49];
        pkt[0] = 0x30;
        pkt[2] = 0x80; // battery 100%
        pkt[3] = 0x08; // Nintendo A
        pkt[6] = 0x00;
        pkt[7] = 0x08;
        pkt[8] = 0x80;
        pkt[9] = 0x00;
        pkt[10] = 0x08;
        pkt[11] = 0x80;

        let cfg = ControllerConfig::default();
        let (st, batt) = parse_switch1_report(ControllerKind::Switch1Pro, &pkt, &cfg).unwrap();
        assert_eq!(batt, Some(100));
        assert!(st.b);

        // Combine Joy-Con 2 L + R into Pair
        let mut left_st = ControllerState::default();
        left_st.dpad_up = true;
        left_st.left_x = -0.8;
        let mut right_st = ControllerState::default();
        right_st.a = true;
        right_st.right_y = 0.7;

        let raw = vec![
            ControllerDevice {
                id: "ble:l".to_string(),
                name: "Joy-Con 2 (L)".to_string(),
                kind: ControllerKind::Switch2JoyConLeft,
                transport: TransportKind::Ble,
                vendor_id: 0x057E,
                product_id: PID_JOYCON2_LEFT,
                battery_percent: Some(90),
                latency_us: 4000,
                report_rate_hz: 125,
                connected: true,
                state: left_st,
            },
            ControllerDevice {
                id: "ble:r".to_string(),
                name: "Joy-Con 2 (R)".to_string(),
                kind: ControllerKind::Switch2JoyConRight,
                transport: TransportKind::Ble,
                vendor_id: 0x057E,
                product_id: PID_JOYCON2_RIGHT,
                battery_percent: Some(80),
                latency_us: 4000,
                report_rate_hz: 125,
                connected: true,
                state: right_st,
            },
        ];

        let combined = build_effective_devices(&raw, &cfg);
        assert_eq!(combined.len(), 1);
        assert_eq!(combined[0].kind, ControllerKind::Switch2JoyConPair);
        assert!(combined[0].state.dpad_up);
        assert!(combined[0].state.a);
        assert_eq!(combined[0].battery_percent, Some(80));

        let mut sep_cfg = cfg;
        sep_cfg.joycon_combine_mode = JoyConCombineMode::Separate;
        let separate = build_effective_devices(&raw, &sep_cfg);
        assert_eq!(separate.len(), 2);
    }

    #[test]
    fn parses_controller_config_json() {
        let json = r#"{"lowLatencyMode":true,"pollingHz":500,"leftDeadzone":0.12,"rightDeadzone":0.09,"joyconCombineMode":"separate","nintendoButtonLayout":true,"bleSwitch2Enabled":false}"#;
        let cfg = ControllerConfig::from_json(json);
        assert!(cfg.low_latency_mode);
        assert_eq!(cfg.polling_hz, 500);
        assert!((cfg.left_deadzone - 0.12).abs() < 0.001);
        assert!((cfg.right_deadzone - 0.09).abs() < 0.001);
        assert_eq!(cfg.joycon_combine_mode, JoyConCombineMode::Separate);
        assert!(cfg.nintendo_button_layout);
        assert!(!cfg.ble_switch2_enabled);
    }
}
