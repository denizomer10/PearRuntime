//! One coherent iOS device identity across sysctl, uname, IOKit, UIDevice,
//! NSProcessInfo, CoreMotion and network interfaces (values from the device
//! profile: PEAR_DEVICE_MODEL / NAME / IDIOM, PEAR_FAKE_IOS_VERSION / BUILD,
//! PEAR_FAKE_DARWIN). An environment-consistency layer: it never inspects or
//! patches an app's own logic or integrity checks. PEAR_NO_DEVICE_SHIM=1 opts out.
//!
//! The C-level interposers run in early sysctl/uname paths: they read the
//! pure-C profile and never touch Foundation or allocate.

use crate::device::profile::{self, Source, Status};
use crate::rt::{self, Interpose};
use pear_objc::{cls, msg0, msg1, nsstring, sel, Class, Id, Imp, Sel, NIL};
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Once, OnceLock};

const ENOMEM: c_int = 12;
const EFAULT: c_int = 14;
const CTL_KERN: c_int = 1;
const CTL_HW: c_int = 6;
const AF_LINK: u8 = 18;
const KERN_SUCCESS: c_int = 0;
const KERN_FAILURE: c_int = 5;
const UTF8: u32 = 0x0800_0100;

type CFTypeRef = *const c_void;

extern "C" {
    fn __error() -> *mut c_int;
    fn getenv(name: *const c_char) -> *const c_char;
    fn strlen(s: *const c_char) -> usize;
    fn sysctlbyname(name: *const c_char, oldp: *mut c_void, oldlenp: *mut usize, newp: *mut c_void, newlen: usize) -> c_int;
    fn __sysctl(name: *mut c_int, namelen: c_uint, oldp: *mut c_void, oldlenp: *mut usize, newp: *mut c_void, newlen: usize) -> c_int;
    fn uname(value: *mut Utsname) -> c_int;
    fn getifaddrs(ifap: *mut *mut IfAddrs) -> c_int;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    // IOKit is in the process through Metal; resolved at load (dynamic_lookup).
    fn IORegistryEntryCreateCFProperty(entry: u32, key: CFTypeRef, allocator: CFTypeRef, options: u32) -> CFTypeRef;
    fn IORegistryEntrySearchCFProperty(entry: u32, plane: *const c_char, key: CFTypeRef, allocator: CFTypeRef, options: u32) -> CFTypeRef;
    fn IORegistryEntryCreateCFProperties(entry: u32, properties: *mut CFTypeRef, allocator: CFTypeRef, options: u32) -> c_int;
    fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
    fn IOServiceGetMatchingServices(main_port: u32, matching: CFTypeRef, existing: *mut u32) -> c_int;

    fn CFDataCreate(allocator: CFTypeRef, bytes: *const u8, length: isize) -> CFTypeRef;
    fn CFStringCreateWithCString(allocator: CFTypeRef, s: *const c_char, encoding: u32) -> CFTypeRef;
    fn CFStringGetCString(s: CFTypeRef, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
    fn CFStringGetTypeID() -> usize;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFRelease(cf: CFTypeRef);
    fn CFDictionarySetValue(dict: CFTypeRef, key: CFTypeRef, value: CFTypeRef);
    fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFDictionaryCreateMutable(allocator: CFTypeRef, capacity: isize, keys: *const c_void, values: *const c_void) -> CFTypeRef;
    fn CFArrayCreateMutable(allocator: CFTypeRef, capacity: isize, callbacks: *const c_void) -> CFTypeRef;
    fn CFArrayAppendValue(array: CFTypeRef, value: CFTypeRef);
    fn CFNumberCreate(allocator: CFTypeRef, kind: isize, value: *const c_void) -> CFTypeRef;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    static kCFTypeArrayCallBacks: c_void;
    static kCFBooleanTrue: CFTypeRef;
    static kCFBooleanFalse: CFTypeRef;
}

fn shim_disabled() -> bool {
    unsafe { !getenv(c"PEAR_NO_DEVICE_SHIM".as_ptr()).is_null() }
}

fn env_set(name: &CStr) -> bool {
    unsafe { !getenv(name.as_ptr()).is_null() }
}

fn p() -> &'static profile::Profile {
    profile::get()
}

// ---- sysctl / uname values ---------------------------------------------

enum Value {
    Text(*const c_char),
    U32(u32),
    U64(u64),
}

