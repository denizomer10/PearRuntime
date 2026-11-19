//! Local answers for platform services Catalyst apps use during startup but
//! that have no working host equivalent in a sideloaded app: the audio
//! session is accepted (and its preferred I/O buffer applied to the Mac
//! devices), RemoteIO maps to the host's HAL output unit, haptics and review
//! prompts are inert, remote notifications and ARKit report unavailable, and
//! background tasks get local identifiers. No purchase, receipt or
//! notification token is ever simulated. PEAR_NO_SERVICES=1 disables it;
//! PEAR_DEBUG_SERVICES=1 logs each surface and audio-unit call.

use crate::rt::{self, Interpose};
use pear_objc::{cls, msg0, nsstring, objc_release, objc_retain, Class, Id, Imp, Sel, NIL};
use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

type OSStatus = i32;
type AudioComponent = *mut c_void;
type AudioUnit = *mut c_void;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct ComponentDescription {
    kind: u32,
    sub_type: u32,
    manufacturer: u32,
    flags: u32,
    flags_mask: u32,
}

#[repr(C)]
struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

extern "C" {
    fn AudioComponentFindNext(existing: AudioComponent, description: *const ComponentDescription) -> AudioComponent;
    fn AudioComponentGetDescription(component: AudioComponent, description: *mut ComponentDescription) -> OSStatus;
    fn AudioComponentInstanceGetComponent(unit: AudioUnit) -> AudioComponent;
    fn AudioComponentInstanceNew(component: AudioComponent, out: *mut AudioUnit) -> OSStatus;
    fn AudioComponentInstanceDispose(unit: AudioUnit) -> OSStatus;
    fn AudioUnitSetProperty(unit: AudioUnit, id: u32, scope: u32, element: u32, data: *const c_void, size: u32) -> OSStatus;
    fn AudioUnitGetProperty(unit: AudioUnit, id: u32, scope: u32, element: u32, data: *mut c_void, size: *mut u32) -> OSStatus;
    fn AudioUnitInitialize(unit: AudioUnit) -> OSStatus;
    fn AudioUnitUninitialize(unit: AudioUnit) -> OSStatus;
    fn AudioOutputUnitStart(unit: AudioUnit) -> OSStatus;
    fn AudioOutputUnitStop(unit: AudioUnit) -> OSStatus;
    fn AudioObjectGetPropertyData(object: u32, address: *const PropertyAddress, qsize: u32, qdata: *const c_void, size: *mut u32, data: *mut c_void) -> OSStatus;
    fn AudioObjectSetPropertyData(object: u32, address: *const PropertyAddress, qsize: u32, qdata: *const c_void, size: u32, data: *const c_void) -> OSStatus;
    fn os_proc_available_memory() -> usize;
    fn mach_host_self() -> u32;
    fn host_page_size(host: u32, size: *mut usize) -> c_int;
    fn host_statistics64(host: u32, flavor: c_int, info: *mut i32, count: *mut u32) -> c_int;
    fn atexit(f: extern "C" fn()) -> c_int;
}

fn disabled() -> bool {
    rt::env_flag("PEAR_NO_SERVICES")
}

fn debug() -> bool {
    rt::env_flag("PEAR_DEBUG_SERVICES")
}

fn log_once(flag: &AtomicBool, message: &str) {
    if debug() && !flag.swap(true, Ordering::AcqRel) {
        rt::log(message);
    }
}

unsafe fn replace_instance(c: Class, selector: &str, imp: Imp) {
    if c.is_null() {
        return;
    }
    let m = rt::class_getInstanceMethod(c, pear_objc::sel(selector));
    if !m.is_null() {
        rt::method_setImplementation(m, imp);
    }
}

unsafe fn replace_class(c: Class, selector: &str, imp: Imp) {
    if c.is_null() {
        return;
    }
    let m = rt::class_getClassMethod(c, pear_objc::sel(selector));
    if !m.is_null() {
        rt::method_setImplementation(m, imp);
    }
}

unsafe extern "C" fn noop0(_s: Id, _c: Sel) {}
unsafe extern "C" fn noop1(_s: Id, _c: Sel, _a: usize) {}
unsafe extern "C" fn noop_f(_s: Id, _c: Sel, _a: f64) {}
unsafe extern "C" fn ret_nil(_s: Id, _c: Sel) -> Id { NIL }
unsafe extern "C" fn ret_no(_s: Id, _c: Sel) -> bool { false }

