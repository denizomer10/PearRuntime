// UIKit.framework ABI shim: exported NSString constants iOS binaries bind to,
// and the Objective-C exception boundaries Rust cannot express (@try/@catch).
// The image initializer and all behavior are Rust (rust/crates/pear-uikit).
#import "PearUIKitPrivate.h"
#import <objc/message.h>

PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidFinishLaunchingNotification = @"UIApplicationDidFinishLaunchingNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidBecomeActiveNotification = @"UIApplicationDidBecomeActiveNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationWillResignActiveNotification = @"UIApplicationWillResignActiveNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidEnterBackgroundNotification = @"UIApplicationDidEnterBackgroundNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationWillEnterForegroundNotification = @"UIApplicationWillEnterForegroundNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationWillTerminateNotification = @"UIApplicationWillTerminateNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidReceiveMemoryWarningNotification = @"UIApplicationDidReceiveMemoryWarningNotification";

PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardWillShowNotification = @"UIKeyboardWillShowNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardDidShowNotification = @"UIKeyboardDidShowNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardWillHideNotification = @"UIKeyboardWillHideNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardDidHideNotification = @"UIKeyboardDidHideNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardWillChangeFrameNotification = @"UIKeyboardWillChangeFrameNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardDidChangeFrameNotification = @"UIKeyboardDidChangeFrameNotification";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardFrameBeginUserInfoKey = @"UIKeyboardFrameBeginUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardFrameEndUserInfoKey = @"UIKeyboardFrameEndUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardAnimationDurationUserInfoKey = @"UIKeyboardAnimationDurationUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardAnimationCurveUserInfoKey = @"UIKeyboardAnimationCurveUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardIsLocalUserInfoKey = @"UIKeyboardIsLocalUserInfoKey";
PEAR_UIKIT_EXPORT NSNotificationName const UITextViewTextDidBeginEditingNotification = @"UITextViewTextDidBeginEditingNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UITextViewTextDidChangeNotification = @"UITextViewTextDidChangeNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UITextViewTextDidEndEditingNotification = @"UITextViewTextDidEndEditingNotification";

// Runs fn(ctx); an Objective-C exception escaping it is logged with its stack
// and aborts. AppKit swallows exceptions thrown from launch notifications, but
// an iOS app would have crashed there, so say so loudly and stop.
void PearUIKitRunCatchingExceptions(void *ctx, void (*fn)(void *)) {
    @try {
        fn(ctx);
    } @catch (NSException *e) {
        PearHostLog("app", "launch raised %s: %s", e.name.UTF8String ?: "", e.reason.UTF8String ?: "");
        for (NSString *f in e.callStackSymbols) {
            PearHostLog("app", "  %s", f.UTF8String ?: "");
        }
        abort();
    }
}

BOOL PearUIKitTrySetValueForKey(id obj, id value, NSString *key) {
    @try {
        [obj setValue:value forKey:key];
        return YES;
    } @catch (NSException *e) {
        return NO;
    }
}

BOOL PearUIKitTryProbeDisplayLinkSelector(id target, SEL sel, BOOL isSetter) {
    @try {
        if (isSetter) {
            ((void (*)(id, SEL, NSInteger))objc_msgSend)(target, sel, 1);
        } else {
            ((NSInteger (*)(id, SEL))objc_msgSend)(target, sel);
        }
        return YES;
    } @catch (NSException *e) {
        return NO;
    }
}