/// The iOS-looking value for a hardware or kernel fact.
unsafe fn value_for_name(name: &[u8]) -> Option<Value> {
    let profile = p();
    let kernel = || env_set(c"PEAR_DEVICE_KERNEL");
    let text = |t: *const c_char| (!t.is_null()).then_some(Value::Text(t));
    Some(match name {
        b"hw.machine" | b"hw.model" | b"hw.product" | b"hw.targettype" => return text(profile.model),
        b"hw.machinearch" => Value::Text(c"arm64".as_ptr()),
        b"hw.memsize" => Value::U64(6 << 30), // A16 device class
        b"hw.ncpu" | b"hw.physicalcpu" | b"hw.logicalcpu" | b"hw.physicalcpu_max" | b"hw.logicalcpu_max" => Value::U32(6),
        b"hw.pagesize" => Value::U32(16384),
        b"hw.cpufrequency" | b"hw.cpufrequency_max" | b"hw.cpufrequency_min" => Value::U64(3_460_000_000),
        b"hw.l1icachesize" | b"hw.l1dcachesize" => Value::U32(131072),
        b"hw.l2cachesize" => Value::U32(16777216),
        b"hw.l3cachesize" => Value::U32(0),
        b"hw.cachelinesize" => Value::U32(128),
        b"hw.byteorder" | b"hw.vectorunit" => Value::U32(1),
        b"machdep.cpu.brand_string" => Value::Text(c"Apple A16 Bionic".as_ptr()),
        b"machdep.cpu.vendor" => Value::Text(c"Apple".as_ptr()),
        b"kern.osproductversion" => return if kernel() { text(profile.system_version) } else { None },
        b"kern.osversion" => return if kernel() { text(profile.system_build) } else { None },
        b"kern.osrelease" => return if kernel() { text(profile.darwin_release) } else { None },
        b"kern.ostype" => Value::Text(c"Darwin".as_ptr()),
        b"kern.hostname" => Value::Text(c"iPhone".as_ptr()),
        b"kern.maxfilesperproc" => Value::U32(10240),
        b"kern.maxproc" => Value::U32(1024),
        _ => return None,
    })
}

/// sysctl conventions: 0, or -1 with ENOMEM when the buffer is too small;
/// a size query reports the length required.
unsafe fn write_value(value: &Value, oldp: *mut c_void, oldlenp: *mut usize) -> c_int {
    let need = match value {
        Value::Text(t) => strlen(*t) + 1,
        Value::U32(_) => 4,
        Value::U64(_) => 8,
    };
    if oldp.is_null() {
        if !oldlenp.is_null() {
            *oldlenp = need;
        }
        return 0;
    }
    if oldlenp.is_null() {
        *__error() = EFAULT;
        return -1;
    }
    if *oldlenp < need {
        *oldlenp = need;
        *__error() = ENOMEM;
        return -1;
    }
    match value {
        Value::Text(t) => std::ptr::copy_nonoverlapping(*t as *const u8, oldp as *mut u8, need),
        Value::U32(n) => std::ptr::copy_nonoverlapping(n as *const u32 as *const u8, oldp as *mut u8, 4),
        Value::U64(n) => std::ptr::copy_nonoverlapping(n as *const u64 as *const u8, oldp as *mut u8, 8),
    }
    *oldlenp = need;
    0
}

unsafe extern "C" fn pear_sysctlbyname(name: *const c_char, oldp: *mut c_void, oldlenp: *mut usize, newp: *mut c_void, newlen: usize) -> c_int {
    if !shim_disabled() && !name.is_null() {
        if let Some(v) = value_for_name(CStr::from_ptr(name).to_bytes()) {
            return write_value(&v, oldp, oldlenp);
        }
    }
    sysctlbyname(name, oldp, oldlenp, newp, newlen)
}

fn name_for_mib(name: &[c_int]) -> Option<&'static [u8]> {
    if name.len() < 2 {
        return None;
    }
    match (name[0], name[1]) {
        (CTL_HW, 1) => Some(b"hw.machine"),
        (CTL_HW, 2) => Some(b"hw.model"),
        (CTL_HW, 27) => Some(b"hw.product"),
        (CTL_HW, 3) => Some(b"hw.ncpu"),
        (CTL_HW, 4) => Some(b"hw.byteorder"),
        (CTL_HW, 7) => Some(b"hw.pagesize"),
        (CTL_HW, 12) => Some(b"hw.machinearch"),
        (CTL_HW, 15) => Some(b"hw.cpufrequency"),
        (CTL_HW, 16) => Some(b"hw.cachelinesize"),
        (CTL_HW, 17) => Some(b"hw.l1icachesize"),
        (CTL_HW, 18) => Some(b"hw.l1dcachesize"),
        (CTL_HW, 20) => Some(b"hw.l2cachesize"),
        (CTL_HW, 22) => Some(b"hw.l3cachesize"),
        (CTL_HW, 24) => Some(b"hw.memsize"),
        (CTL_KERN, 1) => Some(b"kern.ostype"),
        (CTL_KERN, 2) => Some(b"kern.osrelease"),
        (CTL_KERN, 65) => Some(b"kern.osversion"),
        (CTL_KERN, 10) => Some(b"kern.hostname"),
        (CTL_KERN, 6) => Some(b"kern.maxproc"),
        (CTL_KERN, 29) => Some(b"kern.maxfilesperproc"),
        _ => None,
    }
}

