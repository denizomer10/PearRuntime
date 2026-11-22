//! UIKit.framework ABI data iOS binaries bind to (exported NSString
//! constants), and the Objective-C exception boundaries of the UIKit layer
//! (pear_objc::abi catches them; Rust cannot).

use pear_objc::{msg0, nsarray_count, nsarray_object_at, nsstring_to_string, objc_release, sel, Class, Id, Sel};
use std::ffi::{c_char, c_void, CString};

pear_objc::nsstring_constant!(UIApplicationDidFinishLaunchingNotification = "UIApplicationDidFinishLaunchingNotification");
pear_objc::nsstring_constant!(UIApplicationDidBecomeActiveNotification = "UIApplicationDidBecomeActiveNotification");
pear_objc::nsstring_constant!(UIApplicationWillResignActiveNotification = "UIApplicationWillResignActiveNotification");
pear_objc::nsstring_constant!(UIApplicationDidEnterBackgroundNotification = "UIApplicationDidEnterBackgroundNotification");
pear_objc::nsstring_constant!(UIApplicationWillEnterForegroundNotification = "UIApplicationWillEnterForegroundNotification");
pear_objc::nsstring_constant!(UIApplicationWillTerminateNotification = "UIApplicationWillTerminateNotification");
pear_objc::nsstring_constant!(UIApplicationDidReceiveMemoryWarningNotification = "UIApplicationDidReceiveMemoryWarningNotification");

pear_objc::nsstring_constant!(UIKeyboardWillShowNotification = "UIKeyboardWillShowNotification");
pear_objc::nsstring_constant!(UIKeyboardDidShowNotification = "UIKeyboardDidShowNotification");
pear_objc::nsstring_constant!(UIKeyboardWillHideNotification = "UIKeyboardWillHideNotification");
pear_objc::nsstring_constant!(UIKeyboardDidHideNotification = "UIKeyboardDidHideNotification");
pear_objc::nsstring_constant!(UIKeyboardWillChangeFrameNotification = "UIKeyboardWillChangeFrameNotification");
pear_objc::nsstring_constant!(UIKeyboardDidChangeFrameNotification = "UIKeyboardDidChangeFrameNotification");
pear_objc::nsstring_constant!(UIKeyboardFrameBeginUserInfoKey = "UIKeyboardFrameBeginUserInfoKey");
pear_objc::nsstring_constant!(UIKeyboardFrameEndUserInfoKey = "UIKeyboardFrameEndUserInfoKey");
pear_objc::nsstring_constant!(UIKeyboardAnimationDurationUserInfoKey = "UIKeyboardAnimationDurationUserInfoKey");
pear_objc::nsstring_constant!(UIKeyboardAnimationCurveUserInfoKey = "UIKeyboardAnimationCurveUserInfoKey");
pear_objc::nsstring_constant!(UIKeyboardIsLocalUserInfoKey = "UIKeyboardIsLocalUserInfoKey");
pear_objc::nsstring_constant!(UITextViewTextDidBeginEditingNotification = "UITextViewTextDidBeginEditingNotification");
pear_objc::nsstring_constant!(UITextViewTextDidChangeNotification = "UITextViewTextDidChangeNotification");
pear_objc::nsstring_constant!(UITextViewTextDidEndEditingNotification = "UITextViewTextDidEndEditingNotification");
pear_objc::nsstring_constant!(UISceneWillDeactivateNotification = "UISceneWillDeactivateNotification");
pear_objc::nsstring_constant!(UISceneDidDisconnectNotification = "UISceneDidDisconnectNotification");
pear_objc::nsstring_constant!(UIPointerLockStateDidChangeNotification = "UIPointerLockStateDidChangeNotification");
pear_objc::nsstring_constant!(UIPointerLockStateSceneUserInfoKey = "UIPointerLockStateSceneUserInfoKey");
pear_objc::nsstring_constant!(UIWindowSceneSessionRoleApplication = "UIWindowSceneSessionRoleApplication");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternCalendarEvent = "com.apple.uikit.pasteboard-detection-pattern.calendar-event");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternEmailAddress = "com.apple.uikit.pasteboard-detection-pattern.email-address");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternFlightNumber = "com.apple.uikit.pasteboard-detection-pattern.flight-number");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternLink = "com.apple.uikit.pasteboard-detection-pattern.link");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternMoneyAmount = "com.apple.uikit.pasteboard-detection-pattern.money-amount");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternNumber = "com.apple.uikit.pasteboard-detection-pattern.number");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternPhoneNumber = "com.apple.uikit.pasteboard-detection-pattern.phone-number");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternPostalAddress = "com.apple.uikit.pasteboard-detection-pattern.postal-address");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternProbableWebSearch = "com.apple.uikit.pasteboard-detection-pattern.probable-web-search");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternProbableWebURL = "com.apple.uikit.pasteboard-detection-pattern.probable-web-url");
pear_objc::nsstring_constant!(UIPasteboardDetectionPatternShipmentTrackingNumber = "com.apple.uikit.pasteboard-detection-pattern.shipment-tracking-number");

