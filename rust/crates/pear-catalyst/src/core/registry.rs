//! Module registry: decides once per process which PearRuntime modules run.
//!
//!   PEAR_RUNTIME_DISABLE=services,graphics   # comma-separated module names
//!   PEAR_RUNTIME_ONLY=core,input              # allow-list (core is implied)
//!
//! `graphics` owns shared geometry; `graphics-gl` / `graphics-metal` are its
//! backends: selecting graphics includes both, disabling graphics disables
//! both, selecting a child includes the shared geometry but not the sibling.

use std::ffi::{c_char, c_int, CStr, CString};
use std::sync::OnceLock;

const MODULES: [&str; 9] = [
    "core", "input", "graphics", "device", "storage", "services", "performance", "graphics-gl", "graphics-metal",
];

struct Registry {
    enabled: [bool; MODULES.len()],
    list: CString,
}

fn list_contains(list: Option<&str>, name: &str) -> bool {
    match list {
        None | Some("") => false,
        Some(l) => l.split([',', ' ']).any(|item| item == name),
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| {
        let disabled = env("PEAR_RUNTIME_DISABLE");
        let only = env("PEAR_RUNTIME_ONLY");
        let (disabled, only) = (disabled.as_deref(), only.as_deref());
        let has_only = matches!(only, Some(o) if !o.is_empty());
        let mut enabled = [true; MODULES.len()];
        for (i, &name) in MODULES.iter().enumerate().skip(1) {
            let mut on = true;
            if has_only {
                on = list_contains(only, name);
            }
            if name == "graphics" && has_only {
                on = list_contains(only, "graphics")
                    || list_contains(only, "graphics-gl")
                    || list_contains(only, "graphics-metal");
            }
            if name.starts_with("graphics-") {
                on = (!has_only || list_contains(only, "graphics") || list_contains(only, name))
                    && !list_contains(disabled, "graphics");
            }
            if list_contains(disabled, name) {
                on = false;
            }
            enabled[i] = on;
        }
        let names: Vec<&str> = MODULES.iter().zip(enabled).filter(|(_, on)| *on).map(|(n, _)| *n).collect();
        Registry { enabled, list: CString::new(names.join(",")).unwrap_or_default() }
    })
}

/// Unknown names are not gated.
pub fn enabled(name: &str) -> bool {
    let r = registry();
    MODULES.iter().position(|&m| m == name).map(|i| r.enabled[i]).unwrap_or(true)
}

#[no_mangle]
pub unsafe extern "C" fn PearRuntimeModuleEnabled(name: *const c_char) -> c_int {
    if name.is_null() {
        return 1;
    }
    enabled(CStr::from_ptr(name).to_str().unwrap_or("")) as c_int
}

#[no_mangle]
pub extern "C" fn PearRuntimeEnabledModules() -> *const c_char {
    registry().list.as_ptr()
}

#[no_mangle]
pub extern "C" fn PearRuntimeVersion() -> *const c_char {
    static V: OnceLock<CString> = OnceLock::new();
    V.get_or_init(|| CString::new(option_env!("PEAR_RUNTIME_VERSION").unwrap_or("dev")).unwrap_or_default())
        .as_ptr()
}
