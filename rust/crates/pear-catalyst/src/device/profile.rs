//! Startup device identity shared by the early C hooks and the shims
//! (PearDeviceProfile.h). Captured once with only getenv, bounded copies and
//! pure validation: no allocation, ObjC, Foundation, files or IPC, because it
//! is read from sysctl hooks that can run before anything else is ready.
//!
//! No source/metadata: legacy defaults and non-empty overrides, kept verbatim
//! up to LEGACY_MAX bytes (longer rejects the whole snapshot). Any source or
//! metadata selects managed validation: only `host-v1` with a complete,
//! canonical profile, bundle and vendor UUID is accepted.

use std::ffi::{c_char, c_int, CStr};
use std::sync::Once;

const LEGACY_MAX: usize = 4095;
const BUNDLE_MAX: usize = 255;

#[repr(C)]
#[derive(Copy, Clone, PartialEq)]
pub enum Source {
    Legacy = 0,
    HostV1 = 1,
    Unknown = 2,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq)]
pub enum Status {
    Unavailable = 0,
    Ready = 1,
    InvalidSource = 2,
    MissingField = 3,
    InvalidField = 4,
    TooLong = 5,
}

/// C layout of `PearDeviceProfile`. All value pointers are NULL on rejection.
#[repr(C)]
pub struct Profile {
    pub source: Source,
    pub status: Status,
    pub invalid_field: *const c_char,
    pub bundle: *const c_char,
    pub model: *const c_char,
    pub name: *const c_char,
    pub idiom: *const c_char,
    pub system_name: *const c_char,
    pub system_version: *const c_char,
    pub system_build: *const c_char,
    pub darwin_release: *const c_char,
    pub vendor_identifier: *const c_char,
}

static ONCE: Once = Once::new();
static mut PROFILE: Profile = Profile {
    source: Source::Legacy,
    status: Status::Unavailable,
    invalid_field: std::ptr::null(),
    bundle: std::ptr::null(),
    model: std::ptr::null(),
    name: std::ptr::null(),
    idiom: std::ptr::null(),
    system_name: std::ptr::null(),
    system_version: std::ptr::null(),
    system_build: std::ptr::null(),
    darwin_release: std::ptr::null(),
    vendor_identifier: std::ptr::null(),
};
static mut VALUES: [[u8; LEGACY_MAX + 1]; 5] = [[0; LEGACY_MAX + 1]; 5];
static mut BUNDLE: [u8; BUNDLE_MAX + 1] = [0; BUNDLE_MAX + 1];
static mut VENDOR: [u8; 37] = [0; 37];

const KEYS: [&CStr; 6] = [
    c"PEAR_DEVICE_MODEL",
    c"PEAR_DEVICE_NAME",
    c"PEAR_FAKE_IOS_VERSION",
    c"PEAR_FAKE_IOS_BUILD",
    c"PEAR_FAKE_DARWIN",
    c"PEAR_DEVICE_IDIOM",
];
const DEFAULTS: [&CStr; 5] = [c"iPhone19,2", c"iPhone", c"27.2.1", c"24B91034", c"27.2.0"];

extern "C" {
    fn getenv(name: *const c_char) -> *const c_char;
    fn strnlen(s: *const c_char, max: usize) -> usize;
}

unsafe fn env(key: &CStr) -> *const c_char {
    getenv(key.as_ptr())
}

unsafe fn reject(p: *mut Profile, status: Status, key: &CStr) -> bool {
    (*p).status = status;
    (*p).invalid_field = key.as_ptr();
    false
}

/// Copies `text` (NUL included) into `dst` when it fits `maximum` bytes.
unsafe fn copy(p: *mut Profile, dst: *mut u8, text: *const c_char, maximum: usize, key: &CStr) -> bool {
    if text.is_null() {
        return reject(p, Status::MissingField, key);
    }
    let len = strnlen(text, maximum + 1);
    if len > maximum {
        return reject(p, Status::TooLong, key);
    }
    std::ptr::copy_nonoverlapping(text as *const u8, dst, len + 1);
    true
}

/// The NUL-terminated bytes at `p` (without the NUL).
unsafe fn bytes<'a>(p: *const u8) -> &'a [u8] {
    CStr::from_ptr(p as *const c_char).to_bytes()
}

