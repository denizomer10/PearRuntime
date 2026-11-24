//! Nintendo Switch 1 (Pro Controller & Joy-Con L/R) HID report parser.

use crate::types::{
    apply_radial_deadzone, ControllerConfig, ControllerKind, ControllerState,
};

pub const PID_JOYCON1_LEFT: u16 = 0x2006;
pub const PID_JOYCON1_RIGHT: u16 = 0x2007;
pub const PID_PRO1: u16 = 0x2009;
pub const PID_CHARGING_GRIP: u16 = 0x200E;

pub fn classify_switch1_pid(pid: u16) -> Option<ControllerKind> {
    match pid {
        PID_JOYCON1_LEFT => Some(ControllerKind::Switch1JoyConLeft),
        PID_JOYCON1_RIGHT => Some(ControllerKind::Switch1JoyConRight),
        PID_PRO1 | PID_CHARGING_GRIP => Some(ControllerKind::Switch1Pro),
        _ => None,
    }
}

/// Output report to switch Switch 1 Pro / Joy-Con into `0x30` full-report mode.
pub fn switch1_full_mode_output_report(seq: u8) -> [u8; 12] {
    [
        0x01,
        seq & 0x0F,
        0x00,
        0x01,
        0x40,
        0x40,
        0x00,
        0x01,
        0x40,
        0x40,
        0x03,
        0x30,
    ]
}

fn decode_12bit_stick(b0: u8, b1: u8, b2: u8) -> (f32, f32) {
    let raw_x = (b0 as u16) | (((b1 & 0x0F) as u16) << 8);
    let raw_y = ((b1 >> 4) as u16) | ((b2 as u16) << 4);
    let nx = ((raw_x as f32) - 2048.0) / 1600.0;
    let ny = ((raw_y as f32) - 2048.0) / 1600.0;
    (nx.clamp(-1.0, 1.0), ny.clamp(-1.0, 1.0))
}

pub fn parse_switch1_report(
    kind: ControllerKind,
    data: &[u8],
    cfg: &ControllerConfig,
) -> Option<(ControllerState, Option<u8>)> {
    if data.is_empty() {
        return None;
    }
    if data[0] != 0x30 && data[0] != 0x21 {
        return None;
    }
    if data.len() < 12 {
        return None;
    }

    let battery_nibble = (data[2] >> 4) & 0x0E;
    let battery = match battery_nibble {
        8 => Some(100),
        6 => Some(75),
        4 => Some(50),
        2 => Some(25),
        _ => Some(10),
    };

    let b_right = data[3];
    let b_shared = data[4];
    let b_left = data[5];

    let (raw_lx, raw_ly) = decode_12bit_stick(data[6], data[7], data[8]);
    let (raw_rx, raw_ry) = decode_12bit_stick(data[9], data[10], data[11]);

    let (lx, ly) = apply_radial_deadzone(raw_lx, raw_ly, cfg.left_deadzone);
    let (rx, ry) = apply_radial_deadzone(raw_rx, raw_ry, cfg.right_deadzone);

    let mut state = ControllerState {
        y: (b_right & 0x01) != 0,
        x: (b_right & 0x02) != 0,
        b: (b_right & 0x04) != 0,
        a: (b_right & 0x08) != 0,
        r1: (b_right & 0x40) != 0,
        r2: if (b_right & 0x80) != 0 { 1.0 } else { 0.0 },
        options: (b_shared & 0x01) != 0,
        menu: (b_shared & 0x02) != 0,
        r3: (b_shared & 0x04) != 0,
        l3: (b_shared & 0x08) != 0,
        home: (b_shared & 0x10) != 0,
        capture: (b_shared & 0x20) != 0,
        dpad_down: (b_left & 0x01) != 0,
        dpad_up: (b_left & 0x02) != 0,
        dpad_right: (b_left & 0x04) != 0,
        dpad_left: (b_left & 0x08) != 0,
        l1: (b_left & 0x40) != 0,
        l2: if (b_left & 0x80) != 0 { 1.0 } else { 0.0 },
        gl: (b_left & 0x20) != 0,
        gr: (b_right & 0x10) != 0,
        left_x: lx,
        left_y: ly,
        right_x: rx,
        right_y: ry,
        c_button: false,
    };

    let _ = kind;
    state.apply_nintendo_layout(cfg.nintendo_button_layout);
    Some((state, battery))
}
