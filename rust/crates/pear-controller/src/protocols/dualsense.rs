//! Sony DualSense / DualShock 4 raw HID report parser (USB & Bluetooth).

use crate::types::{
    apply_radial_deadzone, apply_trigger_deadzone, ControllerConfig, ControllerKind, ControllerState,
};

pub const SONY_VID: u16 = 0x054C;
pub const PID_DUALSENSE: u16 = 0x0CE6;
pub const PID_DUALSENSE_EDGE: u16 = 0x0DF2;
pub const PID_DS4_V1: u16 = 0x05C4;
pub const PID_DS4_V2: u16 = 0x09CC;

pub fn classify_sony_pid(pid: u16) -> Option<ControllerKind> {
    match pid {
        PID_DUALSENSE | PID_DUALSENSE_EDGE => Some(ControllerKind::DualSense),
        PID_DS4_V1 | PID_DS4_V2 => Some(ControllerKind::DualShock4),
        _ => None,
    }
}

fn norm_axis_u8(v: u8) -> f32 {
    ((v as f32) - 127.5) / 127.5
}

fn decode_hat(hat: u8) -> (bool, bool, bool, bool) {
    match hat & 0x0F {
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
}

pub fn parse_sony_report(
    kind: ControllerKind,
    data: &[u8],
    cfg: &ControllerConfig,
) -> Option<ControllerState> {
    if data.is_empty() {
        return None;
    }

    let offset = match (kind, data[0]) {
        (ControllerKind::DualSense, 0x01) if data.len() >= 11 => 1,
        (ControllerKind::DualSense, 0x31) if data.len() >= 12 => 2,
        (ControllerKind::DualShock4, 0x01) if data.len() >= 10 => 1,
        (ControllerKind::DualShock4, 0x11) if data.len() >= 12 => 3,
        _ => return None,
    };

    let p = &data[offset..];
    if p.len() < 10 {
        return None;
    }

    let (lx, ly) = apply_radial_deadzone(norm_axis_u8(p[0]), -norm_axis_u8(p[1]), cfg.left_deadzone);
    let (rx, ry) = apply_radial_deadzone(norm_axis_u8(p[2]), -norm_axis_u8(p[3]), cfg.right_deadzone);

    let (l2_raw, r2_raw, b0, b1, b2) = if kind == ControllerKind::DualSense {
        (p[4], p[5], p[7], p[8], p[9])
    } else {
        (p[7], p[8], p[4], p[5], p[6])
    };

    let (dpad_up, dpad_down, dpad_left, dpad_right) = decode_hat(b0);

    Some(ControllerState {
        x: (b0 & 0x10) != 0, // Square
        a: (b0 & 0x20) != 0, // Cross
        b: (b0 & 0x40) != 0, // Circle
        y: (b0 & 0x80) != 0, // Triangle
        l1: (b1 & 0x01) != 0,
        r1: (b1 & 0x02) != 0,
        l2: apply_trigger_deadzone((l2_raw as f32) / 255.0, cfg.trigger_deadzone),
        r2: apply_trigger_deadzone((r2_raw as f32) / 255.0, cfg.trigger_deadzone),
        options: (b1 & 0x10) != 0,
        menu: (b1 & 0x20) != 0,
        l3: (b1 & 0x40) != 0,
        r3: (b1 & 0x80) != 0,
        home: (b2 & 0x01) != 0,
        capture: (b2 & 0x02) != 0,
        dpad_up,
        dpad_down,
        dpad_left,
        dpad_right,
        left_x: lx,
        left_y: ly,
        right_x: rx,
        right_y: ry,
        c_button: false,
        gl: false,
        gr: false,
    })
}
