//! Objective-C runtime hooks for missing-call reporting: every class PearHost
//! defines answers unknown selectors through `forwardingTargetForSelector:`
//! (reported, then sent to a sink whose IMPs return zero) and unknown KVC keys
//! with nil instead of raising. SIGTERM/SIGINT still write the JSONL summary.
//! Clean-room: public <objc/runtime.h>, <dlfcn.h> and libdispatch APIs.

use crate::{pear_missing_zero, write_log_line, PearMissingReport, PearMissingWriteSummary};
use pear_objc::{cls, msg0, nsstring_to_string, sel_getName, Class, Id, Imp, Sel, NIL};
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::OnceLock;

#[repr(C)]
struct DlInfo {
    dli_fname: *const c_char,
    dli_fbase: *const c_void,
    dli_sname: *const c_char,
    dli_saddr: *const c_void,
}

extern "C" {
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    fn backtrace(frames: *mut *mut c_void, size: c_int) -> c_int;
    fn signal(sig: c_int, handler: usize) -> usize;
    fn _exit(code: c_int) -> !;
    fn free(p: *mut c_void);

    fn objc_copyClassNamesForImage(image: *const c_char, count: *mut c_uint) -> *mut *const c_char;
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn class_isMetaClass(cls: Class) -> bool;
    fn class_getSuperclass(cls: Class) -> Class;
    fn class_getName(cls: Class) -> *const c_char;
    fn class_addMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> bool;
    fn object_getClass(obj: Id) -> Class;
    fn sel_registerName(name: *const c_char) -> Sel;

    static _dispatch_source_type_signal: c_void;
    fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut c_void;
    fn dispatch_source_create(kind: *const c_void, handle: usize, mask: usize, queue: *mut c_void) -> *mut c_void;
    fn dispatch_source_set_event_handler_f(source: *mut c_void, handler: extern "C" fn(*mut c_void));
    fn dispatch_resume(object: *mut c_void);

}

const SIGINT: c_int = 2;
const SIGTERM: c_int = 15;
const SIG_IGN: usize = 1;

const KIND_OBJC_INSTANCE: c_int = 0;
const KIND_OBJC_CLASS: c_int = 1;
const KIND_BEHAVIOR: c_int = 3;

unsafe fn image_path(addr: *const c_void) -> Option<(String, *const c_void)> {
    let mut info: DlInfo = std::mem::zeroed();
    if dladdr(addr, &mut info) == 0 || info.dli_fname.is_null() {
        return None;
    }
    Some((CStr::from_ptr(info.dli_fname).to_string_lossy().into_owned(), info.dli_fbase))
}

/// The first class up the chain that PearHost defines (UIView rather than the
/// game's IOSView); otherwise the receiver's own class.
unsafe fn owner_name(class: Class) -> CString {
    let mut c = class;
    while !c.is_null() {
        if let Some((path, _)) = image_path(c as *const c_void) {
            if path.contains("UIKit.framework/UIKit") {
                return CStr::from_ptr(class_getName(c)).to_owned();
            }
        }
        c = class_getSuperclass(c);
    }
    if class.is_null() {
        CString::new("?").unwrap()
    } else {
        CStr::from_ptr(class_getName(class)).to_owned()
    }
}

/// Runtime and Foundation-internal probes pass through untouched.
unsafe fn should_pass_through(s: Sel) -> bool {
    let n = CStr::from_ptr(sel_getName(s)).to_bytes();
    n.first() == Some(&b'_') || n.starts_with(b"CA")
}

/// The frame that sent the message: skips this image and the runtime's
/// forwarding machinery (CoreFoundation, libobjc).
#[inline(never)]
unsafe fn message_sender() -> *const c_void {
    let mut frames = [std::ptr::null_mut::<c_void>(); 16];
    let n = backtrace(frames.as_mut_ptr(), frames.len() as c_int).max(0) as usize;
    let own_base = image_path(message_sender as *const c_void).map(|(_, b)| b);
    for &f in frames.iter().take(n).skip(1) {
        match image_path(f) {
            None => return f,
            Some((path, base)) => {
                if Some(base) == own_base || path.contains("CoreFoundation") || path.contains("libobjc") {
                    continue;
                }
                return f;
            }
        }
    }
    std::ptr::null()
}

struct Sink(usize, usize); // (class, instance)

static SINK: OnceLock<Sink> = OnceLock::new();

