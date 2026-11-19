//! Silences the macOS alert beep for unhandled key events in the loaded app.
//!
//! AppKit plays the alert sound when no responder handles a key; the loaded
//! process has no business beeping. Every zero-argument `…beep…` method on
//! NSApplication becomes a no-op and the C entry point `NSBeep()` (what
//! actually plays the alert) is interposed. Opt out with PEAR_KEEP_BEEP=1;
//! PEAR_DEBUG_BEEP=1 logs what was found.

use crate::rt::{self, Interpose};
use pear_objc::{Id, Imp, Sel, NIL};
use std::ffi::c_void;

extern "C" {
    fn NSBeep();
}

extern "C" fn pear_no_beep() {}

#[used]
#[cfg_attr(not(feature = "smoke"), link_section = "__DATA,__interpose")]
static NSBEEP_INTERPOSE: Interpose = Interpose {
    replacement: pear_no_beep as *const c_void,
    replacee: NSBeep as *const c_void,
};

unsafe extern "C" fn silent(_self: Id, _cmd: Sel) {}

pub fn init() {
    if !rt::module_enabled("services") || rt::env_set("PEAR_KEEP_BEEP") {
        return;
    }
    unsafe {
        // UIKitMacHelper links AppKit; loading it here is harmless if already loaded.
        rt::dlopen_path("/System/Library/Frameworks/AppKit.framework/AppKit", rt::RTLD_LAZY);
        let application = rt::class_named("NSApplication");
        if application.is_null() {
            rt::log("[PearRuntime][beep] NSApplication not found");
            return;
        }
        let debug = rt::env_set("PEAR_DEBUG_BEEP");
        let object = rt::class_named("NSObject");
        let mut swizzled = 0usize;
        let mut c = application;
        while !c.is_null() && c != object {
            let mut count = 0u32;
            let methods = rt::class_copyMethodList(c, &mut count);
            for i in 0..count as usize {
                let m = *methods.add(i);
                let name = rt::sel_getName(rt::method_getName(m));
                if rt::strcasestr(name, c"beep".as_ptr()).is_null() || rt::method_getNumberOfArguments(m) != 2 {
                    continue;
                }
                rt::method_setImplementation(m, silent as Imp);
                swizzled += 1;
                if debug {
                    rt::log(&format!("[PearRuntime][beep] silenced {} on {}", rt::cstr(name), rt::class_name(c)));
                }
            }
            rt::free(methods as *mut c_void);
            c = rt::class_getSuperclass(c);
        }
        let _ = NIL;
        rt::log(&format!(
            "[PearRuntime][beep] NSBeep interposed; {swizzled} method{} silenced",
            if swizzled == 1 { "" } else { "s" }
        ));
    }
}
