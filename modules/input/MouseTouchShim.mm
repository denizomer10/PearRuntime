// MouseTouchShim.mm: mouse-to-touch fallback for games without native mouse touches.
// Enabled by default; the handler yields to a native UIKit touch when one arrives.
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import "PearTouchGeometry.h"

extern void PearSynthesizeVirtualFinger(BOOL down, NSString *key, CGFloat nx, CGFloat ny);
extern void PearSendLegacyDirectTouch(BOOL down, CGFloat nx, CGFloat ny);
extern void PearCancelLegacyDirectTouch(void);
extern void PearRunLegacyDirectTouchSelfTest(void);
extern CGRect PearWindowVisibleContentRect(id window, id contentView);
static BOOL gAdaptiveDirectTouch;

// Only engine-owned rendering views use responder delivery. UIKit controls,
// scroll views and gesture-recognizer surfaces retain the system event path.
static BOOL PearCanDeliverEngineTouch(CGPoint normalized) {
    UIWindow *window = PearPrimaryWindow();
    CGPoint point = CGPointMake(CGRectGetMinX(window.bounds) + normalized.x * window.bounds.size.width,
                               CGRectGetMinY(window.bounds) + normalized.y * window.bounds.size.height);
    UIView *view = [window hitTest:point withEvent:nil];
    if (!view || [view isKindOfClass:UIControl.class] || [view isKindOfClass:UIScrollView.class] || view.gestureRecognizers.count) return NO;
    Class metal = NSClassFromString(@"CAMetalLayer"), gl = NSClassFromString(@"CAEAGLLayer");
    if (!((metal && [view.layer isKindOfClass:metal]) || (gl && [view.layer isKindOfClass:gl]))) return NO;
    SEL began = @selector(touchesBegan:withEvent:);
    return class_getMethodImplementation(view.class, began) != class_getMethodImplementation(UIView.class, began);
}

static void PearSendMouseTouch(BOOL down, CGFloat nx, CGFloat ny) {
    if (gAdaptiveDirectTouch || NSProcessInfo.processInfo.environment[@"PEAR_DIRECT_LEGACY_TOUCH"].boolValue)
        PearSendLegacyDirectTouch(down, nx, ny);
    else
        PearSynthesizeVirtualFinger(down, @"f:mouseclick", nx, ny);
}

static uint64_t gNativeTouchGeneration = 0;
static id gMouseTouchMonitor = nil;

static void InstallNativeTouchTracking(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        Class cls = NSClassFromString(@"UIApplication");
        SEL sel = NSSelectorFromString(@"sendEvent:");
        Method method = cls ? class_getInstanceMethod(cls, sel) : NULL;
        if (!method) return;
        IMP original = method_getImplementation(method);
        // imp_implementationWithBlock blocks receive the receiver followed by
        // the method arguments; the selector is not passed. Declaring a SEL
        // parameter shifts every argument and hands UIKit a garbage event.
        method_setImplementation(method, imp_implementationWithBlock(^(id self, id event) {
            @try {
                NSSet *touches = [event respondsToSelector:@selector(allTouches)]
                    ? ((id (*)(id, SEL))objc_msgSend)(event, @selector(allTouches)) : nil;
                // Pointer hover/move events also carry allTouches. They are
                // not delivery of a press and must not suppress its fallback.
                BOOL began = NO;
                for (UITouch *touch in touches) {
                    if (touch.phase == UITouchPhaseBegan) { began = YES; break; }
                }
                if (began) {
                    gNativeTouchGeneration++;
                    if (getenv("PEAR_DEBUG_TOUCH"))
                        PearLog(@"[PearRuntime][mouse-touch] UIKit touches=%lu generation=%llu", (unsigned long)touches.count, (unsigned long long)gNativeTouchGeneration);
                }
            } @catch (__unused NSException *exception) {}
            ((void (*)(id, SEL, id))original)(self, sel, event);
        }));
    });
}

