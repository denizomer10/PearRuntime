//! Answers the advertising-permission surface locally.
//!
//! A Mac Catalyst process can call the tracking-permission API but the host
//! does not always present the prompt; the completion handler then never runs
//! and an app waits forever after sign-in. This reports a stable local
//! decision; it neither reads nor creates an advertising identifier.
//! Opt out with PEAR_NO_TRACKING_SHIM=1.

use crate::rt;
use pear_objc::{cls, msg0, msg1, nsstring, Class, Id, Imp, Sel};

/// ATTrackingManagerAuthorizationStatusDenied: the privacy-safe answer, and
/// enough for a caller that only waits for a decision.
const DENIED: isize = 2;

unsafe fn tracking_class(name: &str, framework: &str) -> Class {
    let c = cls(name);
    if !c.is_null() {
        return c;
    }
    for root in ["/System/iOSSupport/System/Library/Frameworks", "/System/Library/Frameworks"] {
        rt::dlopen_path(&format!("{root}/{framework}.framework/{framework}"), rt::RTLD_LAZY | rt::RTLD_GLOBAL);
        let c = cls(name);
        if !c.is_null() {
            return c;
        }
    }
    std::ptr::null_mut()
}

unsafe extern "C" fn status(_self: Id, _cmd: Sel) -> isize {
    DENIED
}

unsafe fn call_completion(block: Id, value: usize) {
    if let Some(f) = rt::block_invoke::<unsafe extern "C" fn(Id, isize)>(block) {
        f(block, value as isize);
    }
}

unsafe extern "C" fn request(_self: Id, _cmd: Sel, completion: Id) {
    rt::async_main_with_block(completion, DENIED as usize, call_completion);
}

unsafe extern "C" fn advertising_identifier(_self: Id, _cmd: Sel) -> Id {
    let uuid: Id = msg1(msg0::<Id>(cls("NSUUID"), "alloc"), "initWithUUIDString:", nsstring("00000000-0000-0000-0000-000000000000"));
    rt::autoreleased(uuid)
}

unsafe extern "C" fn tracking_enabled(_self: Id, _cmd: Sel) -> i8 {
    0
}

pub fn init() {
    if !rt::module_enabled("services") || rt::env_set("PEAR_NO_TRACKING_SHIM") {
        return;
    }
    unsafe {
        let tracking = tracking_class("ATTrackingManager", "AppTrackingTransparency");
        if !tracking.is_null() {
            rt::replace_class(tracking, "trackingAuthorizationStatus", status as Imp, "q@:");
            rt::replace_class(tracking, "requestTrackingAuthorizationWithCompletionHandler:", request as Imp, "v@:@?");
        }
        let identifier = tracking_class("ASIdentifierManager", "AdSupport");
        if !identifier.is_null() {
            rt::replace_instance(identifier, "advertisingIdentifier", advertising_identifier as Imp, "@@:");
            rt::replace_instance(identifier, "isAdvertisingTrackingEnabled", tracking_enabled as Imp, "c@:");
        }
        rt::log("[PearRuntime][tracking] local permission surface installed");
    }
}
