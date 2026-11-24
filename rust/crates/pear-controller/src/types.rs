//! Core types, configuration, and JSON serialization for `pear-controller`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerKind {
    Switch2JoyConLeft,
    Switch2JoyConRight,
    Switch2JoyConPair,
    Switch2Pro,
    Switch1JoyConLeft,
    Switch1JoyConRight,
    Switch1JoyConPair,
    Switch1Pro,
    DualSense,
    DualShock4,
    Xbox,
    GenericHID,
}

impl ControllerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Switch2JoyConLeft => "switch2_joycon_l",
            Self::Switch2JoyConRight => "switch2_joycon_r",
            Self::Switch2JoyConPair => "switch2_joycon_pair",
            Self::Switch2Pro => "switch2_pro",
            Self::Switch1JoyConLeft => "switch1_joycon_l",
            Self::Switch1JoyConRight => "switch1_joycon_r",
            Self::Switch1JoyConPair => "switch1_joycon_pair",
            Self::Switch1Pro => "switch1_pro",
            Self::DualSense => "dualsense",
            Self::DualShock4 => "dualshock4",
            Self::Xbox => "xbox",
            Self::GenericHID => "generic_hid",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Switch2JoyConLeft => "Joy-Con 2 (L)",
            Self::Switch2JoyConRight => "Joy-Con 2 (R)",
            Self::Switch2JoyConPair => "Joy-Con 2 (L+R)",
            Self::Switch2Pro => "Switch 2 Pro Controller",
            Self::Switch1JoyConLeft => "Joy-Con (L)",
            Self::Switch1JoyConRight => "Joy-Con (R)",
            Self::Switch1JoyConPair => "Joy-Con (L+R)",
            Self::Switch1Pro => "Switch Pro Controller",
            Self::DualSense => "DualSense",
            Self::DualShock4 => "DualShock 4",
            Self::Xbox => "Xbox Controller",
            Self::GenericHID => "HID Controller",
        }
    }

    pub fn is_nintendo(self) -> bool {
        matches!(
            self,
            Self::Switch2JoyConLeft
                | Self::Switch2JoyConRight
                | Self::Switch2JoyConPair
                | Self::Switch2Pro
                | Self::Switch1JoyConLeft
                | Self::Switch1JoyConRight
                | Self::Switch1JoyConPair
                | Self::Switch1Pro
        )
    }

    pub fn is_switch2_joycon_side(self) -> Option<JoyConSide> {
        match self {
            Self::Switch2JoyConLeft => Some(JoyConSide::Left),
            Self::Switch2JoyConRight => Some(JoyConSide::Right),
            _ => None,
        }
    }

    pub fn is_switch1_joycon_side(self) -> Option<JoyConSide> {
        match self {
            Self::Switch1JoyConLeft => Some(JoyConSide::Left),
            Self::Switch1JoyConRight => Some(JoyConSide::Right),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoyConSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    Ble,
    BluetoothHid,
    UsbHid,
    Combined,
}

impl TransportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ble => "BLE",
            Self::BluetoothHid => "Bluetooth HID",
            Self::UsbHid => "USB HID",
            Self::Combined => "Combined",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoyConCombineMode {
    Auto,
    Separate,
    SingleLeft,
    SingleRight,
}

impl JoyConCombineMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "separate" => Self::Separate,
            "single_left" => Self::SingleLeft,
            "single_right" => Self::SingleRight,
            _ => Self::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Separate => "separate",
            Self::SingleLeft => "single_left",
            Self::SingleRight => "single_right",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControllerConfig {
    pub low_latency_mode: bool,
    pub polling_hz: u32,
    pub left_deadzone: f32,
    pub right_deadzone: f32,
    pub trigger_deadzone: f32,
    pub joycon_combine_mode: JoyConCombineMode,
    pub nintendo_button_layout: bool,
    pub ble_switch2_enabled: bool,
    pub prefer_direct_hid: bool,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            low_latency_mode: true,
            polling_hz: 250,
            left_deadzone: 0.06,
            right_deadzone: 0.06,
            trigger_deadzone: 0.03,
            joycon_combine_mode: JoyConCombineMode::Auto,
            nintendo_button_layout: false,
            ble_switch2_enabled: true,
            prefer_direct_hid: true,
        }
    }
}

