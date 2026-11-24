//! Nintendo Switch 2 (Joy-Con 2 & Pro Controller 2) BLE GATT protocol parser.

use crate::types::{
    apply_radial_deadzone, ControllerConfig, ControllerKind, ControllerState,
};

pub const NINTENDO_VID: u16 = 0x057E;
pub const PID_JOYCON2_LEFT: u16 = 0x2066;
pub const PID_JOYCON2_RIGHT: u16 = 0x2067;
pub const PID_PRO2: u16 = 0x2069;
pub const PID_GAMECUBE2: u16 = 0x2073;

pub const SWITCH2_SERVICE_UUID: &str = "AB7DE9BE-89FE-49AD-828F-118F09DF7FD0";
pub const SWITCH2_CMD_CHAR_UUID: &str = "AB7DE9BE-89FE-49AD-828F-118F09DF7FD1";
pub const SWITCH2_COMMON_INPUT_UUID: &str = "AB7DE9BE-89FE-49AD-828F-118F09DF7FD2";
pub const SWITCH2_JC2_L_INPUT_UUID: &str = "CC1BBBB5-7354-4D32-A716-A81CB241A32A";
pub const SWITCH2_JC2_R_INPUT_UUID: &str = "D5A9E01E-2FFC-4CCA-B20C-8B67142BF442";
pub const SWITCH2_PRO2_INPUT_UUID: &str = "7492866C-EC3E-4619-8258-32755FFCC0F8";

pub fn is_switch2_input_char(uuid: &str) -> bool {
    let u = uuid.to_ascii_uppercase();
    u == SWITCH2_COMMON_INPUT_UUID
        || u == SWITCH2_JC2_L_INPUT_UUID
        || u == SWITCH2_JC2_R_INPUT_UUID
        || u == SWITCH2_PRO2_INPUT_UUID
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
        PID_JOYCON2_LEFT => Some(ControllerKind::Switch2JoyConLeft),
        PID_JOYCON2_RIGHT => Some(ControllerKind::Switch2JoyConRight),
        PID_PRO2 | PID_GAMECUBE2 => Some(ControllerKind::Switch2Pro),
        _ => None,
    }
}

/// Inspect BLE manufacturer data for Nintendo `0x057E` and a Switch 2 PID.
pub fn parse_manufacturer_data(data: &[u8]) -> Option<(u16, ControllerKind)> {
    if data.len() < 4 {
        return None;
    }
    let vid = u16::from_le_bytes([data[0], data[1]]);
    if vid != NINTENDO_VID {
        return None;
    }
    // Check common offsets where Switch 2 BLE advertisement places the 16-bit PID.
    for window in data[2..].windows(2) {
        let pid = u16::from_le_bytes([window[0], window[1]]);
        if let Some(kind) = classify_by_pid(pid) {
            return Some((pid, kind));
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

/// Neutral keep-alive / rumble init frame for Switch 2 BLE command characteristic (`...fd1`).
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

fn decode_12bit_stick(b0: u8, b1: u8, b2: u8) -> (f32, f32) {
    let raw_x = (b0 as u16) | (((b1 & 0x0F) as u16) << 8);
    let raw_y = ((b1 >> 4) as u16) | ((b2 as u16) << 4);
    let nx = ((raw_x as f32) - 2048.0) / 1600.0;
    let ny = ((raw_y as f32) - 2048.0) / 1600.0;
    (nx.clamp(-1.0, 1.0), ny.clamp(-1.0, 1.0))
}

/// Parse a Switch 2 BLE input notification payload (typically 63 or 64 bytes).
pub fn parse_switch2_ble_report(
    kind: ControllerKind,
    data: &[u8],
    cfg: &ControllerConfig,
) -> Option<ControllerState> {
    if data.len() < 16 {
        return None;
    }

    // Standard Switch 2 63/64-byte BLE report packs 32-bit button bitfield at [4..8]
    // and 12-bit dual sticks at [10..16] (Left: 10..13, Right: 13..16).
    let btn = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let (raw_lx, raw_ly) = decode_12bit_stick(data[10], data[11], data[12]);
    let (raw_rx, raw_ry) = decode_12bit_stick(data[13], data[14], data[15]);

    let (lx, ly) = apply_radial_deadzone(raw_lx, raw_ly, cfg.left_deadzone);
    let (rx, ry) = apply_radial_deadzone(raw_rx, raw_ry, cfg.right_deadzone);

    let mut state = ControllerState {
        // Right-side / face buttons (lower word)
        y: (btn & (1 << 0)) != 0,
        x: (btn & (1 << 1)) != 0,
        b: (btn & (1 << 2)) != 0,
        a: (btn & (1 << 3)) != 0,
        r1: (btn & (1 << 6)) != 0,
        r2: if (btn & (1 << 7)) != 0 { 1.0 } else { 0.0 },
        // Shared / system buttons
        options: (btn & (1 << 8)) != 0,  // Minus
        menu: (btn & (1 << 9)) != 0,     // Plus
        r3: (btn & (1 << 10)) != 0,
        l3: (btn & (1 << 11)) != 0,
        home: (btn & (1 << 12)) != 0,
        capture: (btn & (1 << 13)) != 0,
        c_button: (btn & (1 << 14)) != 0,
        // Left-side / D-pad
        dpad_down: (btn & (1 << 16)) != 0,
        dpad_up: (btn & (1 << 17)) != 0,
        dpad_right: (btn & (1 << 18)) != 0,
        dpad_left: (btn & (1 << 19)) != 0,
        l1: (btn & (1 << 22)) != 0,
        l2: if (btn & (1 << 23)) != 0 { 1.0 } else { 0.0 },
        // Grip / SL / SR extras
        gl: (btn & (1 << 24)) != 0 || (btn & (1 << 21)) != 0,
        gr: (btn & (1 << 25)) != 0 || (btn & (1 << 5)) != 0,
        left_x: lx,
        left_y: ly,
        right_x: rx,
        right_y: ry,
    };

    if kind == ControllerKind::Switch2JoyConRight {
        // Right Joy-Con carries its stick in the right slot in common reports,
        // or in bytes 10..13 on single-Joy-Con reports.
        if state.right_x == 0.0 && state.right_y == 0.0 && (state.left_x != 0.0 || state.left_y != 0.0) {
            state.right_x = state.left_x;
            state.right_y = state.left_y;
            state.left_x = 0.0;
            state.left_y = 0.0;
        }
    }

    state.apply_nintendo_layout(cfg.nintendo_button_layout);
    Some(state)
}