// ---- VideoSubscriberAccount: no TV-provider entitlement; registration is
// accepted and reports "no subscription" (Crunchyroll downloads).
unsafe fn install_video_subscriber() {
    let center = cls("VSSubscriptionRegistrationCenter");
    if center.is_null() {
        return;
    }
    replace_instance(center, "setCurrentSubscription:", noop1 as Imp);
    replace_instance(center, "currentSubscription", ret_nil as Imp);
    rt::log("[PearRuntime][services] video subscriber registration is local");
}

// ---- diagnostics: GarageBand's audio-failure alert call site (inspected only).
static ORIGINAL_ALERT: AtomicUsize = AtomicUsize::new(0);
unsafe extern "C" fn alert_controller(s: Id, c: Sel, title: Id, message: Id, style: isize) -> Id {
    let t = rt::ns_string(title);
    let m = rt::ns_string(message);
    if (!message.is_null() && m.contains("audio system")) || (!title.is_null() && t.contains("Unavailable")) {
        let show = |o: Id| if o.is_null() { "(null)".to_string() } else { rt::ns_string(o) };
        rt::log(&format!("[PearRuntime][diagnostic] alert title={} message={}", show(title), show(message)));
        let symbols: Id = msg0(cls("NSThread"), "callStackSymbols");
        for i in 0..pear_objc::nsarray_count(symbols) {
            rt::log(&format!("[PearRuntime][diagnostic]   {}", rt::ns_string(pear_objc::nsarray_object_at(symbols, i))));
        }
    }
    let f: unsafe extern "C" fn(Id, Sel, Id, Id, isize) -> Id = std::mem::transmute(ORIGINAL_ALERT.load(Ordering::Acquire));
    f(s, c, title, message, style)
}
unsafe fn install_alert_logging() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if !debug() || INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    let alert = cls("UIAlertController");
    if alert.is_null() {
        return;
    }
    let m = rt::class_getClassMethod(alert, pear_objc::sel("alertControllerWithTitle:message:preferredStyle:"));
    if !m.is_null() {
        ORIGINAL_ALERT.store(rt::method_getImplementation(m) as usize, Ordering::Release);
        rt::method_setImplementation(m, alert_controller as Imp);
    }
}

// ---- process memory: iOS's per-process budget, from the host's free pages.
unsafe fn available_process_memory() -> usize {
    let host = mach_host_self();
    let mut page: usize = 0;
    if host_page_size(host, &mut page) != 0 || page == 0 {
        page = 4096;
    }
    // vm_statistics64: free_count, active, inactive, wire, ... (natural_t
    // counters); HOST_VM_INFO64 = 4, count = 38 integers.
    let mut stats = [0i32; 64];
    let mut count: u32 = 38;
    if host_statistics64(host, 4, stats.as_mut_ptr(), &mut count) == 0 {
        let free = stats[0] as u32 as u64;
        let inactive = stats[2] as u32 as u64;
        let purgeable = stats[22] as u32 as u64;
        let speculative = stats[23] as u32 as u64;
        return ((free + inactive + speculative + purgeable) * page as u64) as usize;
    }
    msg0::<u64>(msg0::<Id>(cls("NSProcessInfo"), "processInfo"), "physicalMemory") as usize
}

/// The C symbol exists in libSystem as a stub returning 0: interposed.
unsafe extern "C" fn pear_os_proc_available_memory() -> usize {
    available_process_memory()
}

/// A bundled framework imports the C++-mangled name, which no host library
/// provides; dyld would abort at launch without it.
#[export_name = "_Z24os_proc_available_memoryv"]
pub unsafe extern "C" fn mangled_proc_available_memory() -> usize {
    let available = available_process_memory();
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if debug() && !LOGGED.swap(true, Ordering::AcqRel) {
        rt::log(&format!("[PearRuntime][memory] available={} MB", available / (1024 * 1024)));
    }
    available
}

// ---- audio session -----------------------------------------------------

