// WindowChromeShim.mm
//
// Keeps an iOS game's content out from under the Mac title bar.
//
// Catalyst hosts the UIKit scene in an AppKit window whose content view uses
// the full window frame (NSWindowStyleMaskFullSizeContentView), so the top
// ~28 points of the scene are drawn behind the title bar. iOS games place
// HUD elements, banners and buttons in that strip (GTA's minimap and mission
// text, store/trial banners), which then appear cut off and cannot be clicked.
//
// The scene view is placed in the window's content layout rect (below the
// title bar); UIKit then reports a window that is exactly the visible area,
// and UIScreen (ScreenShim) follows it. Changing the window style instead
// confused Catalyst (scene shrunk but stayed under the title bar).
// Full screen has no title bar and gets the whole window. Disable with PEAR_KEEP_TITLEBAR_OVERLAP=1.
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import "PearRuntime.h"
#include <signal.h>
#include <unistd.h>

static const NSUInteger kFullScreenMask = 1 << 14;      // NSWindowStyleMaskFullScreen
static const NSUInteger kFullSizeContentMask = 1 << 15; // NSWindowStyleMaskFullSizeContentView

static BOOL PearIsSceneWindow(id window) {
    for (Class cls = [window class]; cls; cls = class_getSuperclass(cls)) {
        if (strcmp(class_getName(cls), "UINSWindow") == 0) return YES;
    }
    return NO;
}

static Class PearClass(const char *name) { return objc_getClass(name); }

/// Finds the AppKit view that hosts the UIKit scene (UINSSceneView).
static id PearSceneView(id contentView) {
    Class sceneClass = PearClass("UINSSceneView");
    if (!contentView || !sceneClass) return nil;
    if ([contentView isKindOfClass:sceneClass]) return contentView;
    for (id child in ((NSArray *(*)(id, SEL))objc_msgSend)(contentView, @selector(subviews))) {
        if ([child isKindOfClass:sceneClass]) return child;
    }
    return nil;
}

/// The frame the scene view should have inside `window`: the content layout
/// rect (area below the title bar), in content-view coordinates. Returns NO
/// when the window has no overlapping title bar (full screen, or a style
/// without full-size content).
static BOOL PearSceneTargetFrame(id window, id contentView, CGRect *target) {
    if (!window || !PearIsSceneWindow(window)) return NO;
    NSUInteger mask = ((NSUInteger (*)(id, SEL))objc_msgSend)(window, @selector(styleMask));
    if ((mask & kFullScreenMask) || !(mask & kFullSizeContentMask)) return NO;
    SEL layoutRect = NSSelectorFromString(@"contentLayoutRect");
    if (!contentView || ![window respondsToSelector:layoutRect]) return NO;
    CGRect visible = ((CGRect (*)(id, SEL))objc_msgSend)(window, layoutRect);
    CGRect container = ((CGRect (*)(id, SEL))objc_msgSend)(contentView, @selector(bounds));
    if (visible.size.width < 1 || visible.size.height < 1) return NO;
    if (visible.size.height >= container.size.height - 0.5) return NO;
    // contentLayoutRect is in window coordinates (bottom-left origin); the
    // content view spans the window, so only a flipped view needs converting.
    BOOL flipped = ((BOOL (*)(id, SEL))objc_msgSend)(contentView, @selector(isFlipped));
    *target = visible;
    if (flipped) target->origin.y = container.size.height - CGRectGetMaxY(visible);
    return YES;
}

static IMP gOriginalSceneSetFrame;

/// Catalyst lays the scene view out over the whole window on every layout
/// pass, so the frame is clamped where it is set rather than fixed once.
static void PearInstallSceneFrameClamp(void) {
    static BOOL installed = NO;
    if (installed) return;
    Class sceneClass = PearClass("UINSSceneView");
    if (!sceneClass) return;
    Method method = class_getInstanceMethod(sceneClass, @selector(setFrame:));
    if (!method) return;
    installed = YES;
    gOriginalSceneSetFrame = method_getImplementation(method);
    IMP replacement = imp_implementationWithBlock(^(id self, CGRect frame) {
        id window = [self respondsToSelector:@selector(window)]
            ? ((id (*)(id, SEL))objc_msgSend)(self, @selector(window)) : nil;
        id superview = ((id (*)(id, SEL))objc_msgSend)(self, @selector(superview));
        CGRect target;
        if (PearSceneTargetFrame(window, superview, &target)) frame = target;
        ((void (*)(id, SEL, CGRect))gOriginalSceneSetFrame)(self, @selector(setFrame:), frame);
    });
    // Add on the class itself so a superclass implementation is not replaced
    // for unrelated views.
    if (!class_addMethod(sceneClass, @selector(setFrame:), replacement, method_getTypeEncoding(method))) {
        method_setImplementation(method, replacement);
    }
    PearLog(@"[PearRuntime][window] scene view kept below the title bar");
}

