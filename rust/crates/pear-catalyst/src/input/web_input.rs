//! Makes date-of-birth fields in an app's web consent form usable with a
//! keyboard. Web age gates often use a read-only <input> plus a JavaScript
//! wheel picker that never opens with a mouse. A user script injected into
//! every WKWebView makes those inputs typeable and normalizes digits to the
//! YYYY-MM value the page expects. It never chooses a date or submits the
//! form. Opt out with PEAR_NO_WEB_INPUT_SHIM=1.

use crate::rt;
use pear_objc::{cls, msg0, msg1, msg3, nsstring, objc_getAssociatedObject, objc_release, objc_setAssociatedObject, sel, CGRect, Id, Imp, Sel};
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

const DATE_INPUT_SCRIPT: &str = include_str!("date_input.js");
static CONFIGURATION_KEY: u8 = 0;
static ORIGINAL_INIT_FRAME: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_INIT_CODER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

fn disabled() -> bool {
    matches!(std::env::var("PEAR_NO_WEB_INPUT_SHIM"), Ok(v) if !v.is_empty() && v != "0")
}

unsafe fn install_date_input_script(configuration: Id) {
    if configuration.is_null() {
        return;
    }
    let key = &CONFIGURATION_KEY as *const u8 as *const c_void;
    if !objc_getAssociatedObject(configuration, key).is_null() {
        return;
    }
    objc_setAssociatedObject(configuration, key, pear_objc::nsnumber_bool(true), 1);
    let mut controller: Id = msg0(configuration, "userContentController");
    if controller.is_null() {
        let controller_class = cls("WKUserContentController");
        if controller_class.is_null() {
            return;
        }
        controller = msg0(controller_class, "new");
        msg1::<Id, ()>(configuration, "setUserContentController:", controller);
        objc_release(controller);
        controller = msg0(configuration, "userContentController");
    }
    let script_class = cls("WKUserScript");
    if script_class.is_null() || controller.is_null() {
        return;
    }
    // WKUserScriptInjectionTimeAtDocumentEnd == 1
    let script: Id = msg3(
        msg0::<Id>(script_class, "alloc"),
        "initWithSource:injectionTime:forMainFrameOnly:",
        nsstring(DATE_INPUT_SCRIPT),
        1isize,
        false,
    );
    if script.is_null() {
        return;
    }
    msg1::<Id, ()>(controller, "addUserScript:", script);
    objc_release(script);
    rt::log("[PearRuntime][web] date input shim installed");
}

unsafe extern "C" fn init_with_frame(s: Id, c: Sel, frame: CGRect, configuration: Id) -> Id {
    install_date_input_script(configuration);
    let f: unsafe extern "C" fn(Id, Sel, CGRect, Id) -> Id = std::mem::transmute(ORIGINAL_INIT_FRAME.load(Ordering::Acquire));
    f(s, c, frame, configuration)
}

/// Storyboard/NIB web views skip the frame initializer; the script goes on
/// the configuration they decode and applies from the first navigation.
unsafe extern "C" fn init_with_coder(s: Id, c: Sel, decoder: Id) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(ORIGINAL_INIT_CODER.load(Ordering::Acquire));
    let web_view = f(s, c, decoder);
    if !web_view.is_null() {
        install_date_input_script(msg0(web_view, "configuration"));
    }
    web_view
}

unsafe fn hook(c: pear_objc::Class, selector: &str, imp: Imp, slot: &AtomicPtr<c_void>) {
    let m = rt::class_getInstanceMethod(c, sel(selector));
    if !m.is_null() {
        slot.store(rt::method_getImplementation(m) as *mut c_void, Ordering::Release);
        rt::method_setImplementation(m, imp);
    }
}

pub fn init() {
    if !rt::module_enabled("input") || disabled() {
        return;
    }
    unsafe {
        let web_view = cls("WKWebView");
        if web_view.is_null() {
            return;
        }
        hook(web_view, "initWithFrame:configuration:", init_with_frame as Imp, &ORIGINAL_INIT_FRAME);
        hook(web_view, "initWithCoder:", init_with_coder as Imp, &ORIGINAL_INIT_CODER);
    }
}