/// MIB sysctl, also the raw `syscall(SYS_sysctl, …)` entry used by the sandbox.
#[no_mangle]
pub unsafe extern "C" fn PearSysctlMIB(name: *mut c_int, namelen: c_uint, oldp: *mut c_void, oldlenp: *mut usize, newp: *mut c_void, newlen: usize) -> c_int {
    if !shim_disabled() && !name.is_null() {
        let mib = std::slice::from_raw_parts(name, namelen as usize);
        if let Some(v) = name_for_mib(mib).and_then(|n| value_for_name(n)) {
            return write_value(&v, oldp, oldlenp);
        }
    }
    __sysctl(name, namelen, oldp, oldlenp, newp, newlen)
}

#[repr(C)]
struct Utsname {
    sysname: [c_char; 256],
    nodename: [c_char; 256],
    release: [c_char; 256],
    version: [c_char; 256],
    machine: [c_char; 256],
}

unsafe fn copy_into(dst: &mut [c_char; 256], src: *const c_char) {
    let s = CStr::from_ptr(src).to_bytes();
    let n = s.len().min(255);
    for (i, b) in s[..n].iter().enumerate() {
        dst[i] = *b as c_char;
    }
    dst[n] = 0;
}

/// Opt-in (PEAR_DEVICE_UNAME): system frameworks (AppIntents, Swift
/// concurrency) select platform paths from uname and assert otherwise.
unsafe extern "C" fn pear_uname(value: *mut Utsname) -> c_int {
    let result = uname(value);
    let profile = p();
    if result == 0
        && !value.is_null()
        && !shim_disabled()
        && env_set(c"PEAR_DEVICE_UNAME")
        && !profile.model.is_null()
        && !profile.darwin_release.is_null()
    {
        copy_into(&mut (*value).machine, profile.model);
        copy_into(&mut (*value).release, profile.darwin_release);
    }
    result
}

// ---- IORegistry identity -------------------------------------------------

static mut SERIAL: [u8; 13] = [0; 13];
static mut UUID: [u8; 37] = [0; 37];
static IDENTIFIERS: Once = Once::new();

fn stable_hash(text: &[u8], seed: u64) -> u64 {
    text.iter().fold(seed, |h, &b| (h ^ b as u64).wrapping_mul(1099511628211))
}

unsafe fn build_synthetic_identifiers() {
    IDENTIFIERS.call_once(|| {
        let bundle_id: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
        let seed = pear_objc::nsstring_to_string(bundle_id).unwrap_or_else(|| "com.pearsilicon.app".into());
        let a = stable_hash(seed.as_bytes(), 1469598103934665603);
        let model = if p().model.is_null() { &[][..] } else { CStr::from_ptr(p().model).to_bytes() };
        let b = stable_hash(model, a ^ 1099511628211);
        const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";
        let serial = &mut *std::ptr::addr_of_mut!(SERIAL);
        for i in 0..12 {
            serial[i] = ALPHABET[((a >> ((i * 5) % 60)) & 31) as usize];
        }
        let text = format!(
            "{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
            (a >> 32) as u32,
            (a & 0xFFFF) as u32,
            ((b >> 48) & 0x0FFF) as u32 | 0x4000,
            ((b >> 32) & 0x3FFF) as u32 | 0x8000,
            b & 0xFFFF_FFFF_FFFF
        );
        (&mut *std::ptr::addr_of_mut!(UUID))[..36].copy_from_slice(&text.as_bytes()[..36]);
    });
}

unsafe fn cf_data(bytes: &[u8]) -> CFTypeRef {
    CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize)
}

unsafe fn cf_string(text: *const c_char) -> CFTypeRef {
    CFStringCreateWithCString(std::ptr::null(), text, UTF8)
}