const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const SCOPE_OUTPUT: u32 = u32::from_be_bytes(*b"outp");
const SCOPE_INPUT: u32 = u32::from_be_bytes(*b"inpt");
const DEFAULT_OUTPUT_DEVICE: u32 = u32::from_be_bytes(*b"dOut");
const DEFAULT_INPUT_DEVICE: u32 = u32::from_be_bytes(*b"dIn ");
const BUFFER_FRAME_SIZE: u32 = u32::from_be_bytes(*b"fsiz");
const SYSTEM_OBJECT: u32 = 1;

struct SavedBuffers {
    captured: bool,
    output_device: u32,
    output_frames: u32,
    input_device: u32,
    input_frames: u32,
}
static SAVED: Mutex<SavedBuffers> = Mutex::new(SavedBuffers { captured: false, output_device: 0, output_frames: 0, input_device: 0, input_frames: 0 });

unsafe fn default_device(selector: u32) -> u32 {
    let address = PropertyAddress { selector, scope: SCOPE_GLOBAL, element: 0 };
    let mut device: u32 = 0;
    let mut size = 4u32;
    if AudioObjectGetPropertyData(SYSTEM_OBJECT, &address, 0, std::ptr::null(), &mut size, &mut device as *mut u32 as *mut c_void) != 0 {
        return 0;
    }
    device
}

unsafe fn device_buffer_frames(device: u32, scope: u32) -> u32 {
    if device == 0 {
        return 0;
    }
    let address = PropertyAddress { selector: BUFFER_FRAME_SIZE, scope, element: 0 };
    let mut frames: u32 = 0;
    let mut size = 4u32;
    if AudioObjectGetPropertyData(device, &address, 0, std::ptr::null(), &mut size, &mut frames as *mut u32 as *mut c_void) != 0 {
        return 0;
    }
    frames
}

unsafe fn set_device_buffer_frames(device: u32, scope: u32, frames: u32) {
    if device == 0 {
        return;
    }
    let address = PropertyAddress { selector: BUFFER_FRAME_SIZE, scope, element: 0 };
    let status = AudioObjectSetPropertyData(device, &address, 0, std::ptr::null(), 4, &frames as *const u32 as *const c_void);
    if debug() {
        let actual = device_buffer_frames(device, scope);
        rt::log(&format!("[PearRuntime][audio] device buffer device={device} scope={scope} want={frames} status={status} actual={actual}"));
    }
}

extern "C" fn restore_device_buffers() {
    let s = SAVED.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        if s.output_frames != 0 {
            set_device_buffer_frames(s.output_device, SCOPE_OUTPUT, s.output_frames);
        }
        if s.input_frames != 0 {
            set_device_buffer_frames(s.input_device, SCOPE_INPUT, s.input_frames);
        }
    }
}

/// RemoteIO runs at the session's preferred buffer; a Mac device defaults to
/// a much larger one, so engines sized for 256 frames fail with -10874.
/// The requested size applies for the process's life, then is restored.
unsafe fn apply_preferred_io_buffer(frames: u32) {
    if !(32..=4096).contains(&frames) {
        return;
    }
    if debug() {
        rt::log(&format!("[PearRuntime][audio] preferred IO buffer {frames} frames"));
    }
    let (out_frames, in_frames) = {
        let mut s = SAVED.lock().unwrap_or_else(|e| e.into_inner());
        if !s.captured {
            s.captured = true;
            s.output_device = default_device(DEFAULT_OUTPUT_DEVICE);
            s.output_frames = device_buffer_frames(s.output_device, SCOPE_OUTPUT);
            s.input_device = default_device(DEFAULT_INPUT_DEVICE);
            s.input_frames = device_buffer_frames(s.input_device, SCOPE_INPUT);
            atexit(restore_device_buffers);
        }
        (s.output_frames, s.input_frames)
    };
    if out_frames != 0 && out_frames != frames {
        set_device_buffer_frames(default_device(DEFAULT_OUTPUT_DEVICE), SCOPE_OUTPUT, frames);
    }
    if in_frames != 0 && in_frames != frames {
        set_device_buffer_frames(default_device(DEFAULT_INPUT_DEVICE), SCOPE_INPUT, frames);
    }
}