fn is_model(t: &[u8]) -> bool {
    !t.is_empty()
        && t[0].is_ascii_alphabetic()
        && t[1..].iter().all(|&c| c.is_ascii_alphanumeric() || matches!(c, b',' | b'_' | b'-'))
}

fn is_version(t: &[u8]) -> bool {
    let (mut parts, mut digits) = (1u32, 0u32);
    for &c in t {
        if c.is_ascii_digit() {
            digits += 1;
            if digits > 4 {
                return false;
            }
        } else if c == b'.' && digits != 0 {
            parts += 1;
            if parts > 3 {
                return false;
            }
            digits = 0;
        } else {
            return false;
        }
    }
    digits != 0
}

fn is_build(t: &[u8]) -> bool {
    !t.is_empty()
        && t[0].is_ascii_alphanumeric()
        && t[1..].iter().all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// Unicode Cc/Cf (including bidi/format controls), without Foundation/ICU.
fn is_unicode_control(c: u32) -> bool {
    c <= 0x1F
        || (0x7F..=0x9F).contains(&c)
        || c == 0xAD
        || (0x600..=0x605).contains(&c)
        || c == 0x61C
        || c == 0x6DD
        || c == 0x70F
        || (0x890..=0x891).contains(&c)
        || c == 0x8E2
        || c == 0x180E
        || (0x200B..=0x200F).contains(&c)
        || (0x202A..=0x202E).contains(&c)
        || (0x2060..=0x2064).contains(&c)
        || (0x2066..=0x206F).contains(&c)
        || c == 0xFEFF
        || (0xFFF9..=0xFFFB).contains(&c)
        || c == 0x110BD
        || c == 0x110CD
        || (0x13430..=0x1343F).contains(&c)
        || (0x1BCA0..=0x1BCA3).contains(&c)
        || (0x1D173..=0x1D17A).contains(&c)
        || c == 0xE0001
        || (0xE0020..=0xE007F).contains(&c)
}

/// Strict UTF-8 (no overlongs/surrogates) without control characters.
fn is_utf8_text(t: &[u8]) -> bool {
    let mut i = 0;
    while i < t.len() {
        let mut c = t[i] as u32;
        i += 1;
        let (extra, minimum) = match c {
            0xC2..=0xDF => {
                c &= 0x1F;
                (1, 0x80)
            }
            0xE0..=0xEF => {
                c &= 0x0F;
                (2, 0x800)
            }
            0xF0..=0xF4 => {
                c &= 0x07;
                (3, 0x10000)
            }
            0x80.. => return false,
            _ => (0, 0),
        };
        for _ in 0..extra {
            match t.get(i) {
                Some(&b) if (0x80..=0xBF).contains(&b) => {
                    c = (c << 6) | (b as u32 & 0x3F);
                    i += 1;
                }
                _ => return false,
            }
        }
        if c < minimum || c > 0x10FFFF || (0xD800..=0xDFFF).contains(&c) || is_unicode_control(c) {
            return false;
        }
    }
    true
}

fn eq_ascii(t: &[u8], lower: &[u8]) -> bool {
    t.len() == lower.len() && t.iter().zip(lower).all(|(a, b)| a.to_ascii_lowercase() == *b)
}

fn idiom(text: Option<&[u8]>, managed: bool) -> Option<&'static CStr> {
    match text {
        Some(t) if eq_ascii(t, b"pad") || eq_ascii(t, b"ipad") => Some(c"pad"),
        Some(t) if eq_ascii(t, b"phone") || eq_ascii(t, b"iphone") => Some(c"phone"),
        _ if !managed => Some(c"phone"),
        _ => None,
    }
}

/// 8-4-4-4-12 hex, normalized to upper case in place.
unsafe fn normalize_uuid(t: *mut u8) -> bool {
    if bytes(t).len() != 36 {
        return false;
    }
    for i in 0..36 {
        let c = *t.add(i);
        if matches!(i, 8 | 13 | 18 | 23) {
            if c != b'-' {
                return false;
            }
        } else {
            let u = c.to_ascii_uppercase();
            if !(u.is_ascii_digit() || (b'A'..=b'F').contains(&u)) {
                return false;
            }
            *t.add(i) = u;
        }
    }
    true
}

