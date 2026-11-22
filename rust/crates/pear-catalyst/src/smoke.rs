//! Test seams for the source-level smoke tests (`scripts/*-smoke*`), built
//! only with the `smoke` feature (scripts/build-smoke-runtime.sh), never by
//! build.sh. A smoke build has no image initializer and inert interpose
//! tables; fixtures link the static library, replace the few process-level
//! dependencies a test fakes (window lookup, GL touch mapping, class lookup,
//! libc and Security entry points) through these hooks, and reach module
//! internals through the `PearSmoke*` C entry points below.

use pear_objc::{CGPoint, CGRect, Id};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A replaceable function: 0 keeps the production path.
pub type Hook = AtomicUsize;

/// The installed replacement in `slot`, if any.
///
/// # Safety
/// `F` must be the function-pointer type the slot's setter documents.
pub unsafe fn hook<F: Copy>(slot: &Hook) -> Option<F> {
    debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<usize>());
    let p = slot.load(Ordering::Acquire);
    (p != 0).then(|| std::mem::transmute_copy::<usize, F>(&p))
}

fn set(slot: &Hook, f: usize) {
    slot.store(f, Ordering::Release);
}

// ---- input/legacy_touch, graphics/screen ----------------------------------

/// `PearPrimaryWindow()` (input/touch_synth.rs).
pub static PRIMARY_WINDOW: Hook = Hook::new(0);
/// `PearGLMapTouchPoint()` (graphics_gl/shim.rs).
pub static GL_MAP_TOUCH_POINT: Hook = Hook::new(0);

#[no_mangle]
pub extern "C" fn PearSmokeSetPrimaryWindow(f: Option<unsafe extern "C" fn() -> Id>) {
    set(&PRIMARY_WINDOW, f.map_or(0, |f| f as usize));
}

#[no_mangle]
pub extern "C" fn PearSmokeSetGLMapTouchPoint(f: Option<unsafe extern "C" fn(Id, Id, CGPoint) -> CGPoint>) {
    set(&GL_MAP_TOUCH_POINT, f.map_or(0, |f| f as usize));
}

/// Registers PearLegacyTouch / PearLegacyEvent (input::legacy_touch::init).
#[no_mangle]
pub extern "C" fn PearSmokeLegacyTouchInit() {
    crate::input::legacy_touch::init();
}

/// The UIScreen bounds the screen shim reports (graphics/screen.rs).
#[no_mangle]
pub unsafe extern "C" fn PearSmokeReportedBounds() -> CGRect {
    crate::graphics::screen::reported_bounds()
}

/// A coalesced main-thread window-geometry refresh is pending.
#[no_mangle]
pub extern "C" fn PearSmokeScreenRefreshQueued() -> bool {
    crate::graphics::screen::REFRESH_QUEUED.load(Ordering::Acquire)
}

// ---- storage/sandbox (file-mode repair) -----------------------------------

pub static FS_FSTAT: Hook = Hook::new(0);
pub static FS_FCHMOD: Hook = Hook::new(0);
pub static FS_GETPATH: Hook = Hook::new(0);
pub static FS_OPENAT: Hook = Hook::new(0);
pub static FS_OPEN: Hook = Hook::new(0);

/// Replaces the libc calls the file-mode repair makes (NULL restores libc):
/// `fstat`, `fchmod`, `fcntl(fd, F_GETPATH, buf)`, `openat` and `open`, the
/// last two with libc's variadic mode argument.
#[no_mangle]
pub extern "C" fn PearSmokeSetFilesystem(
    fstat: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void) -> i32>,
    fchmod: Option<unsafe extern "C" fn(i32, u16) -> i32>,
    getpath: Option<unsafe extern "C" fn(i32, i32, *mut std::ffi::c_char) -> i32>,
    openat: Option<unsafe extern "C" fn(i32, *const std::ffi::c_char, i32, ...) -> i32>,
    open: Option<unsafe extern "C" fn(*const std::ffi::c_char, i32, ...) -> i32>,
) {
    set(&FS_FSTAT, fstat.map_or(0, |f| f as usize));
    set(&FS_FCHMOD, fchmod.map_or(0, |f| f as usize));
    set(&FS_GETPATH, getpath.map_or(0, |f| f as usize));
    set(&FS_OPENAT, openat.map_or(0, |f| f as usize));
    set(&FS_OPEN, open.map_or(0, |f| f as usize));
}

