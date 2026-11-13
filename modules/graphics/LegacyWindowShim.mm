// LegacyWindowShim.mm: scene-less UIKit window compatibility.
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import "PearTouchGeometry.h"
#import <objc/runtime.h>
#import <objc/message.h>

// Legacy iPhone-only apps without a scene manifest can reach UINSWindow's
// "should have proxy before assigning wc" assertion on current macOS. The
// minimum-size pass is not available before the proxy exists; skip that one
// pass and let UIKit attach the window normally.
static BOOL PearIsLegacyApp(void) {
    if (NSProcessInfo.processInfo.environment[@"PEAR_LEGACY_WINDOW_FIX"].boolValue) return YES;
    NSDictionary *info = NSBundle.mainBundle.infoDictionary;
    if (info[@"UIApplicationSceneManifest"]) return NO;
    NSString *sdk = [(NSString *)info[@"DTSDKName"] ?: @"" lowercaseString];
    NSString *version = [sdk hasPrefix:@"iphoneos"]
        ? [sdk substringFromIndex:@"iphoneos".length] : sdk;
    double major = version.doubleValue;
    return major > 0 && major < 15.0;
}

/// The part of a window's content view that is not under the title bar, in
/// the content view's coordinates. Catalyst windows use a full-size content
/// view, so its bounds include the title bar strip. Full screen has no title
/// bar and returns the whole view.
CGRect PearWindowVisibleContentRect(id window, id contentView) {
    if (!contentView) return CGRectZero;
    CGRect bounds = ((CGRect (*)(id, SEL))objc_msgSend)(contentView, @selector(bounds));
    SEL layoutSel = NSSelectorFromString(@"contentLayoutRect");
    if (!window || ![window respondsToSelector:layoutSel]) return bounds;
    NSUInteger mask = ((NSUInteger (*)(id, SEL))objc_msgSend)(window, @selector(styleMask));
    if (mask & (1 << 14)) return bounds;  // full screen
    CGRect layout = ((CGRect (*)(id, SEL))objc_msgSend)(window, layoutSel);
    CGRect converted = ((CGRect (*)(id, SEL, CGRect, id))objc_msgSend)(
        contentView, NSSelectorFromString(@"convertRect:fromView:"), layout, nil);
    CGRect visible = CGRectIntersection(converted, bounds);
    return (visible.size.width >= 1 && visible.size.height >= 1) ? visible : bounds;
}

static BOOL gLegacyWindowFixInstalled = NO;
static char kPearAttachedLegacyRoot;

static void PearInstallLegacyWindowFixNow(void) {
    if (gLegacyWindowFixInstalled) return;
    Class cls = NSClassFromString(@"UINSWindow");
    SEL sel = NSSelectorFromString(@"_setContentMinSizeFromScene");
    Method method = cls ? class_getInstanceMethod(cls, sel) : NULL;
    if (!method) return;
    method_setImplementation(method, imp_implementationWithBlock(^(id self) {
        PearLog(@"[PearRuntime][window] skipped pre-proxy content min size");
    }));
    Class container = NSClassFromString(@"UINSSceneContainerView");
    Method constraints = container ? class_getInstanceMethod(container, NSSelectorFromString(@"updateConstraints")) : NULL;
    if (constraints && !NSProcessInfo.processInfo.environment[@"PEAR_LEGACY_CONTAINER_LAYOUT"].boolValue) {
        method_setImplementation(constraints, imp_implementationWithBlock(^(id self) {
            PearLog(@"[PearRuntime][window] skipped scene container constraints");
        }));
    }
    Class nsView = NSClassFromString(@"NSView");
    Method update = nsView ? class_getInstanceMethod(nsView, NSSelectorFromString(@"updateConstraintsForSubtreeIfNeeded")) : NULL;
    if (update) {
        SEL updateSEL = NSSelectorFromString(@"updateConstraintsForSubtreeIfNeeded");
        IMP original = method_getImplementation(update);
        method_setImplementation(update, imp_implementationWithBlock(^(id self) {
            id window = [self respondsToSelector:@selector(window)]
                ? ((id (*)(id, SEL))objc_msgSend)(self, @selector(window)) : nil;
            NSString *windowClass = window ? NSStringFromClass([window class]) : @"";
            // AppKit permission alerts and their glass host need normal layout;
            // suppressing every NSView made the entire modal appear blank.
            BOOL alertView = [windowClass containsString:@"NSAlertPanel"] ||
                             [windowClass containsString:@"NSGlassEffectWindow"];
            if (alertView) {
                ((void (*)(id, SEL))original)(self, updateSEL);
            } else if (getenv("PEAR_DEBUG_VIEWS")) {
                PearLog(@"[PearRuntime][window] skipped legacy subtree constraints on %@", NSStringFromClass([self class]));
            }
        }));
    }
    gLegacyWindowFixInstalled = YES;
    PearLog(@"[PearRuntime][window] legacy window fix installed (constraints=%d, subtree=%d)", constraints != NULL, update != NULL);
}

