//! Apple silicon scheduling and latency tuning for iOS games on the Mac.
//!
//! 1. App Nap and timer coalescing: a latency-critical, user-initiated
//!    activity turns both off for the life of the game.
//! 2. Efficiency cores: threads created without a QoS (and without an
//!    explicit scheduling policy) are promoted, user-initiated by default and
//!    user-interactive in Game Mode. Threads asking for a QoS or a real-time
//!    policy (audio) are never touched.
//! 3. Game Mode: the runtime side is the stronger thread QoS above.
//!
//! PEAR_GAME_MODE=0, PEAR_THREAD_QOS=off|initiated|interactive,
//! PEAR_NO_LATENCY_ACTIVITY.

use crate::rt::{self, Interpose};
use pear_objc::{cls, msg0, msg2, nsstring, objc_retain, Id};
use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::OnceLock;

const QOS_UNSPECIFIED: u32 = 0x00;
const QOS_USER_INITIATED: u32 = 0x19;
const QOS_USER_INTERACTIVE: u32 = 0x21;
const PTHREAD_EXPLICIT_SCHED: c_int = 2;

/// Darwin `pthread_attr_t` (`long __sig; char __opaque[56]`).
#[repr(C)]
#[derive(Copy, Clone)]
struct PthreadAttr {
    sig: i64,
    opaque: [u8; 56],
}

type StartFn = extern "C" fn(*mut c_void) -> *mut c_void;

extern "C" {
    fn pthread_create(thread: *mut usize, attr: *const PthreadAttr, start: StartFn, arg: *mut c_void) -> c_int;
    fn pthread_attr_init(attr: *mut PthreadAttr) -> c_int;
    fn pthread_attr_destroy(attr: *mut PthreadAttr) -> c_int;
    fn pthread_attr_get_qos_class_np(attr: *const PthreadAttr, qos: *mut u32, relative: *mut c_int) -> c_int;
    fn pthread_attr_set_qos_class_np(attr: *mut PthreadAttr, qos: u32, relative: c_int) -> c_int;
    fn pthread_attr_getinheritsched(attr: *const PthreadAttr, inherit: *mut c_int) -> c_int;
    fn pthread_set_qos_class_self_np(qos: u32, relative: c_int) -> c_int;
}

/// QoS for threads created without one; unspecified disables the promotion.
/// Resolved once in the initializer, before the first interposed call can race.
static THREAD_QOS: AtomicU32 = AtomicU32::new(QOS_UNSPECIFIED);
static LATENCY_ACTIVITY: AtomicUsize = AtomicUsize::new(0);

fn game_mode() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| match std::env::var("PEAR_GAME_MODE") {
        Ok(v) => !(v == "0" || v.eq_ignore_ascii_case("off") || v.eq_ignore_ascii_case("false")),
        Err(_) => true,
    })
}

fn resolve_thread_qos() {
    if !rt::module_enabled("performance") {
        return;
    }
    let value = std::env::var("PEAR_THREAD_QOS").ok();
    let qos = match value.as_deref() {
        Some(v) if v.eq_ignore_ascii_case("off") || v == "0" => return,
        Some(v) if v.eq_ignore_ascii_case("interactive") => QOS_USER_INTERACTIVE,
        Some(v) if v.eq_ignore_ascii_case("initiated") => QOS_USER_INITIATED,
        _ => {
            if game_mode() {
                QOS_USER_INTERACTIVE
            } else {
                QOS_USER_INITIATED
            }
        }
    };
    THREAD_QOS.store(qos, Ordering::Release);
}

/// dyld applies interposing to every image except this one, so the
/// pthread_create calls here reach the real implementation.
unsafe extern "C" fn pear_pthread_create(
    thread: *mut usize,
    attr: *const PthreadAttr,
    start: StartFn,
    arg: *mut c_void,
) -> c_int {
    let target = THREAD_QOS.load(Ordering::Acquire);
    if target == QOS_UNSPECIFIED {
        return pthread_create(thread, attr, start, arg);
    }
    let mut local: PthreadAttr = std::mem::zeroed();
    if !attr.is_null() {
        let mut requested = QOS_UNSPECIFIED;
        let mut relative = 0;
        if pthread_attr_get_qos_class_np(attr, &mut requested, &mut relative) == 0 && requested != QOS_UNSPECIFIED {
            return pthread_create(thread, attr, start, arg);
        }
        let mut inherit = 0;
        if pthread_attr_getinheritsched(attr, &mut inherit) == 0 && inherit == PTHREAD_EXPLICIT_SCHED {
            // An explicit policy (real-time audio, fixed priority) wins.
            return pthread_create(thread, attr, start, arg);
        }
        local = *attr;
    } else {
        pthread_attr_init(&mut local);
    }
    pthread_attr_set_qos_class_np(&mut local, target, 0);
    let result = pthread_create(thread, &local, start, arg);
    if attr.is_null() {
        pthread_attr_destroy(&mut local);
    }
    result
}

#[used]
#[cfg_attr(not(feature = "smoke"), link_section = "__DATA,__interpose")]
static PTHREAD_CREATE_INTERPOSE: Interpose = Interpose {
    replacement: pear_pthread_create as *const c_void,
    replacee: pthread_create as *const c_void,
};

unsafe fn begin_latency_activity() {
    if rt::env_set("PEAR_NO_LATENCY_ACTIVITY") {
        return;
    }
    // NSActivityLatencyCritical is declared for macOS only, but the process is
    // a Mac process and Foundation honours the bit under Catalyst as well.
    const LATENCY_CRITICAL: u64 = 0xFF_0000_0000;
    const USER_INITIATED_ALLOWING_IDLE_SLEEP: u64 = 0x00FF_FFFF & !(1 << 20);
    let info: Id = msg0(cls("NSProcessInfo"), "processInfo");
    let activity: Id = msg2(
        info,
        "beginActivityWithOptions:reason:",
        USER_INITIATED_ALLOWING_IDLE_SLEEP | LATENCY_CRITICAL,
        nsstring("PearRuntime: game loop timing"),
    );
    if !activity.is_null() {
        LATENCY_ACTIVITY.store(objc_retain(activity) as usize, Ordering::Release);
    }
}

pub fn init() {
    resolve_thread_qos();
    if !rt::module_enabled("performance") {
        return;
    }
    unsafe {
        // The main thread drives most iOS game loops (CADisplayLink on the main
        // run loop); nothing should have launched it below user-interactive.
        pthread_set_qos_class_self_np(QOS_USER_INTERACTIVE, 0);
        begin_latency_activity();
    }
    let qos = match THREAD_QOS.load(Ordering::Acquire) {
        QOS_USER_INTERACTIVE => "user-interactive",
        QOS_USER_INITIATED => "user-initiated",
        _ => "off",
    };
    rt::log(&format!(
        "[PearRuntime][perf] game mode={} thread qos={qos} latency activity={}",
        if game_mode() { "on" } else { "off" },
        if LATENCY_ACTIVITY.load(Ordering::Acquire) != 0 { "on" } else { "off" }
    ));
}
