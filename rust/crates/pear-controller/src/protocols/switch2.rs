//! Nintendo Switch 2 (Joy-Con 2 & Pro Controller 2) BLE GATT protocol parser.

use crate::types::{
    apply_radial_deadzone, ControllerConfig, ControllerKind, ControllerState,
};

pub const NINTENDO_VID: u16 = 0x057E;
pub const NINTENDO_BLE_COMPANY_ID: u16 = 0x0553;
pub const PID_JOYCON2_RIGHT: u16 = 0x2066;
pub const PID_JOYCON2_LEFT: u16 = 0x2067;
pub const PID_PRO2: u16 = 0x2069;
pub const PID_GAMECUBE2: u16 = 0x2073;

pub const SWITCH2_SERVICE_UUID: &str = "AB7DE9BE-89FE-49AD-828F-118F09DF7FD0";
pub const SWITCH2_CMD_CHAR_UUID: &str = "AB7DE9BE-89FE-49AD-828F-118F09DF7FD1";
pub const SWITCH2_CTRL_CHAR_UUID: &str = "649D4AC9-8EB7-4E6C-AF44-1EA54FE5F005";
pub const SWITCH2_COMMON_INPUT_UUID: &str = "AB7DE9BE-89FE-49AD-828F-118F09DF7FD2";
pub const SWITCH2_JC2_L_INPUT_UUID: &str = "CC1BBBB5-7354-4D32-A716-A81CB241A32A";
pub const SWITCH2_JC2_R_INPUT_UUID: &str = "D5A9E01E-2FFC-4CCA-B20C-8B67142BF442";
pub const SWITCH2_PRO2_INPUT_UUID: &str = "7492866C-EC3E-4619-8258-32755FFCC0F8";

#[derive(Clone, Copy, Debug, Default)]
pub struct Switch2StickCal {
    pub lx_center: Option<u16>,
    pub ly_center: Option<u16>,
    pub rx_center: Option<u16>,
    pub ry_center: Option<u16>,
    pub prev_opt_x: Option<u16>,
    pub prev_opt_y: Option<u16>,
}

pub fn is_switch2_input_char(uuid: &str) -> bool {
    let u = uuid.to_ascii_uppercase();
    u == SWITCH2_COMMON_INPUT_UUID
        || u == SWITCH2_JC2_L_INPUT_UUID
        || u == SWITCH2_JC2_R_INPUT_UUID
        || u == SWITCH2_PRO2_INPUT_UUID
}

pub fn is_switch2_ctrl_char(uuid: &str) -> bool {
    let u = uuid.to_ascii_uppercase();
    u == SWITCH2_CTRL_CHAR_UUID || u == SWITCH2_CMD_CHAR_UUID
}

pub fn classify_by_char_uuid(uuid: &str) -> Option<ControllerKind> {
    let u = uuid.to_ascii_uppercase();
    if u == SWITCH2_JC2_L_INPUT_UUID {
        Some(ControllerKind::Switch2JoyConLeft)
    } else if u == SWITCH2_JC2_R_INPUT_UUID {
        Some(ControllerKind::Switch2JoyConRight)
    } else if u == SWITCH2_PRO2_INPUT_UUID {
        Some(ControllerKind::Switch2Pro)
    } else {
        None
    }
}

pub fn classify_by_pid(pid: u16) -> Option<ControllerKind> {
    match pid {
        PID_JOYCON2_RIGHT => Some(ControllerKind::Switch2JoyConRight),
        PID_JOYCON2_LEFT => Some(ControllerKind::Switch2JoyConLeft),
        PID_PRO2 | PID_GAMECUBE2 => Some(ControllerKind::Switch2Pro),
        _ => None,
    }
}