// The pre-proxy workaround can leave the host NSWindow at 0x0 even when the
// UIKit window and renderer are alive. Repair only empty UINSWindows, never a
// window the user or app has already sized.
static void PearRepairEmptyLegacyWindows(void) {
    Class windowClass = NSClassFromString(@"UINSWindow");
    Class appClass = NSClassFromString(@"NSApplication");
    id app = [appClass respondsToSelector:@selector(sharedApplication)]
        ? ((id (*)(id, SEL))objc_msgSend)(appClass, @selector(sharedApplication)) : nil;
    if (!windowClass || !app) return;
    UIWindow *uiWindow = PearPrimaryWindow();
    CGSize preferred = uiWindow.bounds.size;
    if (preferred.width < 100 || preferred.height < 100) preferred = CGSizeMake(1024, 768);
    for (id window in [app windows]) {
        if (![window isKindOfClass:windowClass] || ![window respondsToSelector:@selector(frame)]) continue;
        NSRect frame = ((NSRect (*)(id, SEL))objc_msgSend)(window, @selector(frame));
        BOOL empty = frame.size.width <= 1 || frame.size.height <= 1;
        if (empty && [window respondsToSelector:@selector(setContentSize:)]) {
            ((void (*)(id, SEL, NSSize))objc_msgSend)(window, @selector(setContentSize:),
                NSMakeSize(preferred.width, preferred.height));
        }
        NSRect after = ((NSRect (*)(id, SEL))objc_msgSend)(window, @selector(frame));
        if (empty && (after.size.width <= 1 || after.size.height <= 1) &&
            [window respondsToSelector:@selector(setFrame:display:)]) {
            after.size = NSMakeSize(preferred.width, preferred.height);
            ((void (*)(id, SEL, NSRect, BOOL))objc_msgSend)(window, @selector(setFrame:display:), after, YES);
        }
        if (empty) PearLog(@"[PearRuntime][window] repaired empty legacy host window to %.0fx%.0f",
                           preferred.width, preferred.height);
        if (getenv("PEAR_DEBUG_VIEWS")) {
            static int buttonLogs = 0;
            if (buttonLogs++ % 300 == 0) {
                SEL buttonSel = NSSelectorFromString(@"standardWindowButton:");
                for (long kind = 0; kind < 3 && [window respondsToSelector:buttonSel]; kind++) {
                    id button = ((id (*)(id, SEL, long))objc_msgSend)(window, buttonSel, kind);
                    if (!button) continue;
                    NSRect frame = ((NSRect (*)(id, SEL))objc_msgSend)(button, @selector(frame));
                    BOOL enabled = ((BOOL (*)(id, SEL))objc_msgSend)(button, NSSelectorFromString(@"isEnabled"));
                    id superview = ((id (*)(id, SEL))objc_msgSend)(button, @selector(superview));
                    NSRect superFrame = superview ? ((NSRect (*)(id, SEL))objc_msgSend)(superview, @selector(frame)) : NSZeroRect;
                    PearLog(@"[PearRuntime][window] button %ld enabled=%d frame=%@ in %@ %@", kind, enabled,
                            NSStringFromRect(frame), NSStringFromClass([superview class]), NSStringFromRect(superFrame));
                }
            }
        }
        // Diagnostic experiment only: some scene-less apps have a UIKit layer
        // tree but an empty AppKit backing layer. Do not enable without testing.
        if (NSProcessInfo.processInfo.environment[@"PEAR_LEGACY_ATTACH_LAYER"].boolValue && uiWindow) {
            id view = [window respondsToSelector:@selector(contentView)]
                ? ((id (*)(id, SEL))objc_msgSend)(window, @selector(contentView)) : nil;
            if ([view respondsToSelector:@selector(setWantsLayer:)])
                ((void (*)(id, SEL, BOOL))objc_msgSend)(view, @selector(setWantsLayer:), YES);
            // A blank native scene container may advertise that its background
            // moves the window. Once it carries game content, drags belong to
            // that content; the native title bar retains its normal behavior.
            if ([window respondsToSelector:@selector(setMovableByWindowBackground:)])
                ((void (*)(id, SEL, BOOL))objc_msgSend)(window, @selector(setMovableByWindowBackground:), NO);
            if (view) {
                static NSMutableSet<NSValue *> *dragClasses;
                if (!dragClasses) dragClasses = [NSMutableSet set];
                Class viewClass = [view class];
                NSValue *classKey = [NSValue valueWithPointer:(__bridge const void *)viewClass];
                if (![dragClasses containsObject:classKey]) {
                    SEL dragSelector = NSSelectorFromString(@"mouseDownCanMoveWindow");
                    Method dragMethod = class_getInstanceMethod(viewClass, dragSelector);
                    if (dragMethod) {
                        IMP noDrag = imp_implementationWithBlock(^BOOL(__unused id receiver) { return NO; });
                        if (!class_addMethod(viewClass, dragSelector, noDrag, method_getTypeEncoding(dragMethod)))
                            method_setImplementation(dragMethod, noDrag);
                        [dragClasses addObject:classKey];
                    }
                }
            }
            CALayer *host = [view respondsToSelector:@selector(layer)]
                ? ((id (*)(id, SEL))objc_msgSend)(view, @selector(layer)) : nil;
            CALayer *root = uiWindow.layer;
            if (!host || !root) continue;
            CALayer *top = objc_getAssociatedObject(host, &kPearAttachedLegacyRoot);
            if (!top) {
                top = root;
                if (!NSProcessInfo.processInfo.environment[@"PEAR_LEGACY_WINDOW_LAYER_ONLY"].boolValue)
                    while (top.superlayer) top = top.superlayer;
                if (top == host || host.sublayers.count != 0) continue;
                [host addSublayer:top];
                objc_setAssociatedObject(host, &kPearAttachedLegacyRoot, top, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
                PearLog(@"[PearRuntime][window] attached UIKit root layer %@ to legacy host", NSStringFromClass(top.class));
            }
            // Changing frame changes bounds and crops the engine's layer tree.
            // Keep a logical (UIKit) size and scale/center the tree instead.
            //
            // The logical size must not be read back from the UIWindow after
            // the transform is applied: UIKit keeps the window's frame and
            // reports bounds = frame / scale, so every pass grew (or shrank)
            // the size again. Engines size their render target from those
            // bounds (Unity: UnityView -> CAMetalLayer drawable), which made
            // the internal resolution depend on timing; Gutterball ended up at
            // 557x418 in a large window. The logical size is now the host
            // view's size: one UIKit point per window point, so the content
            // scale factor (the display's backing scale) maps it onto the
            // window's real pixels, stays sharp at any size and follows
            // resizes. It is applied through UIKit (window and root view) so
            // autoresizing lays the views out again.
            //
            // The aspect ratio is the one the game launched with (its first
            // window), not the current window's: legacy titles lay out their
            // UI once for that shape, and a 16:9 full-screen surface left a
            // 4:3 game's HUD in one part of the frame with a black strip
            // beside it. The logical size is that aspect fitted into the
            // window, so the game keeps its shape at full resolution and the
            // rest is letterboxed.
            static CGFloat designAspect = 0;
            if (designAspect <= 0 && host.bounds.size.width >= 1 && host.bounds.size.height >= 1) {
                CGSize initial = CATransform3DIsIdentity(top.transform) && uiWindow.bounds.size.height >= 1
                    ? uiWindow.bounds.size : host.bounds.size;
                designAspect = initial.width / initial.height;
                PearLog(@"[PearRuntime][window] legacy design aspect %.4f (%.0fx%.0f)",
                        designAspect, initial.width, initial.height);
            }
            // Fit into the visible area below the title bar, not the whole
            // full-size content view (the top of the game was hidden).
            CGRect area = PearWindowVisibleContentRect(window, view);
            if (area.size.width < 1 || area.size.height < 1) area = host.bounds;
            CGRect designFit = PearAspectFitRect(CGSizeMake(designAspect * 1000.0, 1000.0), area);
            CGSize logical = CGSizeMake(round(designFit.size.width), round(designFit.size.height));
            if (logical.width < 1 || logical.height < 1) continue;
            NSString *sizeOverride = NSProcessInfo.processInfo.environment[@"PEAR_LEGACY_LOGICAL_SIZE"];
            NSArray<NSString *> *parts = [sizeOverride.lowercaseString componentsSeparatedByString:@"x"];
            if (parts.count == 2) {
                double width = parts[0].doubleValue, height = parts[1].doubleValue;
                if (isfinite(width) && isfinite(height) && width >= 100 && height >= 100 && width <= 8192 && height <= 8192) {
                    logical = CGSizeMake(width, height);
                    [CATransaction begin];
                    [CATransaction setDisableActions:YES];
                    if (!CGSizeEqualToSize(uiWindow.bounds.size, logical)) uiWindow.bounds = CGRectMake(0, 0, width, height);
                    UIView *content = uiWindow.rootViewController.isViewLoaded ? uiWindow.rootViewController.view : nil;
                    if (content && !CGSizeEqualToSize(content.bounds.size, logical)) content.frame = CGRectMake(0, 0, width, height);
                    [CATransaction commit];
                }
            }
            if (parts.count != 2) {
                [CATransaction begin];
                [CATransaction setDisableActions:YES];
                if (!CGSizeEqualToSize(uiWindow.bounds.size, logical)) {
                    uiWindow.bounds = CGRectMake(0, 0, logical.width, logical.height);
                }
                UIView *content = uiWindow.rootViewController.isViewLoaded ? uiWindow.rootViewController.view : nil;
                if (content && !CGSizeEqualToSize(content.bounds.size, logical)) {
                    content.frame = CGRectMake(0, 0, logical.width, logical.height);
                }
                [CATransaction commit];
            }
            CGRect fitted = PearAspectFitRect(logical, area);
            if (!CGRectIsEmpty(fitted)) {
                CGFloat scale = fitted.size.width / logical.width;
                CGPoint center = CGPointMake(CGRectGetMidX(fitted), CGRectGetMidY(fitted));
                if (CGSizeEqualToSize(top.bounds.size, logical) &&
                    CGPointEqualToPoint(top.position, center) &&
                    fabs(top.transform.m11 - scale) < 0.0001 && fabs(top.transform.m22 - scale) < 0.0001) continue;
                [CATransaction begin];
                [CATransaction setDisableActions:YES];
                // The detached top layer can already have host-sized bounds
                // while its UIKit descendants still use the larger window space.
                // Restore that full logical space before fitting it visually.
                top.bounds = CGRectMake(0, 0, logical.width, logical.height);
                top.position = CGPointMake(CGRectGetMidX(fitted), CGRectGetMidY(fitted));
                top.transform = CATransform3DMakeScale(scale, scale, 1);
                [CATransaction commit];
                if (getenv("PEAR_DEBUG_VIEWS")) PearLog(@"[PearRuntime][window] legacy fit bounds=%@ host=%@ scale=%.4f", NSStringFromCGRect(top.bounds), NSStringFromCGRect(host.bounds), scale);
            }
        }
    }
}

void PearInstallLegacyWindowFix(void) {
    if (!PearIsLegacyApp()) return;
    PearLog(@"[PearRuntime][window] legacy window fix requested");
    // Try before UIApplicationMain can create a window; if the class is not
    // loaded yet, retry on the main queue before later scene callbacks.
    if (NSThread.isMainThread) PearInstallLegacyWindowFixNow();
    dispatch_async(dispatch_get_main_queue(), ^{ PearInstallLegacyWindowFixNow(); });
    [NSNotificationCenter.defaultCenter addObserverForName:@"NSWindowDidBecomeKeyNotification"
        object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) {
            PearRepairEmptyLegacyWindows();
        }];
    [NSNotificationCenter.defaultCenter addObserverForName:@"NSWindowDidResizeNotification"
        object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) {
            PearRepairEmptyLegacyWindows();
        }];
    // UIKit can update its logical surface after the native resize callback.
    // Follow that settled geometry throughout live resizing and full-screen
    // transitions instead of stopping after the startup retry window.
    if (NSProcessInfo.processInfo.environment[@"PEAR_LEGACY_ATTACH_LAYER"].boolValue) {
        static dispatch_source_t geometryTimer;
        if (!geometryTimer) {
            geometryTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
            dispatch_source_set_timer(geometryTimer, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), NSEC_PER_SEC / 30, NSEC_PER_SEC / 300);
            dispatch_source_set_event_handler(geometryTimer, ^{ PearRepairEmptyLegacyWindows(); });
            dispatch_resume(geometryTimer);
        }
    }
    for (NSNumber *delay in @[@1.0, @3.0, @8.0, @15.0, @30.0]) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay.doubleValue * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{ PearRepairEmptyLegacyWindows(); });
    }
    for (NSNumber *delay in @[@0.05, @0.5, @2.0]) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay.doubleValue * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{ PearInstallLegacyWindowFixNow(); });
    }
}