static void PearFixTitlebarOverlap(id window) {
    if (!window || !PearIsSceneWindow(window)) return;
    PearInstallSceneFrameClamp();
    id contentView = ((id (*)(id, SEL))objc_msgSend)(window, @selector(contentView));
    id sceneView = PearSceneView(contentView);
    CGRect target;
    if (!sceneView || !PearSceneTargetFrame(window, contentView, &target)) {
        // Full screen: give the scene the whole window back.
        if (sceneView && contentView) {
            CGRect bounds = ((CGRect (*)(id, SEL))objc_msgSend)(contentView, @selector(bounds));
            CGRect current = ((CGRect (*)(id, SEL))objc_msgSend)(sceneView, @selector(frame));
            if (!CGRectEqualToRect(current, bounds)) {
                ((void (*)(id, SEL, CGRect))objc_msgSend)(sceneView, @selector(setFrame:), bounds);
            }
        }
        return;
    }
    CGRect current = ((CGRect (*)(id, SEL))objc_msgSend)(sceneView, @selector(frame));
    if (CGRectEqualToRect(current, target)) return;
    ((void (*)(id, SEL, CGRect))objc_msgSend)(sceneView, @selector(setFrame:), target);
    if (getenv("PEAR_DEBUG_VIEWS")) {
        PearLog(@"[PearRuntime][window] scene placed below the title bar: %@",
                NSStringFromCGRect(target));
    }
}

static void PearFixAllWindows(void) {
    Class appClass = NSClassFromString(@"NSApplication");
    id app = [appClass respondsToSelector:@selector(sharedApplication)]
        ? ((id (*)(id, SEL))objc_msgSend)(appClass, @selector(sharedApplication)) : nil;
    if (![app respondsToSelector:@selector(windows)]) return;
    for (id window in ((NSArray *(*)(id, SEL))objc_msgSend)(app, @selector(windows))) {
        PearFixTitlebarOverlap(window);
    }
}

__attribute__((constructor))
static void PearWindowChromeShimInit(void) {
    PEAR_MODULE_GUARD();
    // Closing the last game scene closes the runtime process as requested by
    // the launcher. Ignore auxiliary windows and keep other game scenes alive.
    [NSNotificationCenter.defaultCenter addObserverForName:@"NSWindowWillCloseNotification" object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            if (!PearIsSceneWindow(note.object)) return;
            __weak id closing = note.object;
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC / 2), dispatch_get_main_queue(), ^{
                Class cls = NSClassFromString(@"NSApplication");
                id app = ((id (*)(id, SEL))objc_msgSend)(cls, @selector(sharedApplication));
                for (id window in ((NSArray *(*)(id, SEL))objc_msgSend)(app, @selector(windows))) {
                    if (window == closing || !PearIsSceneWindow(window)) continue;
                    BOOL visible = ((BOOL (*)(id, SEL))objc_msgSend)(window, @selector(isVisible));
                    BOOL minimized = ((BOOL (*)(id, SEL))objc_msgSend)(window, @selector(isMiniaturized));
                    if (visible || minimized) return;
                }
                [NSNotificationCenter.defaultCenter postNotificationName:@"NSApplicationWillTerminateNotification" object:app];
                PearLog(@"[PearRuntime][window] last game window closed; terminating owned process");
                kill(getpid(), SIGKILL);
            });
        }];
    if (getenv("PEAR_KEEP_TITLEBAR_OVERLAP")) return;
    // Resizable UIKit applications already lay out navigation/safe areas in
    // their native Catalyst host. Applying the game's chrome clamp as well
    // shifts that content twice (for example YouTube).
    id fullScreenRequired = NSBundle.mainBundle.infoDictionary[@"UIRequiresFullScreen"];
    if ([fullScreenRequired isKindOfClass:NSNumber.class] && ![fullScreenRequired boolValue] &&
        !getenv("PEAR_LEGACY_ATTACH_LAYER")) return;
    PearInstallSceneFrameClamp();
    // Catalyst can restore the style when the scene updates its titlebar or
    // the window leaves full screen, so re-apply on those events.
    for (NSString *name in @[ @"NSWindowDidBecomeKeyNotification",
                              @"NSWindowDidResizeNotification",
                              @"NSWindowDidEndLiveResizeNotification",
                              @"NSWindowDidEnterFullScreenNotification",
                              @"NSWindowDidExitFullScreenNotification" ]) {
        [NSNotificationCenter.defaultCenter addObserverForName:name object:nil
            queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
                PearFixTitlebarOverlap(note.object);
                // Catalyst may lay the scene out again after the resize.
                dispatch_async(dispatch_get_main_queue(), ^{ PearFixTitlebarOverlap(note.object); });
            }];
    }
    for (NSNumber *delay in @[ @0.2, @1.0, @3.0 ]) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay.doubleValue * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{ PearFixAllWindows(); });
    }
}