impl ControllerConfig {
    pub fn from_json(json: &str) -> Self {
        let mut cfg = Self::default();
        if let Some(v) = extract_bool(json, "lowLatencyMode") {
            cfg.low_latency_mode = v;
        }
        if let Some(v) = extract_u32(json, "pollingHz") {
            cfg.polling_hz = v.clamp(60, 1000);
        }
        if let Some(v) = extract_f32(json, "leftDeadzone") {
            cfg.left_deadzone = v.clamp(0.0, 0.45);
        }
        if let Some(v) = extract_f32(json, "rightDeadzone") {
            cfg.right_deadzone = v.clamp(0.0, 0.45);
        }
        if let Some(v) = extract_f32(json, "triggerDeadzone") {
            cfg.trigger_deadzone = v.clamp(0.0, 0.35);
        }
        if let Some(v) = extract_str(json, "joyconCombineMode") {
            cfg.joycon_combine_mode = JoyConCombineMode::from_str(&v);
        }
        if let Some(v) = extract_bool(json, "nintendoButtonLayout") {
            cfg.nintendo_button_layout = v;
        }
        if let Some(v) = extract_bool(json, "bleSwitch2Enabled") {
            cfg.ble_switch2_enabled = v;
        }
        if let Some(v) = extract_bool(json, "preferDirectHID") {
            cfg.prefer_direct_hid = v;
        }
        cfg
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControllerState {
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub l1: bool,
    pub r1: bool,
    pub l2: f32,
    pub r2: f32,
    pub l3: bool,
    pub r3: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub menu: bool,    // Plus / Start / Options
    pub options: bool, // Minus / Select / Create / Share
    pub home: bool,    // Home / PS / Xbox
    pub capture: bool, // Capture / Touchpad / Mute / C
    pub c_button: bool,
    pub gl: bool,
    pub gr: bool,
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
}

impl ControllerState {
    /// Apply Nintendo A/B & X/Y swap when `nintendo_button_layout == false`
    /// so Nintendo face buttons match standard Xbox / iOS GCExtendedGamepad positions.
    pub fn apply_nintendo_layout(&mut self, keep_nintendo_labels: bool) {
        if !keep_nintendo_labels {
            std::mem::swap(&mut self.a, &mut self.b);
            std::mem::swap(&mut self.x, &mut self.y);
        }
    }

    pub fn pressed_buttons_json(&self) -> String {
        let mut names = Vec::with_capacity(16);
        if self.a { names.push("\"A\""); }
        if self.b { names.push("\"B\""); }
        if self.x { names.push("\"X\""); }
        if self.y { names.push("\"Y\""); }
        if self.l1 { names.push("\"L1\""); }
        if self.r1 { names.push("\"R1\""); }
        if self.l2 > 0.25 { names.push("\"L2\""); }
        if self.r2 > 0.25 { names.push("\"R2\""); }
        if self.l3 { names.push("\"L3\""); }
        if self.r3 { names.push("\"R3\""); }
        if self.dpad_up { names.push("\"Up\""); }
        if self.dpad_down { names.push("\"Down\""); }
        if self.dpad_left { names.push("\"Left\""); }
        if self.dpad_right { names.push("\"Right\""); }
        if self.menu { names.push("\"Menu\""); }
        if self.options { names.push("\"Options\""); }
        if self.home { names.push("\"Home\""); }
        if self.capture { names.push("\"Capture\""); }
        if self.c_button { names.push("\"C\""); }
        if self.gl { names.push("\"GL\""); }
        if self.gr { names.push("\"GR\""); }
        format!("[{}]", names.join(","))
    }
}

#[derive(Clone, Debug)]
pub struct ControllerDevice {
    pub id: String,
    pub name: String,
    pub kind: ControllerKind,
    pub transport: TransportKind,
    pub vendor_id: u16,
    pub product_id: u16,
    pub battery_percent: Option<u8>,
    pub latency_us: u64,
    pub report_rate_hz: u32,
    pub connected: bool,
    pub state: ControllerState,
}

impl ControllerDevice {
    pub fn to_json(&self) -> String {
        let battery = match self.battery_percent {
            Some(b) => b.to_string(),
            None => "null".to_string(),
        };
        format!(
            concat!(
                "{{\"id\":\"{}\",\"name\":\"{}\",\"kind\":\"{}\",\"kindDisplay\":\"{}\",",
                "\"transport\":\"{}\",\"vendorId\":{},\"productId\":{},",
                "\"batteryPercent\":{},\"latencyUs\":{},\"reportRateHz\":{},\"connected\":{},",
                "\"state\":{{\"leftX\":{:.4},\"leftY\":{:.4},\"rightX\":{:.4},\"rightY\":{:.4},",
                "\"l2\":{:.4},\"r2\":{:.4},\"buttons\":{}}}}}"
            ),
            escape_json(&self.id),
            escape_json(&self.name),
            self.kind.as_str(),
            escape_json(self.kind.display_name()),
            self.transport.as_str(),
            self.vendor_id,
            self.product_id,
            battery,
            self.latency_us,
            self.report_rate_hz,
            self.connected,
            self.state.left_x,
            self.state.left_y,
            self.state.right_x,
            self.state.right_y,
            self.state.l2,
            self.state.r2,
            self.state.pressed_buttons_json(),
        )
    }
}

pub fn apply_radial_deadzone(x: f32, y: f32, deadzone: f32) -> (f32, f32) {
    let x = x.clamp(-1.0, 1.0);
    let y = y.clamp(-1.0, 1.0);
    let dz = deadzone.clamp(0.0, 0.45);
    let mag = (x * x + y * y).sqrt();
    if mag <= dz || mag < 1e-6 {
        return (0.0, 0.0);
    }
    let scaled = ((mag - dz) / (1.0 - dz)).min(1.0);
    let factor = scaled / mag;
    ((x * factor).clamp(-1.0, 1.0), (y * factor).clamp(-1.0, 1.0))
}

pub fn apply_trigger_deadzone(v: f32, deadzone: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    let dz = deadzone.clamp(0.0, 0.35);
    if v <= dz {
        0.0
    } else {
        ((v - dz) / (1.0 - dz)).clamp(0.0, 1.0)
    }
}

pub fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn find_json_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\"", key);
    let idx = json.find(&needle)?;
    let rest = &json[idx + needle.len()..];
    let colon = rest.find(':')?;
    Some(rest[colon + 1..].trim_start())
}

fn extract_bool(json: &str, key: &str) -> Option<bool> {
    let v = find_json_value(json, key)?;
    if v.starts_with("true") {
        Some(true)
    } else if v.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_u32(json: &str, key: &str) -> Option<u32> {
    let v = find_json_value(json, key)?;
    let end = v
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(v.len());
    v[..end].parse().ok()
}

fn extract_f32(json: &str, key: &str) -> Option<f32> {
    let v = find_json_value(json, key)?;
    let end = v
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(v.len());
    v[..end].parse().ok()
}

fn extract_str(json: &str, key: &str) -> Option<String> {
    let v = find_json_value(json, key)?;
    if !v.starts_with('"') {
        return None;
    }
    let rest = &v[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