void PearInstallMouseToTouchFallback(void) {
    // A native UIKit touch and a synthetic finger must never both be
    // generated. A short grace period lets UIKit deliver a native touch
    // before the fallback starts, which keeps normal mouse handling intact.
    if (!NSProcessInfo.processInfo.environment[@"PEAR_MOUSE_TOUCH"].boolValue) return;
    InstallNativeTouchTracking();
    Class eventClass = NSClassFromString(@"NSEvent");
    SEL add = NSSelectorFromString(@"addLocalMonitorForEventsMatchingMask:handler:");
    if (!eventClass || ![eventClass respondsToSelector:add]) return;
    __block BOOL pending = NO, held = NO, synthetic = NO;
    __block uint64_t serial = 0, nativeGeneration = 0;
    __block CGPoint start = CGPointZero, last = CGPointZero;
    __block id gestureWindow = nil;
    void (^cancel)(void) = ^{
        if (synthetic) PearSendMouseTouch(NO, last.x, last.y);
        gAdaptiveDirectTouch = NO;
        pending = held = synthetic = NO;
        gestureWindow = nil;
        serial++;
    };
    NSUInteger mask = (1u << 1) | (1u << 2) | (1u << 6); // left down/up/drag
    id (^handler)(id) = ^id(id event) {
        NSInteger type = ((NSInteger (*)(id, SEL))objc_msgSend)(event, @selector(type));
        BOOL directGesture = pending && NSProcessInfo.processInfo.environment[@"PEAR_DIRECT_LEGACY_TOUCH"].boolValue;
        id window = ((id (*)(id, SEL))objc_msgSend)(event, @selector(window));
        if (type == 1) {
            cancel();
            if (UIApplication.sharedApplication.applicationState != UIApplicationStateActive ||
                !window || !((BOOL (*)(id, SEL))objc_msgSend)(window, @selector(isKeyWindow))) return event;
        } else if (!pending || window != gestureWindow) {
            if (pending && type == 2) cancel();
            return event;
        }
        id content = window ? ((id (*)(id, SEL))objc_msgSend)(window, @selector(contentView)) : nil;
        if (!content) { cancel(); return event; }
        // Only the area below the title bar is game content: the full-size
        // content view also covers the title bar, and treating clicks there
        // as touches swallowed the window's close/minimize/zoom buttons.
        NSRect bounds = PearWindowVisibleContentRect(window, content);
        NSPoint point = ((NSPoint (*)(id, SEL))objc_msgSend)(event, @selector(locationInWindow));
        point = ((NSPoint (*)(id, SEL, NSPoint, id))objc_msgSend)(content, @selector(convertPoint:fromView:), point, nil);
        if (NSProcessInfo.processInfo.environment[@"PEAR_LEGACY_ATTACH_LAYER"].boolValue) {
            UIWindow *uiWindow = PearPrimaryWindow();
            CGRect fitted = uiWindow ? PearAspectFitRect(uiWindow.bounds.size, bounds) : CGRectZero;
            if (!CGRectIsEmpty(fitted)) bounds = fitted;
        }
        if (bounds.size.width <= 0 || bounds.size.height <= 0 ||
            (type == 1 && !NSPointInRect(point, bounds))) return event;
        CGFloat nx = MIN(MAX((point.x - bounds.origin.x) / bounds.size.width, 0), 1);
        CGFloat y = MIN(MAX((point.y - bounds.origin.y) / bounds.size.height, 0), 1);
        BOOL flipped = ((BOOL (*)(id, SEL))objc_msgSend)(content, @selector(isFlipped));
        last = CGPointMake(nx, flipped ? y : 1.0 - y);
        if (type == 1) {
            pending = held = YES;
            start = last;
            gestureWindow = window;
            nativeGeneration = gNativeTouchGeneration;
            uint64_t thisGesture = ++serial;
            if (getenv("PEAR_DEBUG_TOUCH")) PearLog(@"[PearRuntime][mouse-touch] down normalized=(%.3f,%.3f) generation=%llu", start.x, start.y, (unsigned long long)nativeGeneration);
            // This explicitly selected path owns input for the reattached
            // legacy window. Preserve the timing of real NSEvent gestures;
            // waiting for absent UIKit routing collapses short swipes.
            if (NSProcessInfo.processInfo.environment[@"PEAR_DIRECT_LEGACY_TOUCH"].boolValue) {
                synthetic = YES;
                PearSendMouseTouch(YES, start.x, start.y);
                return nil;
            }
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.22 * NSEC_PER_SEC)),
                           dispatch_get_main_queue(), ^{
                if (!pending || serial != thisGesture) return;
                if (nativeGeneration != gNativeTouchGeneration ||
                    UIApplication.sharedApplication.applicationState != UIApplicationStateActive ||
                    !((BOOL (*)(id, SEL))objc_msgSend)(gestureWindow, @selector(isKeyWindow))) {
                    if (getenv("PEAR_DEBUG_TOUCH")) PearLog(@"[PearRuntime][mouse-touch] fallback cancelled native=%d active=%d key=%d", nativeGeneration != gNativeTouchGeneration, UIApplication.sharedApplication.applicationState == UIApplicationStateActive, (int)((BOOL (*)(id, SEL))objc_msgSend)(gestureWindow, @selector(isKeyWindow)));
                    cancel();
                    return;
                }
                synthetic = YES;
                gAdaptiveDirectTouch = PearCanDeliverEngineTouch(start);
                if (getenv("PEAR_DEBUG_TOUCH")) PearLog(@"[PearRuntime][mouse-touch] engine responder fallback=%d", gAdaptiveDirectTouch);
                if (getenv("PEAR_DEBUG_TOUCH")) PearLog(@"[PearRuntime][mouse-touch] fallback begin (%.3f,%.3f)", start.x, start.y);
                PearSendMouseTouch(YES, start.x, start.y);
                if (!CGPointEqualToPoint(start, last))
                    PearSendMouseTouch(YES, last.x, last.y);
                if (!held) {
                    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.06 * NSEC_PER_SEC)),
                                   dispatch_get_main_queue(), ^{
                        if (pending && serial == thisGesture && !held) cancel();
                    });
                }
            });
        } else if (type == 6) {
            if (getenv("PEAR_DEBUG_TOUCH")) PearLog(@"[PearRuntime][mouse-touch] drag normalized=(%.3f,%.3f) synthetic=%d", last.x, last.y, synthetic);
            if (synthetic) PearSendMouseTouch(YES, last.x, last.y);
        } else if (type == 2) {
            if (getenv("PEAR_DEBUG_TOUCH")) PearLog(@"[PearRuntime][mouse-touch] up normalized=(%.3f,%.3f) synthetic=%d", last.x, last.y, synthetic);
            held = NO;
            if (synthetic) cancel();
        }
        return directGesture ? nil : event;
    };
    gMouseTouchMonitor = ((id (*)(id, SEL, NSUInteger, id))objc_msgSend)(eventClass, add, mask, handler);
    [NSNotificationCenter.defaultCenter addObserverForName:UIApplicationWillResignActiveNotification
        object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) {
            if (synthetic && (gAdaptiveDirectTouch || NSProcessInfo.processInfo.environment[@"PEAR_DIRECT_LEGACY_TOUCH"].boolValue)) {
                PearCancelLegacyDirectTouch();
                synthetic = NO;
            }
            cancel();
        }];
    PearLog(@"[PearRuntime][touch] mouse-to-touch fallback %@", gMouseTouchMonitor ? @"installed" : @"unavailable");
    PearRunLegacyDirectTouchSelfTest();
}