/// Inspect BLE manufacturer data for Nintendo (`0x0553` or `0x057E`) and Switch 2 subtype/PID.
pub fn parse_manufacturer_data(data: &[u8]) -> Option<(u16, ControllerKind)> {
    if data.len() < 4 {
        return None;
    }
    let company = u16::from_le_bytes([data[0], data[1]]);
    if company == NINTENDO_BLE_COMPANY_ID && data.len() >= 8 {
        // Switch 2 BLE advertisement: [53 05] 01 00 03 7e 05 <subtype> ...
        // Subtype byte at index 7 (or index 5 of payload): 0x66 = Right, 0x67 = Left, 0x69 = Pro 2
        if data[4] == 0x03 && data[5] == 0x7E && data[6] == 0x05 {
            let pid = 0x2000 | (data[7] as u16);
            if let Some(kind) = classify_by_pid(pid) {
                return Some((pid, kind));
            }
        }
    }
    if company == NINTENDO_VID || company == NINTENDO_BLE_COMPANY_ID {
        for window in data[2..].windows(2) {
            let pid = u16::from_le_bytes([window[0], window[1]]);
            if let Some(kind) = classify_by_pid(pid) {
                return Some((pid, kind));
            }
        }
    }
    None
}

pub fn classify_by_name(name: &str) -> Option<ControllerKind> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("joy-con 2") || lower.contains("joycon 2") || lower.contains("jc2") {
        if lower.contains("(r)") || lower.contains("right") {
            return Some(ControllerKind::Switch2JoyConRight);
        }
        return Some(ControllerKind::Switch2JoyConLeft);
    }
    if lower.contains("pro controller 2") || lower.contains("switch 2 pro") {
        return Some(ControllerKind::Switch2Pro);
    }
    None
}

/// Real Switch 2 Player LED command (`0x09 0x91 0x01 0x07`) from BlueRetro `sw2.c`.
pub fn switch2_player_led_packet(player_num: u8) -> [u8; 16] {
    const MAP: [u8; 8] = [0x01, 0x02, 0x04, 0x08, 0x03, 0x06, 0x0C, 0x0F];
    let idx = player_num.clamp(1, 8) as usize - 1;
    [
        0x09, 0x91, 0x01, 0x07, 0x00, 0x08, 0x00, 0x00, MAP[idx], 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]
}