fn sink() -> &'static Sink {
    SINK.get_or_init(|| unsafe {
        let name = c"PearMissingSink";
        let mut c = pear_objc::objc_getClass(name.as_ptr());
        if c.is_null() {
            c = objc_allocateClassPair(cls("NSObject"), name.as_ptr(), 0);
            objc_registerClassPair(c);
        }
        let inst: Id = msg0(c, "new");
        Sink(c as usize, inst as usize)
    })
}

/// Every class PearHost defines: report the unknown selector once, then hand
/// the send to the sink, whose IMP for it is `pear_missing_zero`.
unsafe extern "C" fn forwarding_target(self_obj: Id, _cmd: Sel, s: Sel) -> Id {
    if should_pass_through(s) {
        return NIL;
    }
    let class = object_getClass(self_obj);
    let is_meta = class_isMetaClass(class);
    let owner = owner_name(if is_meta { self_obj as Class } else { class });
    PearMissingReport(
        if is_meta { KIND_OBJC_CLASS } else { KIND_OBJC_INSTANCE },
        owner.as_ptr(),
        sel_getName(s),
        message_sender(),
    );
    let sk = sink();
    class_addMethod(sk.0 as Class, s, pear_missing_zero as Imp, c"@@:".as_ptr());
    sk.1 as Id
}

unsafe fn report_kvc(self_obj: Id, what: &str, key: Id) {
    let k = nsstring_to_string(key).unwrap_or_default();
    let owner = owner_name(object_getClass(self_obj));
    let name = CString::new(format!("{what}{k}")).unwrap_or_default();
    PearMissingReport(KIND_BEHAVIOR, owner.as_ptr(), name.as_ptr(), message_sender());
}

unsafe extern "C" fn value_for_undefined_key(self_obj: Id, _cmd: Sel, key: Id) -> Id {
    report_kvc(self_obj, "valueForKey:", key);
    NIL
}

unsafe extern "C" fn set_value_for_undefined_key(self_obj: Id, _cmd: Sel, _value: Id, key: Id) {
    report_kvc(self_obj, "setValue:forKey:", key);
}

extern "C" fn on_stop_signal(_ctx: *mut c_void) {
    PearMissingWriteSummary();
    unsafe { _exit(0) };
}

/// Harness and user stops arrive as SIGTERM/SIGINT; still write the summary.
fn install_signal_summary_handlers() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| unsafe {
        let q = dispatch_get_global_queue(0, 0);
        for sig in [SIGTERM, SIGINT] {
            signal(sig, SIG_IGN);
            let src = dispatch_source_create(&_dispatch_source_type_signal, sig as usize, 0, q);
            if !src.is_null() {
                dispatch_source_set_event_handler_f(src, on_stop_signal);
                dispatch_resume(src);
            }
        }
    });
}

/// Installs the hooks on every Objective-C class defined in the image that
/// contains `address_in_image`.
#[no_mangle]
pub unsafe extern "C" fn PearMissingInstallForImage(address_in_image: *const c_void) {
    install_signal_summary_handlers();
    let Some((path, _)) = image_path(address_in_image) else { return };
    let Ok(cpath) = CString::new(path.clone()) else { return };
    let mut count: c_uint = 0;
    let names = objc_copyClassNamesForImage(cpath.as_ptr(), &mut count);
    if names.is_null() {
        return;
    }
    let sink_cls = sink().0 as Class;
    let fwd = sel_registerName(c"forwardingTargetForSelector:".as_ptr());
    let vfuk = sel_registerName(c"valueForUndefinedKey:".as_ptr());
    let svfuk = sel_registerName(c"setValue:forUndefinedKey:".as_ptr());
    let mut installed = 0u32;
    for i in 0..count as usize {
        let c = pear_objc::objc_getClass(*names.add(i));
        if c.is_null() || c == sink_cls {
            continue;
        }
        if class_addMethod(c, fwd, forwarding_target as Imp, c"@@::".as_ptr()) {
            installed += 1;
        }
        class_addMethod(object_getClass(c as Id), fwd, forwarding_target as Imp, c"@@::".as_ptr());
        class_addMethod(c, vfuk, value_for_undefined_key as Imp, c"@@:@".as_ptr());
        class_addMethod(c, svfuk, set_value_for_undefined_key as Imp, c"v@:@@".as_ptr());
    }
    free(names as *mut c_void);
    write_log_line(Some("missing"), &format!("reporter installed on {installed} classes of {path}"));
}
