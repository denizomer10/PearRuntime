//! PearRuntime Core (Rust): thread-safe logging, missing-call tracking,
//! caller symbolication (`dladdr`), and JSONL report emission.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

mod forwarding;
pub use forwarding::PearMissingInstallForImage;

/// Zero result for any return convention that fits in registers (AAPCS64):
/// integers/pointers in x0-x1, floating point and HFA structs (CGRect,
/// CGPoint, CGSize, UIEdgeInsets) in d0-d3. IMP of unimplemented selectors and
/// tail of generated C function stubs.
#[unsafe(naked)]
#[no_mangle]
pub extern "C" fn pear_missing_zero() {
    core::arch::naked_asm!(
        "mov x0, #0",
        "mov x1, #0",
        "movi d0, #0",
        "movi d1, #0",
        "movi d2, #0",
        "movi d3, #0",
        "ret",
    )
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum PearMissingKind {
    ObjCInstance = 0,
    ObjCClass = 1,
    CFunction = 2,
    Behavior = 3,
}

impl PearMissingKind {
    fn from_raw(v: c_int) -> Self {
        match v {
            0 => Self::ObjCInstance,
            1 => Self::ObjCClass,
            2 => Self::CFunction,
            _ => Self::Behavior,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ObjCInstance => "objc-instance",
            Self::ObjCClass => "objc-class",
            Self::CFunction => "c-function",
            Self::Behavior => "behavior",
        }
    }
}

#[repr(C)]
struct DlInfo {
    dli_fname: *const c_char,
    dli_fbase: *const c_void,
    dli_sname: *const c_char,
    dli_saddr: *const c_void,
}

extern "C" {
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    fn pthread_main_np() -> c_int;
    fn atexit(func: extern "C" fn()) -> c_int;
    fn abort() -> !;
}

unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

// ---------------------------------------------------------------------------
// Logging

static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

fn write_log_line(area: Option<&str>, message: &str) {
    let lock = LOG_FILE.get_or_init(|| {
        let file = std::env::var_os("PEAR_LOG_FILE")
            .filter(|v| !v.is_empty())
            .and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok());
        Mutex::new(file)
    });

    let line = match area {
        Some("") => format!("[PearRuntime] {message}\n"),
        Some(tag) => format!("[PearRuntime][{tag}] {message}\n"),
        None => format!("{message}\n"),
    };

    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    let _ = std::io::stderr().write_all(line.as_bytes());
    if let Some(ref mut f) = *guard {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

#[no_mangle]
pub extern "C" fn PearHostLogMessage(area: *const c_char, message: *const c_char) {
    let area_str = unsafe { cstr_to_str(area) };
    let msg_str = unsafe { cstr_to_str(message) }.unwrap_or("");
    write_log_line(area_str, msg_str);
}

// ---------------------------------------------------------------------------
// Missing-call reporting

struct MissingEntry {
    kind: PearMissingKind,
    key: String,
    caller: String,
    count: u64,
}

struct MissingState {
    entries: Vec<MissingEntry>,
    index: HashMap<(PearMissingKind, String), usize>,
    report: Option<File>,
    report_opened: bool,
}

static MISSING: OnceLock<Mutex<MissingState>> = OnceLock::new();
static SUMMARY_WRITTEN: AtomicBool = AtomicBool::new(false);

fn missing_mutex() -> &'static Mutex<MissingState> {
    MISSING.get_or_init(|| {
        Mutex::new(MissingState {
            entries: Vec::new(),
            index: HashMap::new(),
            report: None,
            report_opened: false,
        })
    })
}

fn describe_caller(caller: *const c_void) -> String {
    if caller.is_null() {
        return "0x0".to_string();
    }
    let mut info = DlInfo {
        dli_fname: std::ptr::null(),
        dli_fbase: std::ptr::null(),
        dli_sname: std::ptr::null(),
        dli_saddr: std::ptr::null(),
    };
    let ok = unsafe { dladdr(caller, &mut info) };
    if ok != 0 && !info.dli_fname.is_null() {
        if let Some(fname) = unsafe { cstr_to_str(info.dli_fname) } {
            let base = fname.rsplit('/').next().unwrap_or(fname);
            let offset = (caller as usize).saturating_sub(info.dli_fbase as usize);
            return format!("{base}+0x{offset:x}");
        }
    }
    format!("{caller:p}")
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[no_mangle]
pub extern "C" fn PearMissingWriteSummary() {
    if SUMMARY_WRITTEN.swap(true, Ordering::SeqCst) {
        return;
    }
    let count = {
        let mut guard = missing_mutex().lock().unwrap_or_else(|e| e.into_inner());
        let MissingState { entries, report, .. } = &mut *guard;
        if let Some(ref mut f) = report {
            for e in entries.iter() {
                let k = json_escape(&e.key);
                let c = json_escape(&e.caller);
                let _ = writeln!(
                    f,
                    "{{\"event\":\"summary\",\"kind\":\"{}\",\"api\":\"{}\",\"count\":{},\"first_caller\":\"{}\"}}",
                    e.kind.as_str(),
                    k,
                    e.count,
                    c
                );
            }
            let _ = f.flush();
        }
        entries.len()
    };
    write_log_line(
        Some("missing"),
        &format!("{count} distinct missing APIs were called in this run"),
    );
}

#[no_mangle]
pub extern "C" fn PearMissingReport(
    kind: c_int,
    owner: *const c_char,
    name: *const c_char,
    caller: *const c_void,
) {
    let kind_enum = PearMissingKind::from_raw(kind);
    let owner_str = unsafe { cstr_to_str(owner) };
    let name_str = unsafe { cstr_to_str(name) }.unwrap_or("?");

    let key = match kind_enum {
        PearMissingKind::ObjCInstance => format!("-[{} {}]", owner_str.unwrap_or("?"), name_str),
        PearMissingKind::ObjCClass => format!("+[{} {}]", owner_str.unwrap_or("?"), name_str),
        PearMissingKind::CFunction => format!("{name_str}()"),
        PearMissingKind::Behavior => format!("{}: {}", owner_str.unwrap_or("?"), name_str),
    };

    let where_str;
    {
        let mut guard = missing_mutex().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&idx) = guard.index.get(&(kind_enum, key.clone())) {
            guard.entries[idx].count += 1;
            return;
        }
        where_str = describe_caller(caller);
        let idx = guard.entries.len();
        guard.index.insert((kind_enum, key.clone()), idx);
        guard.entries.push(MissingEntry {
            kind: kind_enum,
            key: key.clone(),
            caller: where_str.clone(),
            count: 1,
        });

        if !guard.report_opened {
            guard.report_opened = true;
            guard.report = std::env::var_os("PEAR_MISSING_REPORT")
                .filter(|v| !v.is_empty())
                .and_then(|path| File::create(path).ok());
            unsafe {
                atexit(PearMissingWriteSummary);
            }
        }

        if let Some(ref mut f) = guard.report {
            let k = json_escape(&key);
            let c = json_escape(&where_str);
            let is_main = unsafe { pthread_main_np() != 0 };
            let _ = writeln!(
                f,
                "{{\"event\":\"first\",\"kind\":\"{}\",\"api\":\"{}\",\"caller\":\"{}\",\"thread_main\":{}}}",
                kind_enum.as_str(),
                k,
                c,
                if is_main { "true" } else { "false" }
            );
            let _ = f.flush();
        }
    }

    write_log_line(
        Some("missing"),
        &format!("{} {} caller={}", kind_enum.as_str(), key, where_str),
    );
    if std::env::var_os("PEAR_MISSING_FATAL").as_deref() == Some(std::ffi::OsStr::new("1")) {
        write_log_line(Some("missing"), "PEAR_MISSING_FATAL=1: aborting");
        unsafe { abort() };
    }
}