/// Optical mouse sensor initialization commands (`649d4ac9-...`).
pub fn switch2_optical_init_packets() -> [[u8; 12]; 2] {
    [
        [0x0C, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
        [0x0C, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
    ]
}

pub fn switch2_keepalive_packet(seq: u8) -> [u8; 10] {
    [
        0x00,
        seq & 0x0F,
        0x00,
        0x01,
        0x40,
        0x40,
        0x00,
        0x01,
        0x40,
        0x40,
    ]
}

fn decode_12bit_stick(b0: u8, b1: u8, b2: u8) -> (u16, u16) {
    let raw_x = (b0 as u16) | (((b1 & 0x0F) as u16) << 8);
    let raw_y = ((b1 >> 4) as u16) | ((b2 as u16) << 4);
    (raw_x, raw_y)
}

fn normalize_12bit(raw: u16, center: &mut Option<u16>) -> f32 {
    let c = match *center {
        Some(v) => v,
        None => {
            // Auto-calibrate center from the first neutral reading (~1998/2048).
            let init = if (1350..=2650).contains(&raw) {
                raw
            } else {
                2000
            };
            *center = Some(init);
            init
        }
    };
    (((raw as f32) - (c as f32)) / 1250.0).clamp(-1.0, 1.0)
}

/// Parse a Switch 2 BLE input notification payload (buttons, sticks, and optical mouse delta).
pub fn parse_switch2_ble_report(
    kind: ControllerKind,
    data: &[u8],
    cfg: &ControllerConfig,
    cal: &mut Switch2StickCal,
) -> Option<(ControllerState, (i16, i16))> {
    if data.len() < 9 {
        return None;
    }

    // Compact optical/report-0x20 format (`data[1] == 0x20`):
    // [counter, 0x20, btn_lo, btn_hi, status(0x07), stick[0..3], tag(0x30), ...]
    if data[1] == 0x20 {
        let mut state = ControllerState::default();
        let (raw_x, raw_y) = decode_12bit_stick(data[5], data[6], data[7]);
        let nx = normalize_12bit(raw_x, &mut cal.lx_center);
        let ny = normalize_12bit(raw_y, &mut cal.ly_center);

        if kind == ControllerKind::Switch2JoyConRight {
            let (rx, ry) = apply_radial_deadzone(nx, ny, cfg.right_deadzone);
            state.right_x = rx;
            state.right_y = ry;
            let b = data[2];
            state.y = (b & 0x01) != 0;
            state.x = (b & 0x02) != 0;
            state.b = (b & 0x04) != 0;
            state.a = (b & 0x08) != 0;
            state.r1 = (b & 0x40) != 0;
            state.r2 = if (b & 0x80) != 0 { 1.0 } else { 0.0 };
        } else {
            let (lx, ly) = apply_radial_deadzone(nx, ny, cfg.left_deadzone);
            state.left_x = lx;
            state.left_y = ly;
            let b = data[3];
            state.dpad_down = (b & 0x01) != 0;
            state.dpad_up = (b & 0x02) != 0;
            state.dpad_right = (b & 0x04) != 0;
            state.dpad_left = (b & 0x08) != 0;
            state.l1 = (b & 0x40) != 0;
            state.l2 = if (b & 0x80) != 0 { 1.0 } else { 0.0 };
        }

        let mut mouse_delta = (0i16, 0i16);
        if data.len() >= 20 {
            let cur_x = u16::from_le_bytes([data[16], data[17]]);
            let cur_y = u16::from_le_bytes([data[18], data[19]]);
            if let (Some(px), Some(py)) = (cal.prev_opt_x, cal.prev_opt_y) {
                let dx = cur_x.wrapping_sub(px) as i16;
                let dy = cur_y.wrapping_sub(py) as i16;
                if dx.abs() <= 400 && dy.abs() <= 400 {
                    mouse_delta = (dx, dy);
                }
            }
            cal.prev_opt_x = Some(cur_x);
            cal.prev_opt_y = Some(cur_y);
        }
        state.apply_nintendo_layout(cfg.nintendo_button_layout);
        return Some((state, mouse_delta));
    }

    if data.len() < 16 {
        return None;
    }

    let b4 = data[4];
    let b5 = data[5];
    let b6 = data[6];

    let mut state = ControllerState::default();

    match kind {
        ControllerKind::Switch2JoyConRight => {
            state.y = (b4 & 0x01) != 0;
            state.x = (b4 & 0x02) != 0;
            state.b = (b4 & 0x04) != 0;
            state.a = (b4 & 0x08) != 0;
            state.gr = (b4 & 0x10) != 0; // R-SR
            state.gl = (b4 & 0x20) != 0; // R-SL
            state.r1 = (b4 & 0x40) != 0;
            state.r2 = if (b4 & 0x80) != 0 { 1.0 } else { 0.0 };

            state.menu = (b5 & 0x02) != 0;
            state.r3 = (b5 & 0x04) != 0;
            state.home = (b5 & 0x10) != 0;
            state.c_button = (b5 & 0x40) != 0;

            let (raw_rx, raw_ry) = decode_12bit_stick(data[13], data[14], data[15]);
            let nx = normalize_12bit(raw_rx, &mut cal.rx_center);
            let ny = normalize_12bit(raw_ry, &mut cal.ry_center);
            let (rx, ry) = apply_radial_deadzone(nx, ny, cfg.right_deadzone);
            state.right_x = rx;
            state.right_y = ry;
        }
        ControllerKind::Switch2JoyConLeft => {
            state.options = (b5 & 0x01) != 0; // Minus
            state.l3 = (b5 & 0x08) != 0;
            state.capture = (b5 & 0x20) != 0;

            state.dpad_down = (b6 & 0x01) != 0;
            state.dpad_up = (b6 & 0x02) != 0;
            state.dpad_right = (b6 & 0x04) != 0;
            state.dpad_left = (b6 & 0x08) != 0;
            state.gr = (b6 & 0x10) != 0; // L-SR
            state.gl = (b6 & 0x20) != 0; // L-SL
            state.l1 = (b6 & 0x40) != 0;
            state.l2 = if (b6 & 0x80) != 0 { 1.0 } else { 0.0 };

            let (raw_lx, raw_ly) = decode_12bit_stick(data[10], data[11], data[12]);
            let nx = normalize_12bit(raw_lx, &mut cal.lx_center);
            let ny = normalize_12bit(raw_ly, &mut cal.ly_center);
            let (lx, ly) = apply_radial_deadzone(nx, ny, cfg.left_deadzone);
            state.left_x = lx;
            state.left_y = ly;
        }
        _ => {
            // Switch 2 Pro Controller / full layout
            state.y = (b4 & 0x01) != 0;
            state.x = (b4 & 0x02) != 0;
            state.b = (b4 & 0x04) != 0;
            state.a = (b4 & 0x08) != 0;
            state.r1 = (b4 & 0x40) != 0;
            state.r2 = if (b4 & 0x80) != 0 { 1.0 } else { 0.0 };

            state.options = (b5 & 0x01) != 0;
            state.menu = (b5 & 0x02) != 0;
            state.r3 = (b5 & 0x04) != 0;
            state.l3 = (b5 & 0x08) != 0;
            state.home = (b5 & 0x10) != 0;
            state.capture = (b5 & 0x20) != 0;
            state.c_button = (b5 & 0x40) != 0;

            state.dpad_down = (b6 & 0x01) != 0;
            state.dpad_up = (b6 & 0x02) != 0;
            state.dpad_right = (b6 & 0x04) != 0;
            state.dpad_left = (b6 & 0x08) != 0;
            state.l1 = (b6 & 0x40) != 0;
            state.l2 = if (b6 & 0x80) != 0 { 1.0 } else { 0.0 };

            let (raw_lx, raw_ly) = decode_12bit_stick(data[10], data[11], data[12]);
            let (raw_rx, raw_ry) = decode_12bit_stick(data[13], data[14], data[15]);
            let (lx, ly) = apply_radial_deadzone(
                normalize_12bit(raw_lx, &mut cal.lx_center),
                normalize_12bit(raw_ly, &mut cal.ly_center),
                cfg.left_deadzone,
            );
            let (rx, ry) = apply_radial_deadzone(
                normalize_12bit(raw_rx, &mut cal.rx_center),
                normalize_12bit(raw_ry, &mut cal.ry_center),
                cfg.right_deadzone,
            );
            state.left_x = lx;
            state.left_y = ly;
            state.right_x = rx;
            state.right_y = ry;
        }
    }

    // Optical mouse sensor (bytes 15..20: [flag, x_lo, x_hi, y_lo, y_hi])
    let mut mouse_delta = (0i16, 0i16);
    if data.len() >= 20 {
        let cur_x = u16::from_le_bytes([data[16], data[17]]);
        let cur_y = u16::from_le_bytes([data[18], data[19]]);
        if let (Some(px), Some(py)) = (cal.prev_opt_x, cal.prev_opt_y) {
            let dx = cur_x.wrapping_sub(px) as i16;
            let dy = cur_y.wrapping_sub(py) as i16;
            if dx.abs() <= 400 && dy.abs() <= 400 {
                mouse_delta = (dx, dy);
            }
        }
        cal.prev_opt_x = Some(cur_x);
        cal.prev_opt_y = Some(cur_y);
    }

    state.apply_nintendo_layout(cfg.nintendo_button_layout);
    Some((state, mouse_delta))
}