/// The storage module's start-up (trace flags, optional iOS sandbox profile).
#[no_mangle]
pub extern "C" fn PearSmokeSandboxInit() {
    crate::storage::sandbox::init();
}

/// PEAR_DEBUG_PATHS tracing is active.
#[no_mangle]
pub extern "C" fn PearTraceEnabled() -> bool {
    crate::storage::sandbox::trace_enabled()
}

// ---- device/identity, services/network -------------------------------------

/// Class lookup of the device and network installers (rt::installer_class).
pub static INSTALLER_CLASS: Hook = Hook::new(0);
/// Opt-out predicate of the device installer only (not its C interposers).
pub static DEVICE_INSTALLER_DISABLED: Hook = Hook::new(0);

#[no_mangle]
pub extern "C" fn PearSmokeSetInstallerClassLookup(f: Option<unsafe extern "C" fn(Id) -> pear_objc::Class>) {
    set(&INSTALLER_CLASS, f.map_or(0, |f| f as usize));
}

#[no_mangle]
pub extern "C" fn PearSmokeSetDeviceInstallerDisabled(f: Option<unsafe extern "C" fn() -> bool>) {
    set(&DEVICE_INSTALLER_DISABLED, f.map_or(0, |f| f as usize));
}

/// A sysctl value the device layer reports: text, or a number of `size` bytes.
#[repr(C)]
pub struct PearDeviceValue {
    pub text: *const std::ffi::c_char,
    pub number: u64,
    pub size: usize,
}

#[no_mangle]
pub unsafe extern "C" fn PearDeviceValueForName(name: *const std::ffi::c_char, out: *mut PearDeviceValue) -> i32 {
    use crate::device::identity::Value;
    if name.is_null() || out.is_null() {
        return 0;
    }
    let Some(value) = crate::device::identity::value_for_name(std::ffi::CStr::from_ptr(name).to_bytes()) else { return 0 };
    *out = match value {
        Value::Text(t) => PearDeviceValue { text: t, number: 0, size: 0 },
        Value::U32(n) => PearDeviceValue { text: std::ptr::null(), number: n as u64, size: 4 },
        Value::U64(n) => PearDeviceValue { text: std::ptr::null(), number: n, size: 8 },
    };
    1
}

#[no_mangle]
pub unsafe extern "C" fn PearWriteDeviceValue(value: *const PearDeviceValue, oldp: *mut std::ffi::c_void, oldlenp: *mut usize) -> i32 {
    use crate::device::identity::Value;
    let v = &*value;
    let value = if !v.text.is_null() {
        Value::Text(v.text)
    } else if v.size == 4 {
        Value::U32(v.number as u32)
    } else {
        Value::U64(v.number)
    };
    crate::device::identity::write_value(&value, oldp, oldlenp)
}

unsafe fn profile_text(p: *const std::ffi::c_char) -> Id {
    if p.is_null() { pear_objc::NIL } else { pear_objc::nsstring(crate::rt::cstr(p)) }
}

/// The profile values the UIDevice/NSProcessInfo/network shims report.
#[no_mangle]
pub unsafe extern "C" fn PearSmokeDeviceModel() -> Id { profile_text(crate::device::profile::get().model) }
#[no_mangle]
pub unsafe extern "C" fn PearSmokeDeviceName() -> Id { profile_text(crate::device::profile::get().name) }
#[no_mangle]
pub unsafe extern "C" fn PearSmokeIOSVersion() -> Id { profile_text(crate::device::profile::get().system_version) }
#[no_mangle]
pub unsafe extern "C" fn PearSmokeIOSBuild() -> Id { profile_text(crate::device::profile::get().system_build) }
#[no_mangle]
pub unsafe extern "C" fn PearSmokeDarwinRelease() -> Id { profile_text(crate::device::profile::get().darwin_release) }
#[no_mangle]
pub extern "C" fn PearSmokeWantsPadIdiom() -> bool { crate::device::identity::wants_pad() }
#[no_mangle]
pub unsafe extern "C" fn PearSmokeSyntheticVendorID() -> Id { crate::device::identity::synthetic_vendor_id() }
#[no_mangle]
pub extern "C" fn PearSmokeDeviceShimDisabled() -> bool { crate::device::identity::shim_disabled() }
/// identity::init (the device module's start-up, behind its opt-outs).
#[no_mangle]
pub extern "C" fn PearSmokeDeviceShimInit() { crate::device::identity::init() }
#[no_mangle]
pub unsafe extern "C" fn PearSmokeInstallDeviceProperties() {
    pear_objc::autorelease_pool(|| crate::device::identity::install_device_properties())
}
/// The device module installed its CoreMotion mocks.
#[no_mangle]
pub extern "C" fn PearSmokeMotionMocksInstalled() -> bool {
    !crate::device::identity::MOCK_ACCELEROMETER.load(Ordering::Acquire).is_null()
        || !crate::device::identity::MOCK_GYRO.load(Ordering::Acquire).is_null()
}