unsafe fn capture() {
    let p = std::ptr::addr_of_mut!(PROFILE);
    let values = std::ptr::addr_of_mut!(VALUES) as *mut [u8; LEGACY_MAX + 1];
    let bundle_buf = std::ptr::addr_of_mut!(BUNDLE) as *mut u8;
    let vendor_buf = std::ptr::addr_of_mut!(VENDOR) as *mut u8;
    let source = env(c"PEAR_DEVICE_PROFILE_SOURCE");
    let bundle = env(c"PEAR_DEVICE_PROFILE_BUNDLE");
    let vendor = env(c"PEAR_DEVICE_VENDOR_UUID");
    let managed = !source.is_null() || !bundle.is_null() || !vendor.is_null();
    (*p).source = if managed { Source::Unknown } else { Source::Legacy };
    if managed {
        // The exact marker fixes the schema: never fall back to legacy for a
        // future, empty or incomplete managed snapshot.
        if source.is_null() || CStr::from_ptr(source).to_bytes() != b"host-v1" {
            reject(p, Status::InvalidSource, c"PEAR_DEVICE_PROFILE_SOURCE");
            return;
        }
        (*p).source = Source::HostV1;
        if !copy(p, bundle_buf, bundle, BUNDLE_MAX, c"PEAR_DEVICE_PROFILE_BUNDLE")
            || !copy(p, vendor_buf, vendor, 36, c"PEAR_DEVICE_VENDOR_UUID")
        {
            return;
        }
        let b = bytes(bundle_buf);
        if b.is_empty() || !is_utf8_text(b) {
            reject(p, Status::InvalidField, c"PEAR_DEVICE_PROFILE_BUNDLE");
            return;
        }
        if !normalize_uuid(vendor_buf) {
            reject(p, Status::InvalidField, c"PEAR_DEVICE_VENDOR_UUID");
            return;
        }
    }
    const MANAGED_LIMITS: [usize; 5] = [63, 127, 14, 64, 14];
    for i in 0..5 {
        let mut text = env(KEYS[i]);
        if !managed && (text.is_null() || *text == 0) {
            text = DEFAULTS[i].as_ptr();
        }
        let dst = (*values.add(i)).as_mut_ptr();
        if !copy(p, dst, text, if managed { MANAGED_LIMITS[i] } else { LEGACY_MAX }, KEYS[i]) {
            return;
        }
        if managed {
            let v = bytes(dst);
            let ok = match i {
                0 => is_model(v),
                1 => !v.is_empty() && is_utf8_text(v),
                3 => is_build(v),
                _ => is_version(v),
            };
            if !ok {
                reject(p, Status::InvalidField, KEYS[i]);
                return;
            }
        }
    }
    let idiom_text = env(KEYS[5]);
    let idiom_bytes = if idiom_text.is_null() { None } else { Some(CStr::from_ptr(idiom_text).to_bytes()) };
    let Some(idiom) = idiom(idiom_bytes, managed) else {
        reject(p, if idiom_text.is_null() { Status::MissingField } else { Status::InvalidField }, KEYS[5]);
        return;
    };
    // Publish only after the whole snapshot passed; Once publishes to readers.
    (*p).bundle = if managed { bundle_buf as *const c_char } else { std::ptr::null() };
    (*p).model = (*values.add(0)).as_ptr() as *const c_char;
    (*p).name = (*values.add(1)).as_ptr() as *const c_char;
    (*p).idiom = idiom.as_ptr();
    (*p).system_name = c"iOS".as_ptr();
    (*p).system_version = (*values.add(2)).as_ptr() as *const c_char;
    (*p).system_build = (*values.add(3)).as_ptr() as *const c_char;
    (*p).darwin_release = (*values.add(4)).as_ptr() as *const c_char;
    (*p).vendor_identifier = if managed { vendor_buf as *const c_char } else { std::ptr::null() };
    (*p).status = Status::Ready;
}

/// Never NULL; storage lives for the process.
#[no_mangle]
pub extern "C" fn PearDeviceProfileGet() -> *const Profile {
    ONCE.call_once(|| unsafe { capture() });
    std::ptr::addr_of!(PROFILE)
}

pub fn get() -> &'static Profile {
    unsafe { &*PearDeviceProfileGet() }
}

#[allow(dead_code)]
fn _assert_c_int(_: c_int) {}