/// What the app reads back: the host's AVAudioSession never runs the iOS
/// activation path, so isActive stayed NO after setActive:YES (GarageBand
/// treats that as an interruption).
static SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);
static SESSION_CATEGORY: AtomicUsize = AtomicUsize::new(0);
static SESSION_MODE: AtomicUsize = AtomicUsize::new(0);

unsafe fn store_string(slot: &AtomicUsize, value: Id) {
    // The session keeps its own copy of the string (+1 from -copy).
    let new = if value.is_null() { 0 } else { msg0::<Id>(value, "copy") as usize };
    let old = slot.swap(new, Ordering::AcqRel);
    if old != 0 {
        objc_release(old as Id);
    }
}

unsafe fn ok(error: *mut Id) -> bool {
    if !error.is_null() {
        *error = NIL;
    }
    true
}

unsafe extern "C" fn set_category(_s: Id, _c: Sel, category: Id, error: *mut Id) -> bool {
    store_string(&SESSION_CATEGORY, category);
    ok(error)
}
unsafe extern "C" fn set_category_options(_s: Id, _c: Sel, category: Id, _o: usize, error: *mut Id) -> bool {
    store_string(&SESSION_CATEGORY, category);
    ok(error)
}
unsafe extern "C" fn set_category_mode_options(_s: Id, _c: Sel, category: Id, mode: Id, _o: usize, error: *mut Id) -> bool {
    store_string(&SESSION_CATEGORY, category);
    store_string(&SESSION_MODE, mode);
    ok(error)
}
unsafe extern "C" fn set_mode(_s: Id, _c: Sel, mode: Id, error: *mut Id) -> bool {
    store_string(&SESSION_MODE, mode);
    ok(error)
}
unsafe extern "C" fn set_active(_s: Id, _c: Sel, active: bool, error: *mut Id) -> bool {
    SESSION_ACTIVE.store(active, Ordering::Release);
    ok(error)
}
unsafe extern "C" fn set_active_options(_s: Id, _c: Sel, active: bool, _o: usize, error: *mut Id) -> bool {
    SESSION_ACTIVE.store(active, Ordering::Release);
    ok(error)
}
unsafe extern "C" fn accept_double(_s: Id, _c: Sel, _v: f64, error: *mut Id) -> bool {
    ok(error)
}
unsafe extern "C" fn accept_integer(_s: Id, _c: Sel, _v: isize, error: *mut Id) -> bool {
    ok(error)
}
unsafe extern "C" fn set_io_buffer(_s: Id, _c: Sel, duration: f64, error: *mut Id) -> bool {
    apply_preferred_io_buffer((duration * 48000.0 + 0.5) as u32);
    ok(error)
}
unsafe extern "C" fn category(_s: Id, _c: Sel) -> Id {
    let v = SESSION_CATEGORY.load(Ordering::Acquire);
    if v == 0 { nsstring("AVAudioSessionCategoryAmbient") } else { v as Id }
}
unsafe extern "C" fn mode(_s: Id, _c: Sel) -> Id {
    let v = SESSION_MODE.load(Ordering::Acquire);
    if v == 0 { nsstring("AVAudioSessionModeDefault") } else { v as Id }
}
unsafe extern "C" fn is_active(_s: Id, _c: Sel) -> bool {
    SESSION_ACTIVE.load(Ordering::Acquire)
}
unsafe extern "C" fn sample_rate(_s: Id, _c: Sel) -> f64 { 48000.0 }
unsafe extern "C" fn io_buffer_duration(_s: Id, _c: Sel) -> f64 { 0.005 }
unsafe extern "C" fn two_channels(_s: Id, _c: Sel) -> isize { 2 }