/// What the platform expert would return on an iOS device (Create rule), or
/// null for keys that are not identity-related.
unsafe fn spoofed_registry_value(key: CFTypeRef) -> CFTypeRef {
    let model_ptr = p().model;
    if key.is_null() || model_ptr.is_null() {
        return std::ptr::null();
    }
    let mut name = [0 as c_char; 128];
    if !CFStringGetCString(key, name.as_mut_ptr(), 128, UTF8) {
        return std::ptr::null();
    }
    build_synthetic_identifiers();
    let model = CStr::from_ptr(model_ptr).to_bytes();
    let serial = &*std::ptr::addr_of!(SERIAL);
    match CStr::from_ptr(name.as_ptr()).to_bytes() {
        b"model" | b"target-type" => cf_data(model),
        b"product-name" => {
            // Marketing name for the model; unknown models get a generic one.
            let product: &CStr = match model {
                b"iPhone19,2" => c"iPhone 18 Pro",
                b"iPhone19,3" | b"iPhone19,7" => c"iPhone 18 Pro Max",
                b"iPhone15,2" => c"iPhone 14 Pro",
                m if m.starts_with(b"iPad") => c"iPad",
                _ => c"iPhone",
            };
            cf_string(product.as_ptr())
        }
        b"board-id" => cf_data(match model {
            b"iPhone19,2" => b"V63AP",
            b"iPhone19,3" => b"V64AP",
            b"iPhone19,7" => b"V64sAP",
            _ => b"D63AP",
        }),
        b"IOPlatformSerialNumber" => cf_string(serial.as_ptr() as *const c_char),
        b"serial-number" => cf_data(&serial[..12]),
        b"IOPlatformUUID" => cf_string(std::ptr::addr_of!(UUID) as *const c_char),
        b"device-id" => cf_data(&[0x0A, 0x00, 0x00, 0x01]),
        b"compatible" => {
            // At most 31 bytes, NUL included in the data.
            let n = model.len().min(31);
            let mut buf = [0u8; 32];
            buf[..n].copy_from_slice(&model[..n]);
            cf_data(&buf[..n + 1])
        }
        _ => std::ptr::null(),
    }
}

unsafe fn is_platform_expert(entry: u32) -> bool {
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
    let f = dlsym(RTLD_DEFAULT, c"IOObjectConformsTo".as_ptr());
    if f.is_null() {
        return false;
    }
    let f: unsafe extern "C" fn(u32, *const c_char) -> u32 = std::mem::transmute(f);
    f(entry, c"IOPlatformExpertDevice".as_ptr()) != 0
}

unsafe extern "C" fn pear_ioreg_create(entry: u32, key: CFTypeRef, allocator: CFTypeRef, options: u32) -> CFTypeRef {
    if !shim_disabled() && is_platform_expert(entry) {
        let v = spoofed_registry_value(key);
        if !v.is_null() {
            return v;
        }
    }
    IORegistryEntryCreateCFProperty(entry, key, allocator, options)
}

unsafe extern "C" fn pear_ioreg_search(entry: u32, plane: *const c_char, key: CFTypeRef, allocator: CFTypeRef, options: u32) -> CFTypeRef {
    if !shim_disabled() {
        let v = spoofed_registry_value(key);
        if !v.is_null() {
            return v;
        }
    }
    IORegistryEntrySearchCFProperty(entry, plane, key, allocator, options)
}

unsafe extern "C" fn pear_ioreg_properties(entry: u32, properties: *mut CFTypeRef, allocator: CFTypeRef, options: u32) -> c_int {
    let result = IORegistryEntryCreateCFProperties(entry, properties, allocator, options);
    if result != KERN_SUCCESS || properties.is_null() || (*properties).is_null() || shim_disabled() || !is_platform_expert(entry) {
        return result;
    }
    for k in [c"model", c"product-name", c"board-id", c"IOPlatformSerialNumber", c"serial-number", c"IOPlatformUUID", c"device-id", c"compatible"] {
        let key = cf_string(k.as_ptr());
        if key.is_null() {
            continue;
        }
        let value = spoofed_registry_value(key);
        if !value.is_null() {
            CFDictionarySetValue(*properties, key, value);
            CFRelease(value);
        }
        CFRelease(key);
    }
    result
}

// ---- power sources: an internal battery, discharging -------------------

unsafe fn dict_set(dict: CFTypeRef, key: &CStr, value: CFTypeRef, owned: bool) {
    let name = cf_string(key.as_ptr());
    if !name.is_null() && !value.is_null() {
        CFDictionarySetValue(dict, name, value);
    }
    if !name.is_null() {
        CFRelease(name);
    }
    if owned && !value.is_null() {
        CFRelease(value);
    }
}

unsafe fn dict_string(dict: CFTypeRef, key: &CStr, value: &CStr) {
    dict_set(dict, key, cf_string(value.as_ptr()), true);
}

unsafe fn dict_int(dict: CFTypeRef, key: &CStr, value: i64) {
    dict_set(dict, key, CFNumberCreate(std::ptr::null(), 4, &value as *const i64 as *const c_void), true);
}

unsafe fn dict_bool(dict: CFTypeRef, key: &CStr, value: bool) {
    dict_set(dict, key, if value { kCFBooleanTrue } else { kCFBooleanFalse }, false);
}