extern "C" {
    #[link_name = "OBJC_CLASS_$_NSObject"]
    static NSObject_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_NSObject"]
    static NSObject_META: c_void;
}

pear_objc::empty_class!(UITraitHorizontalSizeClass : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(UITraitVerticalSizeClass : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(UIViewLayoutRegion : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(UISceneSession : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(UISceneConnectionOptions : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(UISceneConfiguration : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(UIScreenMode : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(PearLayoutConstraint : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(PearLayoutAnchor : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(UIWindowSceneGeometry : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);

extern "C" {
    fn PearHostLogMessage(area: *const c_char, message: *const c_char);
    fn abort() -> !;
}

fn log_app(message: &str) {
    let m = CString::new(message.replace('\0', "")).unwrap_or_default();
    unsafe { PearHostLogMessage(c"app".as_ptr(), m.as_ptr()) };
}

unsafe fn text(value: Id) -> String {
    if value.is_null() { String::new() } else { nsstring_to_string(value).unwrap_or_default() }
}

/// Runs fn(ctx); an Objective-C exception escaping it is logged with its
/// stack and aborts. AppKit swallows exceptions thrown from launch
/// notifications, but an iOS app would have crashed there, so say so loudly
/// and stop.
#[no_mangle]
pub unsafe extern "C" fn PearUIKitRunCatchingExceptions(ctx: *mut c_void, f: unsafe extern "C-unwind" fn(*mut c_void)) {
    if let Err(e) = pear_objc::abi::try_call(ctx, f) {
        log_app(&format!("launch raised {}: {}", text(msg0(e, "name")), text(msg0(e, "reason"))));
        let frames: Id = msg0(e, "callStackSymbols");
        for i in 0..nsarray_count(frames) {
            log_app(&format!("  {}", text(nsarray_object_at(frames, i))));
        }
        abort();
    }
}

/// `[obj setValue:value forKey:key]`, false if it raised.
#[no_mangle]
pub unsafe extern "C" fn PearUIKitTrySetValueForKey(obj: Id, value: Id, key: Id) -> bool {
    match pear_objc::abi::try_send(obj, sel("setValue:forKey:"), value as usize, key as usize) {
        Ok(()) => true,
        Err(e) => {
            objc_release(e);
            false
        }
    }
}

/// Sends the display-link selector (a setter with 1, or a getter), false if
/// it raised.
#[no_mangle]
pub unsafe extern "C" fn PearUIKitTryProbeDisplayLinkSelector(target: Id, selector: Sel, is_setter: bool) -> bool {
    match pear_objc::abi::try_send(target, selector, is_setter as usize, 0) {
        Ok(()) => true,
        Err(e) => {
            objc_release(e);
            false
        }
    }
}

/// This image's UIFont (UIFoundation defines a class with the same name).
#[no_mangle]
pub extern "C" fn pear_uikit_uifont_class() -> Class {
    crate::objc_classes::UIFont::CLASS.ptr() as Class
}

/// UIGeometry's zero NSDirectionalEdgeInsets (top, leading, bottom, trailing).
/// iOS UIKit defines it; macOS AppKit's copy is no longer re-exported.
#[no_mangle]
pub static NSDirectionalEdgeInsetsZero: [f64; 4] = [0.0, 0.0, 0.0, 0.0];