/// network::init (the services module's start-up, behind its opt-outs).
#[no_mangle]
pub extern "C" fn PearSmokeNetworkShimInit() { crate::services::network::init() }
/// The one-time network installer itself.
#[no_mangle]
pub extern "C" fn PearSmokeInstallNetworkShim() { crate::services::network::install() }
#[no_mangle]
pub extern "C" fn PearSmokeNetworkInstalled() -> bool {
    crate::services::network::INSTALLED.load(Ordering::Acquire)
}
#[no_mangle]
pub extern "C" fn PearSmokeNetworkKeepUA() -> bool {
    crate::services::network::KEEP_UA.load(Ordering::Relaxed)
}
#[no_mangle]
pub extern "C" fn PearSmokeSetNetworkKeepUA(keep: bool) {
    crate::services::network::KEEP_UA.store(keep, Ordering::Relaxed)
}
#[no_mangle]
pub unsafe extern "C" fn PearSmokeMergedHeaders(headers: Id) -> Id {
    crate::services::network::merged_headers(headers)
}

// ---- core/host, storage/cloud ----------------------------------------------

/// The host connection lock (a dispatch semaphore; core/host.rs).
#[no_mangle]
pub extern "C" fn PearSmokeHostLock() -> Id { crate::core::host::connection_lock() }
/// The current host connection descriptor, -1 when disconnected.
#[no_mangle]
pub extern "C" fn PearSmokeHostFD() -> i32 { crate::core::host::FD.load(Ordering::Acquire) }
/// CLOCK_MONOTONIC nanoseconds, the clock of PearHostDeadline().
#[no_mangle]
pub extern "C" fn PearSmokeHostNow() -> u64 { crate::core::host::now() }
/// The cloud key-value gate (a dispatch semaphore; storage/cloud.rs).
#[no_mangle]
pub extern "C" fn PearSmokeCloudKVGate() -> Id { crate::storage::cloud::host_gate() }
/// With the gate owned: consumes queued rejected writes, then returns a copy
/// of the confirmed cache (nil before first use) and the unconfirmed count.
#[no_mangle]
pub unsafe extern "C" fn PearSmokeCloudCacheLocked(unconfirmed: *mut usize) -> Id {
    use pear_objc::msg0;
    let mut s = crate::storage::cloud::KV.lock().unwrap_or_else(|e| e.into_inner());
    crate::storage::cloud::collect_rejected(&mut s);
    if !unconfirmed.is_null() {
        *unconfirmed = if s.unconfirmed.is_null() { 0 } else { msg0::<usize>(s.unconfirmed, "count") };
    }
    if s.kv.is_null() { pear_objc::NIL } else { crate::rt::autoreleased(msg0(s.kv, "copy")) }
}
#[no_mangle]
pub unsafe extern "C" fn PearSmokeInstallKeyValueStore() {
    pear_objc::autorelease_pool(|| crate::storage::cloud::install_key_value_store())
}
#[no_mangle]
pub unsafe extern "C" fn PearSmokeInstallUbiquityContainer() {
    pear_objc::autorelease_pool(|| crate::storage::cloud::install_ubiquity_container())
}
#[no_mangle]
pub unsafe extern "C" fn PearSmokeKVObject(key: Id) -> Id { crate::storage::cloud::kv_object(key) }
#[no_mangle]
pub unsafe extern "C" fn PearSmokeKVSet(key: Id, value: Id) -> bool { crate::storage::cloud::kv_set(key, value) }