unsafe fn install_audio_session() {
    let session = cls("AVAudioSession");
    if session.is_null() {
        return;
    }
    replace_instance(session, "setCategory:error:", set_category as Imp);
    replace_instance(session, "setCategory:withOptions:error:", set_category_options as Imp);
    replace_instance(session, "setCategory:mode:options:error:", set_category_mode_options as Imp);
    replace_instance(session, "setMode:error:", set_mode as Imp);
    replace_instance(session, "setActive:error:", set_active as Imp);
    replace_instance(session, "setActive:withOptions:error:", set_active_options as Imp);
    replace_instance(session, "setPreferredSampleRate:error:", accept_double as Imp);
    replace_instance(session, "setPreferredIOBufferDuration:error:", set_io_buffer as Imp);
    replace_instance(session, "setPreferredInputNumberOfChannels:error:", accept_integer as Imp);
    replace_instance(session, "category", category as Imp);
    replace_instance(session, "mode", mode as Imp);
    replace_instance(session, "isActive", is_active as Imp);
    // macOS mixes every client: "other audio playing" made recording apps
    // conclude their session was interrupted.
    replace_instance(session, "isOtherAudioPlaying", ret_no as Imp);
    replace_instance(session, "secondaryAudioShouldBeSilencedHint", ret_no as Imp);
    replace_instance(session, "sampleRate", sample_rate as Imp);
    replace_instance(session, "IOBufferDuration", io_buffer_duration as Imp);
    replace_instance(session, "inputNumberOfChannels", two_channels as Imp);
    replace_instance(session, "outputNumberOfChannels", two_channels as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][services] audio session configuration accepted");
}

unsafe fn bool_property(o: Id, name: &str) -> String {
    if pear_objc::responds_to(o, name) { (msg0::<bool>(o, name) as i32).to_string() } else { "0".into() }
}

/// Diagnostic: session state and interruption notifications (recording apps
/// quit with "audio system interrupted" when the session reports one).
unsafe fn observe_audio_interruptions() {
    static OBSERVED: AtomicBool = AtomicBool::new(false);
    if !debug() || OBSERVED.swap(true, Ordering::AcqRel) {
        return;
    }
    for name in ["AVAudioSessionInterruptionNotification", "AVAudioSessionRouteChangeNotification", "AVAudioSessionMediaServicesWereResetNotification"] {
        rt::observe_main(name, |note| unsafe {
            let info: Id = msg0(note, "userInfo");
            rt::log(&format!(
                "[PearRuntime][audio] {} {}",
                rt::ns_string(msg0(note, "name")),
                if info.is_null() { "{\n}".into() } else { rt::description(info) }
            ));
        });
    }
    rt::after_main(1.5, || unsafe {
        let class = cls("AVAudioSession");
        let session: Id = if class.is_null() { NIL } else { msg0(class, "sharedInstance") };
        if session.is_null() {
            return;
        }
        let object = |name: &str| -> String {
            if pear_objc::responds_to(session, name) {
                let v: Id = msg0(session, name);
                if v.is_null() { "?".into() } else { rt::ns_string(v) }
            } else {
                "?".into()
            }
        };
        let channels: isize = if pear_objc::responds_to(session, "outputNumberOfChannels") { msg0(session, "outputNumberOfChannels") } else { -1 };
        rt::log(&format!(
            "[PearRuntime][audio] session active={} otherAudio={} category={} mode={} inputAvailable={} outputChannels={channels}",
            bool_property(session, "isActive"),
            bool_property(session, "isOtherAudioPlaying"),
            object("category"),
            object("mode"),
            bool_property(session, "isInputAvailable")
        ));
    });
}

// ---- RemoteIO -> host output unit ---------------------------------------

const TYPE_OUTPUT: u32 = u32::from_be_bytes(*b"auou");
const SUBTYPE_REMOTE_IO: u32 = u32::from_be_bytes(*b"rioc");
const SUBTYPE_VOICE_PROCESSING: u32 = u32::from_be_bytes(*b"vpio");
const SUBTYPE_DEFAULT_OUTPUT: u32 = u32::from_be_bytes(*b"def ");
const SUBTYPE_HAL_OUTPUT: u32 = u32::from_be_bytes(*b"ahal");

static REMOTE_IO_ALIASES: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// HAL output shares RemoteIO's duplex element layout (0 output, 1 input);
/// VoiceProcessingIO installs its own echo-cancellation path that GarageBand's
/// driver frees at teardown. PEAR_REMOTEIO_SUBTYPE=hal|vpio|default|none.
fn remote_io_host_subtype() -> u32 {
    match std::env::var("PEAR_REMOTEIO_SUBTYPE").as_deref() {
        Err(_) | Ok("") => SUBTYPE_HAL_OUTPUT,
        Ok("vpio") => SUBTYPE_VOICE_PROCESSING,
        Ok("default") | Ok("def") => SUBTYPE_DEFAULT_OUTPUT,
        Ok("none") => 0,
        Ok(_) => SUBTYPE_HAL_OUTPUT,
    }
}

