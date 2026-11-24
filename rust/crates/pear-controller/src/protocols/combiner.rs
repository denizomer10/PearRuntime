//! Combines Left + Right Joy-Cons (Switch 1 or Switch 2) or adapts single horizontal Joy-Cons.

use crate::types::{
    ControllerConfig, ControllerDevice, ControllerKind, ControllerState, JoyConCombineMode,
    TransportKind,
};

fn rotate_left_joycon_horizontal(src: &ControllerState) -> ControllerState {
    // Rotate left Joy-Con 90 degrees clockwise for horizontal single-pad play.
    let mut s = *src;
    s.left_x = src.left_y;
    s.left_y = -src.left_x;
    s.a = src.dpad_down;
    s.b = src.dpad_right;
    s.x = src.dpad_left;
    s.y = src.dpad_up;
    s.dpad_up = false;
    s.dpad_down = false;
    s.dpad_left = false;
    s.dpad_right = false;
    s.l1 = src.gl; // SL
    s.r1 = src.gr || src.l1; // SR
    s.menu = src.options;
    s
}

fn rotate_right_joycon_horizontal(src: &ControllerState) -> ControllerState {
    // Rotate right Joy-Con 90 degrees counter-clockwise for horizontal single-pad play.
    let mut s = *src;
    s.left_x = -src.right_y;
    s.left_y = src.right_x;
    s.right_x = 0.0;
    s.right_y = 0.0;
    s.a = src.x;
    s.b = src.a;
    s.x = src.y;
    s.y = src.b;
    s.l1 = src.gl || src.r1;
    s.r1 = src.gr;
    s
}

fn merge_pair_state(left: &ControllerState, right: &ControllerState) -> ControllerState {
    ControllerState {
        a: right.a,
        b: right.b,
        x: right.x,
        y: right.y,
        l1: left.l1,
        r1: right.r1,
        l2: left.l2,
        r2: right.r2,
        l3: left.l3,
        r3: right.r3,
        dpad_up: left.dpad_up,
        dpad_down: left.dpad_down,
        dpad_left: left.dpad_left,
        dpad_right: left.dpad_right,
        menu: right.menu,
        options: left.options,
        home: right.home,
        capture: left.capture,
        c_button: right.c_button,
        gl: left.gl,
        gr: right.gr,
        left_x: left.left_x,
        left_y: left.left_y,
        right_x: right.right_x,
        right_y: right.right_y,
    }
}

pub fn build_effective_devices(
    raw: &[ControllerDevice],
    cfg: &ControllerConfig,
) -> Vec<ControllerDevice> {
    let mut result = Vec::with_capacity(raw.len());

    let jc2_l = raw.iter().find(|d| d.connected && d.kind == ControllerKind::Switch2JoyConLeft);
    let jc2_r = raw.iter().find(|d| d.connected && d.kind == ControllerKind::Switch2JoyConRight);
    let jc1_l = raw.iter().find(|d| d.connected && d.kind == ControllerKind::Switch1JoyConLeft);
    let jc1_r = raw.iter().find(|d| d.connected && d.kind == ControllerKind::Switch1JoyConRight);

    let pair_jc2 = cfg.joycon_combine_mode == JoyConCombineMode::Auto && jc2_l.is_some() && jc2_r.is_some();
    let pair_jc1 = cfg.joycon_combine_mode == JoyConCombineMode::Auto && jc1_l.is_some() && jc1_r.is_some();

    if let (true, Some(l), Some(r)) = (pair_jc2, jc2_l, jc2_r) {
        result.push(ControllerDevice {
            id: format!("pair:{}+{}", l.id, r.id),
            name: "Nintendo Joy-Con 2 (L+R)".to_string(),
            kind: ControllerKind::Switch2JoyConPair,
            transport: TransportKind::Combined,
            vendor_id: l.vendor_id,
            product_id: l.product_id,
            battery_percent: match (l.battery_percent, r.battery_percent) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                _ => None,
            },
            latency_us: l.latency_us.max(r.latency_us),
            report_rate_hz: l.report_rate_hz.min(r.report_rate_hz),
            connected: true,
            state: merge_pair_state(&l.state, &r.state),
        });
    }

    if let (true, Some(l), Some(r)) = (pair_jc1, jc1_l, jc1_r) {
        result.push(ControllerDevice {
            id: format!("pair:{}+{}", l.id, r.id),
            name: "Nintendo Joy-Con (L+R)".to_string(),
            kind: ControllerKind::Switch1JoyConPair,
            transport: TransportKind::Combined,
            vendor_id: l.vendor_id,
            product_id: l.product_id,
            battery_percent: match (l.battery_percent, r.battery_percent) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                _ => None,
            },
            latency_us: l.latency_us.max(r.latency_us),
            report_rate_hz: l.report_rate_hz.min(r.report_rate_hz),
            connected: true,
            state: merge_pair_state(&l.state, &r.state),
        });
    }

    for dev in raw {
        if !dev.connected {
            continue;
        }
        if pair_jc2 && matches!(dev.kind, ControllerKind::Switch2JoyConLeft | ControllerKind::Switch2JoyConRight) {
            continue;
        }
        if pair_jc1 && matches!(dev.kind, ControllerKind::Switch1JoyConLeft | ControllerKind::Switch1JoyConRight) {
            continue;
        }
        match (cfg.joycon_combine_mode, dev.kind) {
            (JoyConCombineMode::SingleLeft, ControllerKind::Switch2JoyConRight | ControllerKind::Switch1JoyConRight) => continue,
            (JoyConCombineMode::SingleRight, ControllerKind::Switch2JoyConLeft | ControllerKind::Switch1JoyConLeft) => continue,
            _ => {}
        }

        let mut cloned = dev.clone();
        if matches!(dev.kind, ControllerKind::Switch2JoyConLeft | ControllerKind::Switch1JoyConLeft)
            && cfg.joycon_combine_mode != JoyConCombineMode::SingleLeft
        {
            cloned.state = rotate_left_joycon_horizontal(&dev.state);
        } else if matches!(dev.kind, ControllerKind::Switch2JoyConRight | ControllerKind::Switch1JoyConRight)
            && cfg.joycon_combine_mode != JoyConCombineMode::SingleRight
        {
            cloned.state = rotate_right_joycon_horizontal(&dev.state);
        }
        result.push(cloned);
    }

    result
}