unsafe extern "C" fn pear_power_sources() -> CFTypeRef {
    if shim_disabled() {
        return IOPSCopyPowerSourcesInfo();
    }
    let battery = CFDictionaryCreateMutable(std::ptr::null(), 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    if battery.is_null() {
        return std::ptr::null();
    }
    dict_string(battery, c"Type", c"InternalBattery");
    dict_string(battery, c"Power Source State", c"Battery Power");
    dict_int(battery, c"Current Capacity", 87);
    dict_int(battery, c"Max Capacity", 100);
    dict_bool(battery, c"Is Charging", false);
    dict_bool(battery, c"Is Present", true);
    dict_bool(battery, c"Is Charged", false);
    dict_int(battery, c"Time to Empty", 300);
    dict_int(battery, c"Time to Full Charge", -1);
    dict_string(battery, c"Battery Health", c"Good");
    dict_string(battery, c"Battery Health Condition", c"Good");
    dict_int(battery, c"Current", 1000);
    dict_int(battery, c"Voltage", 3900);
    dict_int(battery, c"Temperature", 300);
    dict_string(battery, c"Transport Type", c"Internal");
    dict_bool(battery, c"External Connected", false);
    dict_string(battery, c"Name", c"iPhone Battery");
    dict_string(battery, c"Model", c"A16");
    dict_string(battery, c"Serial", c"0");
    dict_string(battery, c"Manufacturer", c"Apple");
    dict_bool(battery, c"Battery Provides Time Remaining", true);
    let array = CFArrayCreateMutable(std::ptr::null(), 1, &kCFTypeArrayCallBacks);
    if array.is_null() {
        CFRelease(battery);
        return std::ptr::null();
    }
    CFArrayAppendValue(array, battery);
    CFRelease(battery);
    array
}

unsafe extern "C" fn pear_io_service_matching(main_port: u32, matching: CFTypeRef, existing: *mut u32) -> c_int {
    if env_set(c"PEAR_DEBUG_IOKIT") && !matching.is_null() {
        let key = cf_string(c"IOProviderClass".as_ptr());
        let value = CFDictionaryGetValue(matching, key);
        CFRelease(key);
        let mut name = [0 as c_char; 128];
        let label = if !value.is_null() && CFGetTypeID(value) == CFStringGetTypeID() && CFStringGetCString(value, name.as_mut_ptr(), 128, UTF8) {
            rt::cstr(name.as_ptr()).to_string()
        } else {
            "(unknown)".into()
        };
        rt::log(&format!("[PearRuntime][iokit] matching services for {label}"));
    }
    let _ = KERN_FAILURE;
    IOServiceGetMatchingServices(main_port, matching, existing)
}

// ---- network: a non-entitled iOS app never sees the Wi-Fi hardware address

#[repr(C)]
struct IfAddrs {
    next: *mut IfAddrs,
    name: *const c_char,
    flags: c_uint,
    addr: *mut SockaddrDl,
    netmask: *mut c_void,
    dstaddr: *mut c_void,
    data: *mut c_void,
}

#[repr(C)]
struct SockaddrDl {
    len: u8,
    family: u8,
    index: u16,
    kind: u8,
    nlen: u8,
    alen: u8,
    slen: u8,
    data: [u8; 12],
}

unsafe extern "C" fn pear_getifaddrs(ifap: *mut *mut IfAddrs) -> c_int {
    let result = getifaddrs(ifap);
    if result != 0 || ifap.is_null() || shim_disabled() {
        return result;
    }
    let mut item = *ifap;
    while !item.is_null() {
        let addr = (*item).addr;
        if !addr.is_null() && (*addr).family == AF_LINK && (*addr).alen == 6 {
            let mac = (addr as *mut u8).add(8 + (*addr).nlen as usize);
            std::ptr::write_bytes(mac, 0, 6);
            *mac = 0x02;
        }
        item = (*item).next;
    }
    result
}

#[used]
#[link_section = "__DATA,__interpose"]
static INTERPOSE: [Interpose; 9] = [
    Interpose { replacement: pear_sysctlbyname as *const c_void, replacee: sysctlbyname as *const c_void },
    Interpose { replacement: PearSysctlMIB as *const c_void, replacee: __sysctl as *const c_void },
    Interpose { replacement: pear_uname as *const c_void, replacee: uname as *const c_void },
    Interpose { replacement: pear_ioreg_create as *const c_void, replacee: IORegistryEntryCreateCFProperty as *const c_void },
    Interpose { replacement: pear_ioreg_search as *const c_void, replacee: IORegistryEntrySearchCFProperty as *const c_void },
    Interpose { replacement: pear_ioreg_properties as *const c_void, replacee: IORegistryEntryCreateCFProperties as *const c_void },
    Interpose { replacement: pear_power_sources as *const c_void, replacee: IOPSCopyPowerSourcesInfo as *const c_void },
    Interpose { replacement: pear_io_service_matching as *const c_void, replacee: IOServiceGetMatchingServices as *const c_void },
    Interpose { replacement: pear_getifaddrs as *const c_void, replacee: getifaddrs as *const c_void },
];

// ---- UIDevice / NSProcessInfo / CoreMotion / assets --------------------

unsafe fn text(t: *const c_char) -> Id {
    if t.is_null() { NIL } else { nsstring(rt::cstr(t)) }
}

unsafe extern "C" fn device_name(_s: Id, _c: Sel) -> Id { text(p().name) }
unsafe extern "C" fn device_system_name(_s: Id, _c: Sel) -> Id { nsstring("iOS") }
unsafe extern "C" fn device_system_version(_s: Id, _c: Sel) -> Id { text(p().system_version) }
unsafe extern "C" fn battery_level(_s: Id, _c: Sel) -> f32 { 0.87 }
unsafe extern "C" fn battery_state(_s: Id, _c: Sel) -> isize { 1 } // unplugged
unsafe extern "C" fn yes(_s: Id, _c: Sel) -> bool { true }
unsafe extern "C" fn no_op(_s: Id, _c: Sel) {}

/// FNV-1a 128-bit vendor UUID from bundle id and model (an identifier, not a
/// secret); a managed profile supplies the host-computed one instead.
unsafe fn synthetic_vendor_id() -> Id {
    static ID: OnceLock<usize> = OnceLock::new();
    *ID.get_or_init(|| {
        let profile = p();
        if profile.status != Status::Ready {
            return 0;
        }
        if profile.source == Source::HostV1 {
            let u: Id = msg1(msg0::<Id>(cls("NSUUID"), "alloc"), "initWithUUIDString:", text(profile.vendor_identifier));
            return u as usize;
        }
        let bundle_id: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
        let seed = format!(
            "{}:{}",
            pear_objc::nsstring_to_string(bundle_id).unwrap_or_else(|| "unknown".into()),
            rt::cstr(profile.model)
        );
        let (mut h1, mut h2) = (1469598103934665603u64, 1099511628211u64);
        for &b in seed.as_bytes() {
            h1 = (h1 ^ b as u64).wrapping_mul(1099511628211);
            h2 = (h2 ^ b as u64).wrapping_mul(1469598103934665603);
        }
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&h1.to_le_bytes());
        bytes[8..].copy_from_slice(&h2.to_le_bytes());
        let u: Id = msg1(msg0::<Id>(cls("NSUUID"), "alloc"), "initWithUUIDBytes:", bytes.as_ptr());
        u as usize
    }) as Id
}