fn fourcc(v: u32) -> String {
    v.to_be_bytes().iter().map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '?' }).collect()
}

unsafe extern "C" fn pear_component_find_next(existing: AudioComponent, description: *const ComponentDescription) -> AudioComponent {
    if !description.is_null() && (*description).kind == TYPE_OUTPUT && (*description).sub_type == SUBTYPE_REMOTE_IO {
        let host = remote_io_host_subtype();
        if host == 0 {
            return AudioComponentFindNext(existing, description);
        }
        let mut patched = *description;
        patched.sub_type = host;
        let mut component = AudioComponentFindNext(existing, &patched);
        if component.is_null() && host != SUBTYPE_VOICE_PROCESSING {
            patched.sub_type = SUBTYPE_VOICE_PROCESSING;
            component = AudioComponentFindNext(existing, &patched);
        }
        if component.is_null() && host != SUBTYPE_DEFAULT_OUTPUT {
            patched.sub_type = SUBTYPE_DEFAULT_OUTPUT;
            component = AudioComponentFindNext(existing, &patched);
        }
        if !component.is_null() {
            let mut aliases = REMOTE_IO_ALIASES.lock().unwrap_or_else(|e| e.into_inner());
            if !aliases.contains(&(component as usize)) && aliases.len() < 8 {
                aliases.push(component as usize);
            }
            drop(aliases);
            static LOGGED: AtomicBool = AtomicBool::new(false);
            if debug() && !LOGGED.swap(true, Ordering::AcqRel) {
                let raw: String = host.to_be_bytes().iter().map(|&b| b as char).collect();
                rt::log(&format!("[PearRuntime][audio] RemoteIO mapped to host unit '{raw}'"));
            }
            return component;
        }
    }
    AudioComponentFindNext(existing, description)
}

/// The aliased host unit still describes itself as RemoteIO.
unsafe extern "C" fn pear_component_get_description(component: AudioComponent, description: *mut ComponentDescription) -> OSStatus {
    let status = AudioComponentGetDescription(component, description);
    if status == 0 && !description.is_null() && REMOTE_IO_ALIASES.lock().unwrap_or_else(|e| e.into_inner()).contains(&(component as usize)) {
        (*description).sub_type = SUBTYPE_REMOTE_IO;
    }
    status
}

unsafe fn unit_description(unit: AudioUnit) -> ComponentDescription {
    let mut d = ComponentDescription::default();
    let component = if unit.is_null() { std::ptr::null_mut() } else { AudioComponentInstanceGetComponent(unit) };
    if !component.is_null() {
        AudioComponentGetDescription(component, &mut d);
    }
    d
}

unsafe fn log_unit(call: &str, unit: AudioUnit, status: OSStatus, detail: &str) {
    if !debug() {
        return;
    }
    let d = unit_description(unit);
    rt::log(&format!(
        "[PearRuntime][audio] {call} unit={unit:p} {}/{} status={status} {detail}",
        fourcc(d.kind),
        fourcc(d.sub_type)
    ));
}

/// The hardware buffer follows the host output unit's slice size so the
/// render callback is never asked for more frames than it allocated.
unsafe extern "C" fn pear_unit_set_property(unit: AudioUnit, id: u32, scope: u32, element: u32, data: *const c_void, size: u32) -> OSStatus {
    let status = AudioUnitSetProperty(unit, id, scope, element, data, size);
    if status == 0 && id == 14 && !data.is_null() && size >= 4 && unit_description(unit).kind == TYPE_OUTPUT {
        apply_preferred_io_buffer(*(data as *const u32));
    }
    if debug() {
        let value = if !data.is_null() && size >= 4 { *(data as *const u32) } else { 0 };
        log_unit(
            if status == 0 { "SetProperty" } else { "SetProperty!" },
            unit,
            status,
            &format!("property={}({id}) scope={scope} element={element} size={size} value={value}", fourcc(id)),
        );
    }
    status
}

