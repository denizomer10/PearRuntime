// Global UIKit notification constants, Objective-C exception boundary for launch,
// and image constructor installing PearMissing hooks.
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

void PearUIKitRunCatchingExceptions(void (^block)(void)) {
    @try {
        block();
    } @catch (NSException *e) {
        // AppKit swallows exceptions thrown from launch notifications; an
        // iOS app would have crashed here, so say so loudly and stop.
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

void PearBackendInvokeScriptCall(id target, SEL sel, NSArray<NSString *> *args) {
    NSMethodSignature *sig = [target methodSignatureForSelector:sel];
    NSInvocation *inv = [NSInvocation invocationWithMethodSignature:sig];
    inv.target = target;
    inv.selector = sel;
    NSMutableArray *vals = [NSMutableArray array];
    NSString *joined = [[args subarrayWithRange:NSMakeRange(2, args.count - 2)] componentsJoinedByString:@" "];
    NSScanner *sc = [NSScanner scannerWithString:joined];
    while (!sc.isAtEnd) {
        NSString *v = nil;
        if ([sc scanString:@"\"" intoString:NULL]) {
            [sc scanUpToString:@"\"" intoString:&v];
            [sc scanString:@"\"" intoString:NULL];
            [vals addObject:@[ @"s", v ?: @"" ]];
        } else if ([sc scanUpToString:@" " intoString:&v]) {
            [vals addObject:@[ @"v", v ]];
        }
    }
    NSMutableArray *keep = [NSMutableArray array];
    for (NSUInteger i = 0; i < vals.count && i + 2 < sig.numberOfArguments; i++) {
        const char *type = [sig getArgumentTypeAtIndex:i + 2];
        NSString *kind = vals[i][0], *v = vals[i][1];
        if (type[0] == '@') {
            id o = [kind isEqualToString:@"s"] ? v : ([v isEqualToString:@"nil"] ? nil : v);
            if (o) [keep addObject:o];
            [inv setArgument:&o atIndex:i + 2];
        } else if (type[0] == 'B') {
            BOOL b = [v isEqualToString:@"YES"] || v.intValue != 0;
            [inv setArgument:&b atIndex:i + 2];
        } else if (type[0] == 'c' || type[0] == 'C') {
            char c = (char)([v isEqualToString:@"YES"] ? 1 : v.intValue);
            [inv setArgument:&c atIndex:i + 2];
        } else if (type[0] == 'i' || type[0] == 'I') {
            int x = v.intValue;
            [inv setArgument:&x atIndex:i + 2];
        } else {
            long long x = v.longLongValue;
            [inv setArgument:&x atIndex:i + 2];
        }
    }
    [inv retainArguments];
    [inv invoke];
}

extern void pear_uikit_register_classes(void);

// Install the missing-call reporter on every class in this image as soon as the
// image loads, before the app sends its first message.
__attribute__((constructor)) static void PearUIKitLoad(void) {
    PearHostLog("", "PearHost UIKit %s (no iOSSupport) loading", PEAR_RUNTIME_VERSION);
    pear_uikit_register_classes();
    PearMissingInstallForImage((const void *)&PearUIKitLoad);
}