unsafe extern "C" fn vendor_id(_s: Id, _c: Sel) -> Id { synthetic_vendor_id() }
fn wants_pad() -> bool {
    !p().idiom.is_null() && unsafe { CStr::from_ptr(p().idiom) }.to_bytes() == b"pad"
}
unsafe extern "C" fn device_idiom(_s: Id, _c: Sel) -> isize { wants_pad() as isize }

static ORIGINAL_TRAIT_IDIOM: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
unsafe extern "C" fn trait_idiom(s: Id, c: Sel) -> isize {
    if wants_pad() {
        return 1;
    }
    let f: unsafe extern "C" fn(Id, Sel) -> isize = std::mem::transmute(ORIGINAL_TRAIT_IDIOM.load(Ordering::Acquire));
    let v = f(s, c);
    if v == 1 || v == 5 { 0 } else { v } // pad/mac -> phone
}

unsafe fn swizzle(c: Class, selector: &str, imp: Imp) -> Imp {
    if c.is_null() {
        return std::ptr::null();
    }
    let m = rt::class_getInstanceMethod(c, sel(selector));
    if m.is_null() { std::ptr::null() } else { rt::method_setImplementation(m, imp) }
}

unsafe fn install_device_properties() {
    if env_set(c"PEAR_NO_DEVICE_UI") {
        return;
    }
    let device = cls("UIDevice");
    if device.is_null() {
        return;
    }
    let ready = p().status == Status::Ready;
    // Layout idiom and vendor token are opt-in: system services use them to
    // pick UIKit code paths and can assert before the first scene.
    if ready {
        swizzle(device, "model", device_name as Imp);
        swizzle(device, "localizedModel", device_name as Imp);
        swizzle(device, "systemName", device_system_name as Imp);
        swizzle(device, "systemVersion", device_system_version as Imp);
        swizzle(device, "name", device_name as Imp);
    }
    swizzle(device, "batteryLevel", battery_level as Imp);
    swizzle(device, "batteryState", battery_state as Imp);
    swizzle(device, "isBatteryMonitoringEnabled", yes as Imp);
    if env_set(c"PEAR_DEVICE_UI_DETAILS") {
        if ready {
            swizzle(device, "identifierForVendor", vendor_id as Imp);
            swizzle(device, "userInterfaceIdiom", device_idiom as Imp);
        }
        swizzle(device, "isMultitaskingSupported", yes as Imp);
    }
    // Diagnostic only: forcing the trait idiom changes the whole layout mode.
    if ready && env_set(c"PEAR_FORCE_TRAIT_IDIOM") {
        let old = swizzle(cls("UITraitCollection"), "userInterfaceIdiom", trait_idiom as Imp);
        ORIGINAL_TRAIT_IDIOM.store(old as *mut c_void, Ordering::Release);
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct OsVersion {
    major: isize,
    minor: isize,
    patch: isize,
}
static OS_VERSION: OnceLock<OsVersion> = OnceLock::new();

unsafe extern "C" fn os_version(_s: Id, _c: Sel) -> OsVersion {
    *OS_VERSION.get().unwrap()
}
unsafe extern "C" fn os_version_string(_s: Id, _c: Sel) -> Id {
    nsstring(&format!("Version {} (Build {})", rt::cstr(p().system_version), rt::cstr(p().system_build)))
}
unsafe extern "C" fn os_at_least(_s: Id, _c: Sel, requested: OsVersion) -> bool {
    let v = OS_VERSION.get().unwrap();
    if v.major != requested.major {
        return v.major > requested.major;
    }
    if v.minor != requested.minor {
        return v.minor > requested.minor;
    }
    v.patch >= requested.patch
}

/// Opt-in (PEAR_SPOOF_OS_VERSION): AppIntents and Swift concurrency assert if
/// the process claims an iOS Foundation version.
unsafe fn install_operating_system_version() {
    if !env_set(c"PEAR_SPOOF_OS_VERSION") || p().system_version.is_null() {
        return;
    }
    let info = cls("NSProcessInfo");
    let parts: Vec<isize> = rt::cstr(p().system_version).split('.').map(|x| rt::ns_string_double(x) as isize).collect();
    let _ = OS_VERSION.set(OsVersion {
        major: parts.first().copied().unwrap_or(16),
        minor: parts.get(1).copied().unwrap_or(0),
        patch: parts.get(2).copied().unwrap_or(0),
    });
    swizzle(info, "operatingSystemVersion", os_version as Imp);
    swizzle(info, "operatingSystemVersionString", os_version_string as Imp);
    swizzle(info, "isOperatingSystemAtLeastVersion:", os_at_least as Imp);
}

unsafe extern "C" fn physical_memory(_s: Id, _c: Sel) -> u64 { 6 << 30 }
unsafe extern "C" fn processor_count(_s: Id, _c: Sel) -> usize { 6 }
unsafe extern "C" fn host_name(_s: Id, _c: Sel) -> Id { nsstring("iPhone") }

/// Process-level hardware facts read by engines and analytics SDKs.
unsafe fn install_process_info_identity() {
    let info = cls("NSProcessInfo");
    swizzle(info, "physicalMemory", physical_memory as Imp);
    swizzle(info, "processorCount", processor_count as Imp);
    swizzle(info, "activeProcessorCount", processor_count as Imp);
    swizzle(info, "hostName", host_name as Imp);
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Vector3 {
    x: f64,
    y: f64,
    z: f64,
}
static MOCK_ACCELEROMETER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static MOCK_GYRO: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "C" fn mock_init(s: Id, _c: Sel) -> Id { s }
unsafe extern "C" fn mock_acceleration(_s: Id, _c: Sel) -> Vector3 { Vector3 { x: 0.0, y: 0.0, z: -1.0 } }
unsafe extern "C" fn mock_rotation(_s: Id, _c: Sel) -> Vector3 { Vector3 { x: 0.0, y: 0.0, z: 0.0 } }
unsafe extern "C" fn mock_timestamp(_s: Id, _c: Sel) -> f64 {
    msg0(msg0::<Id>(cls("NSDate"), "date"), "timeIntervalSinceReferenceDate")
}
unsafe extern "C" fn accelerometer_data(_s: Id, _c: Sel) -> Id { MOCK_ACCELEROMETER.load(Ordering::Acquire) as Id }
unsafe extern "C" fn gyro_data(_s: Id, _c: Sel) -> Id { MOCK_GYRO.load(Ordering::Acquire) as Id }

unsafe fn mock_class(base: &str, name: &str, vector_sel: &str, vector_imp: Imp, vector_types: &str) -> Id {
    let base = cls(base);
    if base.is_null() {
        return NIL;
    }
    let Some(b) = rt::ClassBuilder::new::<()>(name, base) else { return NIL };
    b.method("init", mock_init as Imp, "@@:").method(vector_sel, vector_imp, vector_types).method("timestamp", mock_timestamp as Imp, "d@:");
    let c = b.register();
    msg0(msg0::<Id>(c, "alloc"), "init")
}

/// An accelerometer and gyroscope at rest; device-motion fusion is left to
/// the real implementation (its attitude cannot be synthesized faithfully).
unsafe fn install_motion_identity() {
    rt::dlopen_path("/System/Library/Frameworks/CoreMotion.framework/CoreMotion", rt::RTLD_LAZY);
    let manager = cls("CMMotionManager");
    if manager.is_null() {
        return;
    }
    MOCK_ACCELEROMETER.store(mock_class("CMAccelerometerData", "PearAccelerometerData", "acceleration", mock_acceleration as Imp, "{CMAcceleration=ddd}@:") as *mut c_void, Ordering::Release);
    MOCK_GYRO.store(mock_class("CMGyroData", "PearGyroData", "rotationRate", mock_rotation as Imp, "{CMRotationRate=ddd}@:") as *mut c_void, Ordering::Release);
    swizzle(manager, "isAccelerometerAvailable", yes as Imp);
    swizzle(manager, "isGyroAvailable", yes as Imp);
    swizzle(manager, "accelerometerData", accelerometer_data as Imp);
    swizzle(manager, "gyroData", gyro_data as Imp);
    swizzle(manager, "startAccelerometerUpdates", no_op as Imp);
    swizzle(manager, "startGyroUpdates", no_op as Imp);
}

static ORIGINAL_IMAGE_NAMED: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static IMAGE_NAMED_TRAITS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// iPhone-thinned bundles hold only ~iphone renditions while UIKit resolves
/// against the Mac scene's idiom: retry a failed lookup with phone traits.
unsafe extern "C" fn image_named(c: Id, s: Sel, name: Id) -> Id {
    let plain: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(ORIGINAL_IMAGE_NAMED.load(Ordering::Acquire));
    let image = plain(c, s, name);
    if !image.is_null() || name.is_null() {
        return image;
    }
    let phone: Id = msg1(cls("UITraitCollection"), "traitCollectionWithUserInterfaceIdiom:", 0isize);
    if phone.is_null() {
        return NIL;
    }
    let with_traits: unsafe extern "C" fn(Id, Sel, Id, Id, Id) -> Id = std::mem::transmute(IMAGE_NAMED_TRAITS.load(Ordering::Acquire));
    with_traits(c, sel("imageNamed:inBundle:compatibleWithTraitCollection:"), name, NIL, phone)
}

unsafe fn install_asset_idiom_fallback() {
    if env_set(c"PEAR_NO_ASSET_FALLBACK") {
        return;
    }
    let image = cls("UIImage");
    if image.is_null() || cls("UITraitCollection").is_null() {
        return;
    }
    let plain = rt::class_getClassMethod(image, sel("imageNamed:"));
    let traits = rt::class_getClassMethod(image, sel("imageNamed:inBundle:compatibleWithTraitCollection:"));
    if plain.is_null() || traits.is_null() {
        return;
    }
    ORIGINAL_IMAGE_NAMED.store(rt::method_getImplementation(plain) as *mut c_void, Ordering::Release);
    IMAGE_NAMED_TRAITS.store(rt::method_getImplementation(traits) as *mut c_void, Ordering::Release);
    rt::method_setImplementation(plain, image_named as Imp);
    rt::log("[PearRuntime][device] asset idiom fallback installed");
}

pub fn init() {
    if !rt::module_enabled("device") || shim_disabled() {
        return;
    }
    unsafe {
        pear_objc::autorelease_pool(|| {
            install_device_properties();
            install_asset_idiom_fallback();
            install_motion_identity();
            install_process_info_identity();
            install_operating_system_version();
        });
    }
    rt::after_main(0.5, || unsafe {
        let profile = p();
        if profile.status != Status::Ready {
            rt::log(&format!(
                "[PearRuntime][device] profile unavailable (status={} field={}); profile overrides skipped",
                profile.status as i32,
                if profile.invalid_field.is_null() { "unknown" } else { rt::cstr(profile.invalid_field) }
            ));
        } else {
            let name = rt::cstr(profile.name);
            rt::log(&format!(
                "[PearRuntime][device] identity={}/{name} model={name} iOS={} build={}",
                rt::cstr(profile.model),
                rt::cstr(profile.system_version),
                rt::cstr(profile.system_build)
            ));
        }
    });
}