unsafe extern "C" fn pear_unit_get_property(unit: AudioUnit, id: u32, scope: u32, element: u32, data: *mut c_void, size: *mut u32) -> OSStatus {
    let status = AudioUnitGetProperty(unit, id, scope, element, data, size);
    if debug() {
        let sz = if size.is_null() { 0 } else { *size };
        let value = if !data.is_null() && sz >= 4 && status == 0 { *(data as *const u32) } else { 0 };
        log_unit(
            if status == 0 { "GetProperty" } else { "GetProperty!" },
            unit,
            status,
            &format!("property={}({id}) scope={scope} element={element} size={sz} value={value}", fourcc(id)),
        );
    }
    status
}

unsafe extern "C" fn pear_unit_initialize(unit: AudioUnit) -> OSStatus {
    let status = AudioUnitInitialize(unit);
    log_unit("Initialize", unit, status, if status == 0 { "ok" } else { "failed" });
    status
}
unsafe extern "C" fn pear_unit_uninitialize(unit: AudioUnit) -> OSStatus {
    let status = AudioUnitUninitialize(unit);
    log_unit("Uninitialize", unit, status, "");
    status
}
unsafe extern "C" fn pear_output_start(unit: AudioUnit) -> OSStatus {
    let status = AudioOutputUnitStart(unit);
    log_unit("Start", unit, status, if status == 0 { "ok" } else { "failed" });
    status
}
unsafe extern "C" fn pear_output_stop(unit: AudioUnit) -> OSStatus {
    let status = AudioOutputUnitStop(unit);
    log_unit("Stop", unit, status, "");
    status
}
unsafe extern "C" fn pear_instance_new(component: AudioComponent, out: *mut AudioUnit) -> OSStatus {
    let status = AudioComponentInstanceNew(component, out);
    if debug() {
        let mut d = ComponentDescription::default();
        if !component.is_null() {
            AudioComponentGetDescription(component, &mut d);
        }
        let instance = if out.is_null() { std::ptr::null_mut() } else { *out };
        rt::log(&format!(
            "[PearRuntime][audio] InstanceNew {}/{} status={status} instance={instance:p}",
            fourcc(d.kind),
            fourcc(d.sub_type)
        ));
    }
    status
}
unsafe extern "C" fn pear_instance_dispose(unit: AudioUnit) -> OSStatus {
    log_unit("Dispose", unit, 0, "");
    AudioComponentInstanceDispose(unit)
}

#[used]
#[cfg_attr(not(feature = "smoke"), link_section = "__DATA,__interpose")]
static INTERPOSE: [Interpose; 11] = [
    Interpose { replacement: pear_os_proc_available_memory as *const c_void, replacee: os_proc_available_memory as *const c_void },
    Interpose { replacement: pear_component_find_next as *const c_void, replacee: AudioComponentFindNext as *const c_void },
    Interpose { replacement: pear_component_get_description as *const c_void, replacee: AudioComponentGetDescription as *const c_void },
    Interpose { replacement: pear_unit_set_property as *const c_void, replacee: AudioUnitSetProperty as *const c_void },
    Interpose { replacement: pear_unit_get_property as *const c_void, replacee: AudioUnitGetProperty as *const c_void },
    Interpose { replacement: pear_unit_initialize as *const c_void, replacee: AudioUnitInitialize as *const c_void },
    Interpose { replacement: pear_unit_uninitialize as *const c_void, replacee: AudioUnitUninitialize as *const c_void },
    Interpose { replacement: pear_output_start as *const c_void, replacee: AudioOutputUnitStart as *const c_void },
    Interpose { replacement: pear_output_stop as *const c_void, replacee: AudioOutputUnitStop as *const c_void },
    Interpose { replacement: pear_instance_new as *const c_void, replacee: AudioComponentInstanceNew as *const c_void },
    Interpose { replacement: pear_instance_dispose as *const c_void, replacee: AudioComponentInstanceDispose as *const c_void },
];

// ---- haptics, store, notifications, ARKit, background tasks ---------------

unsafe fn install_haptics() {
    let impact = cls("UIImpactFeedbackGenerator");
    replace_instance(impact, "prepare", noop0 as Imp);
    replace_instance(impact, "impactOccurred", noop0 as Imp);
    replace_instance(impact, "impactOccurredWithIntensity:", noop_f as Imp);
    let notification = cls("UINotificationFeedbackGenerator");
    replace_instance(notification, "prepare", noop0 as Imp);
    replace_instance(notification, "notificationOccurred:", noop1 as Imp);
    let selection = cls("UISelectionFeedbackGenerator");
    replace_instance(selection, "prepare", noop0 as Imp);
    replace_instance(selection, "selectionChanged", noop0 as Imp);
    replace_instance(cls("UIFeedbackGenerator"), "prepare", noop0 as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][services] haptics disabled");
}

/// The review prompt is cosmetic; payments are never simulated.
unsafe fn install_store_surface() {
    let review = cls("SKStoreReviewController");
    replace_class(review, "requestReview", noop0 as Imp);
    replace_class(review, "requestReviewInScene:", noop1 as Imp);
    replace_class(cls("SKPaymentQueue"), "canMakePayments", ret_no as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][services] store surface reports unavailable");
}

unsafe fn report_remote_notification_failure(application: Id) {
    let delegate: Id = msg0(application, "delegate");
    if !pear_objc::responds_to(delegate, "application:didFailToRegisterForRemoteNotificationsWithError:") {
        return;
    }
    let error = rt::error("PearSiliconRemoteNotifications", 1, "Remote notifications are unavailable in this environment.");
    pear_objc::msg2::<Id, Id, ()>(delegate, "application:didFailToRegisterForRemoteNotificationsWithError:", application, error);
}

/// No APNs environment: the documented failure callback instead of a token
/// that never arrives.
unsafe extern "C" fn register_remote(s: Id, _c: Sel) {
    let app = objc_retain(s) as usize;
    rt::async_main(move || unsafe {
        report_remote_notification_failure(app as Id);
        objc_release(app as Id);
    });
}
unsafe extern "C" fn register_remote_types(s: Id, c: Sel, _types: usize) {
    register_remote(s, c);
}

unsafe fn install_remote_notifications() {
    let application = cls("UIApplication");
    if application.is_null() {
        return;
    }
    replace_instance(application, "registerForRemoteNotifications", register_remote as Imp);
    replace_instance(application, "registerForRemoteNotificationTypes:", register_remote_types as Imp);
    replace_instance(application, "unregisterForRemoteNotifications", noop0 as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][services] remote notifications report unavailable");
}

unsafe fn install_ar_availability() {
    replace_class(cls("ARConfiguration"), "isSupported", ret_no as Imp);
    replace_class(cls("ARWorldTrackingConfiguration"), "isSupported", ret_no as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][services] ARKit reports unsupported");
}

static NEXT_BACKGROUND_TASK: AtomicUsize = AtomicUsize::new(1);
unsafe extern "C" fn begin_task(_s: Id, _c: Sel, _handler: Id) -> usize {
    NEXT_BACKGROUND_TASK.fetch_add(1, Ordering::AcqRel)
}
unsafe extern "C" fn begin_task_named(_s: Id, _c: Sel, _name: Id, _handler: Id) -> usize {
    NEXT_BACKGROUND_TASK.fetch_add(1, Ordering::AcqRel)
}

/// No background-time budget while the window is open: stable local ids.
unsafe fn install_background_tasks() {
    let application = cls("UIApplication");
    if application.is_null() {
        return;
    }
    replace_instance(application, "beginBackgroundTaskWithExpirationHandler:", begin_task as Imp);
    replace_instance(application, "beginBackgroundTaskWithName:expirationHandler:", begin_task_named as Imp);
    replace_instance(application, "endBackgroundTask:", noop1 as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][services] background tasks stay local");
}

fn install_all() {
    unsafe {
        pear_objc::autorelease_pool(|| {
            install_audio_session();
            observe_audio_interruptions();
            install_alert_logging();
            install_video_subscriber();
            install_haptics();
            install_store_surface();
            install_remote_notifications();
            install_ar_availability();
            install_background_tasks();
        });
    }
}

pub fn init() {
    if !rt::module_enabled("services") || disabled() {
        return;
    }
    install_all();
    // Late-loaded StoreKit, ARKit or UIKit classes still get their surface.
    for i in 1..=3 {
        rt::after_main(i as f64 * 2.0, install_all);
    }
}
