// macOS backend for PearHost UIKit: native windows, screen metrics, display
// link, run loop and pointer input on AppKit. Written from the public AppKit
// and QuartzCore interfaces.
#import <AppKit/AppKit.h>
#import <IOKit/pwr_mgt/IOPMLib.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import "../../src/PearUIKitPrivate.h"

@interface PearHostView : NSView
@property (nonatomic, weak) UIWindow *uiWindow;
@end

@interface PearHostWindowDelegate : NSObject <NSWindowDelegate>
@property (nonatomic, weak) UIWindow *uiWindow;
@end

static NSMutableArray *gNativeWindows;  // keeps NSWindows alive
static PearHostView *gPrimaryView;

static CGSize defaultPointSize(void) {
    const char *env = getenv("PEAR_HOST_SCREEN");  // e.g. "1280x720"
    int w = 0, h = 0;
    if (env && sscanf(env, "%dx%d", &w, &h) == 2 && w > 0 && h > 0) return CGSizeMake(w, h);
    NSRect vis = NSScreen.mainScreen.visibleFrame;
    CGFloat width = floor(vis.size.width * 0.8);
    CGFloat height = floor(width * 9.0 / 16.0);
    if (height > vis.size.height * 0.85) {
        height = floor(vis.size.height * 0.85);
        width = floor(height * 16.0 / 9.0);
    }
    return CGSizeMake(width, height);
}

static PearHostScreenInfo gScreen;

void PearBackendInitialize(void) {
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    gNativeWindows = [NSMutableArray array];
    gScreen.pointSize = defaultPointSize();
    gScreen.scale = NSScreen.mainScreen.backingScaleFactor ?: 2.0;
    gScreen.maxFPS = NSScreen.mainScreen.maximumFramesPerSecond ?: 60;
    PearBackendInstallQuartzCoreCompat();

    NSNotificationCenter *nc = NSNotificationCenter.defaultCenter;
    [nc addObserverForName:NSApplicationDidBecomeActiveNotification object:nil queue:nil
                usingBlock:^(NSNotification *n) { PearUIKitApplicationDidBecomeActive(); }];
    [nc addObserverForName:NSApplicationWillResignActiveNotification object:nil queue:nil
                usingBlock:^(NSNotification *n) { PearUIKitApplicationWillResignActive(); }];
    [nc addObserverForName:NSApplicationDidHideNotification object:nil queue:nil
                usingBlock:^(NSNotification *n) { PearUIKitApplicationDidEnterBackground(); }];
    [nc addObserverForName:NSApplicationWillUnhideNotification object:nil queue:nil
                usingBlock:^(NSNotification *n) { PearUIKitApplicationWillEnterForeground(); }];
    [nc addObserverForName:NSApplicationWillTerminateNotification object:nil queue:nil
                usingBlock:^(NSNotification *n) { PearUIKitApplicationWillTerminate(); }];
    PearHostLog("host", "AppKit backend: screen %.0fx%.0f pt @%.1fx, %ld Hz", gScreen.pointSize.width,
                gScreen.pointSize.height, gScreen.scale, (long)gScreen.maxFPS);
}

PearHostScreenInfo PearBackendScreenInfo(void) { return gScreen; }

void *PearBackendCreateWindow(UIWindow *window, CGRect frame) {
    NSRect content = NSMakeRect(0, 0, frame.size.width, frame.size.height);
    NSWindow *w = [[NSWindow alloc] initWithContentRect:content
                                              styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                                                        NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
                                                backing:NSBackingStoreBuffered
                                                  defer:NO];
    w.title = NSBundle.mainBundle.infoDictionary[@"CFBundleDisplayName"] ?: NSProcessInfo.processInfo.processName;
    w.releasedWhenClosed = NO;
    w.acceptsMouseMovedEvents = YES;
    w.collectionBehavior |= NSWindowCollectionBehaviorFullScreenPrimary;
    [w setContentAspectRatio:content.size];
    PearHostView *v = [[PearHostView alloc] initWithFrame:content];
    v.uiWindow = window;
    w.contentView = v;
    PearHostWindowDelegate *d = [PearHostWindowDelegate new];
    d.uiWindow = window;
    w.delegate = d;
    objc_setAssociatedObject(w, "pear.delegate", d, OBJC_ASSOCIATION_RETAIN);
    [w center];
    [gNativeWindows addObject:w];
    if (!gPrimaryView) gPrimaryView = v;
    return (__bridge void *)w;
}

void PearBackendSetRootLayer(void *handle, CALayer *layer) {
    NSWindow *w = (__bridge NSWindow *)handle;
    NSView *v = w.contentView;
    // Layer-hosting view: the UIWindow's layer tree is the view's content.
    v.layer = layer;
    v.wantsLayer = YES;
    layer.contentsScale = w.backingScaleFactor;
}

void PearBackendShowWindow(void *handle, BOOL key) {
    NSWindow *w = (__bridge NSWindow *)handle;
    if (key) [w makeKeyAndOrderFront:nil];
    else [w orderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

CADisplayLink *PearBackendDisplayLink(id target, SEL selector) {
    if (@available(macOS 14.0, *)) {
        if (gPrimaryView) return [gPrimaryView displayLinkWithTarget:target selector:selector];
        return [NSScreen.mainScreen displayLinkWithTarget:target selector:selector];
    }
    return nil;
}

// iOS code creates display links with +[CADisplayLink displayLinkWithTarget:selector:],
// which macOS QuartzCore does not offer; macOS hands them out from NSView/NSScreen.
static id pearDisplayLinkClassFactory(id self, SEL _cmd, id target, SEL selector) {
    return PearBackendDisplayLink(target, selector);
}

// iOS CADisplayLink API that macOS's display link lacks. Documented semantics:
// frameInterval N = fire every Nth refresh; preferredFramesPerSecond = target
// rate (0 = native). Both map onto preferredFrameRateRange.
static const void *kFrameIntervalKey = &kFrameIntervalKey;
static const void *kPreferredFPSKey = &kPreferredFPSKey;

static void applyRate(CADisplayLink *link, double fps) API_AVAILABLE(macos(14.0)) {
    double maxFPS = (double)PearBackendScreenInfo().maxFPS;
    if (fps <= 0 || fps > maxFPS) fps = maxFPS;
    link.preferredFrameRateRange = CAFrameRateRangeMake((float)fps, (float)fps, (float)fps);
}
static NSInteger pearFrameInterval(id self, SEL _cmd) {
    NSNumber *n = objc_getAssociatedObject(self, kFrameIntervalKey);
    return n ? n.integerValue : 1;
}
static void pearSetFrameInterval(id self, SEL _cmd, NSInteger interval) {
    if (interval < 1) interval = 1;
    objc_setAssociatedObject(self, kFrameIntervalKey, @(interval), OBJC_ASSOCIATION_RETAIN);
    if (@available(macOS 14.0, *)) applyRate(self, (double)PearBackendScreenInfo().maxFPS / (double)interval);
}
static NSInteger pearPreferredFPS(id self, SEL _cmd) {
    return [objc_getAssociatedObject(self, kPreferredFPSKey) integerValue];
}
static void pearSetPreferredFPS(id self, SEL _cmd, NSInteger fps) {
    objc_setAssociatedObject(self, kPreferredFPSKey, @(fps), OBJC_ASSOCIATION_RETAIN);
    if (@available(macOS 14.0, *)) applyRate(self, (double)fps);
}

static void addIfMissing(Class cls, SEL sel, IMP imp, const char *types) {
    if (![cls instancesRespondToSelector:sel]) {
        class_addMethod(cls, sel, imp, types);
        PearHostLog("host", "-[%s %s] provided by the host", class_getName(cls), sel_getName(sel));
    }
}

void PearBackendInstallQuartzCoreCompat(void) {
    Class cls = objc_getClass("CADisplayLink");
    if (!cls) {
        PearMissingReport(PearMissingBehavior, "QuartzCore", "CADisplayLink class absent (macOS < 14)", NULL);
        return;
    }
    SEL sel = @selector(displayLinkWithTarget:selector:);
    if (![cls respondsToSelector:sel]) {
        class_addMethod(object_getClass(cls), sel, (IMP)pearDisplayLinkClassFactory, "@@:@:");
        PearHostLog("host", "+[CADisplayLink displayLinkWithTarget:selector:] provided by the host");
    }
    // macOS's concrete display link class inherits these selectors but raises
    // when they are sent (observed: -[_NSDisplayLink setFrameInterval:]).
    // Probe a real instance; replace each selector that raises or is missing.
    if (@available(macOS 14.0, *)) {
        CADisplayLink *probe = [NSScreen.mainScreen displayLinkWithTarget:NSNull.null selector:@selector(class)];
        Class concrete = probe ? object_getClass(probe) : cls;
        struct { SEL sel; IMP imp; const char *types; BOOL setter; } table[] = {
            {@selector(frameInterval), (IMP)pearFrameInterval, "q@:", NO},
            {@selector(setFrameInterval:), (IMP)pearSetFrameInterval, "v@:q", YES},
            {@selector(preferredFramesPerSecond), (IMP)pearPreferredFPS, "q@:", NO},
            {@selector(setPreferredFramesPerSecond:), (IMP)pearSetPreferredFPS, "v@:q", YES},
        };
        for (size_t i = 0; i < sizeof table / sizeof table[0]; i++) {
            BOOL works = NO;
            if (probe && [probe respondsToSelector:table[i].sel]) {
                @try {
                    if (table[i].setter) ((void (*)(id, SEL, NSInteger))objc_msgSend)(probe, table[i].sel, 1);
                    else ((NSInteger(*)(id, SEL))objc_msgSend)(probe, table[i].sel);
                    works = YES;
                } @catch (NSException *e) {
                }
            }
            if (!works) {
                class_replaceMethod(concrete, table[i].sel, table[i].imp, table[i].types);
                PearHostLog("host", "-[%s %s] provided by the host", class_getName(concrete), sel_getName(table[i].sel));
            }
        }
        [probe invalidate];
    }
}

BOOL PearBackendOpenURL(NSURL *url) { return [NSWorkspace.sharedWorkspace openURL:url]; }

static IOPMAssertionID gIdleAssertion;
void PearBackendSetIdleTimerDisabled(BOOL disabled) {
    if (disabled && !gIdleAssertion) {
        IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep, kIOPMAssertionLevelOn,
                                    CFSTR("iOS app disabled the idle timer"), &gIdleAssertion);
    } else if (!disabled && gIdleAssertion) {
        IOPMAssertionRelease(gIdleAssertion);
        gIdleAssertion = 0;
    }
}

@interface PearHostAppDelegate : NSObject <NSApplicationDelegate>
@property (nonatomic, copy) void (^didFinish)(void);
@end
@implementation PearHostAppDelegate
- (void)applicationDidFinishLaunching:(NSNotification *)n {
    if (self.didFinish) self.didFinish();
    self.didFinish = nil;
}
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)app { return YES; }
@end

void PearBackendRun(void (^didFinishLaunching)(void)) {
    static PearHostAppDelegate *delegate;
    delegate = [PearHostAppDelegate new];
    delegate.didFinish = didFinishLaunching;
    NSApp.delegate = delegate;

    NSMenu *bar = [NSMenu new];
    NSMenuItem *appItem = [NSMenuItem new];
    [bar addItem:appItem];
    NSMenu *appMenu = [NSMenu new];
    [appMenu addItemWithTitle:@"Quit" action:@selector(terminate:) keyEquivalent:@"q"];
    appItem.submenu = appMenu;
    NSApp.mainMenu = bar;

    [NSApp run];
}

// ---- input -------------------------------------------------------------------

@implementation PearHostView
- (BOOL)isFlipped { return YES; }  // UIKit coordinates: origin top-left
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
- (BOOL)wantsUpdateLayer { return YES; }

- (void)pear_pointer:(NSEvent *)e phase:(UITouchPhase)phase {
    NSPoint p = [self convertPoint:e.locationInWindow fromView:nil];
    UIWindow *w = self.uiWindow;
    if (!w) return;
    // Window content may be scaled relative to the UIWindow's logical bounds.
    CGRect b = w.bounds;
    CGPoint ui = CGPointMake(p.x * b.size.width / MAX(self.bounds.size.width, 1),
                             p.y * b.size.height / MAX(self.bounds.size.height, 1));
    PearUIKitDeliverPointer(w, phase, ui, e.timestamp);
}
- (void)mouseDown:(NSEvent *)e { [self pear_pointer:e phase:UITouchPhaseBegan]; }
- (void)mouseDragged:(NSEvent *)e { [self pear_pointer:e phase:UITouchPhaseMoved]; }
- (void)mouseUp:(NSEvent *)e { [self pear_pointer:e phase:UITouchPhaseEnded]; }
// Keyboard reaches games through GameController's GCKeyboard on macOS; swallow
// key events here so AppKit does not beep.
- (void)keyDown:(NSEvent *)e {}
- (void)keyUp:(NSEvent *)e {}
- (void)flagsChanged:(NSEvent *)e {}
@end

@implementation PearHostWindowDelegate
- (void)windowDidResize:(NSNotification *)n {
    NSWindow *w = n.object;
    UIWindow *ui = self.uiWindow;
    if (ui) PearUIKitWindowDidResize(ui, w.contentView.bounds.size, w.backingScaleFactor);
}
- (void)windowDidChangeBackingProperties:(NSNotification *)n { [self windowDidResize:n]; }
- (void)windowDidBecomeKey:(NSNotification *)n { PearUIKitSetKeyWindow(self.uiWindow); }
@end

void PearBackendPresentAlert(NSString *title, NSString *message, NSArray<NSString *> *buttons, void (^reply)(NSInteger)) {
    dispatch_async(dispatch_get_main_queue(), ^{
        NSAlert *alert = [NSAlert new];
        alert.messageText = title.length ? title : @" ";
        alert.informativeText = message;
        for (NSString *b in buttons) [alert addButtonWithTitle:b];
        if (!buttons.count) [alert addButtonWithTitle:@"OK"];
        void (^done)(NSModalResponse) = ^(NSModalResponse r) {
            NSInteger index = r - NSAlertFirstButtonReturn;
            reply(buttons.count ? index : -1);
        };
        NSWindow *host = NSApp.keyWindow ?: gNativeWindows.firstObject;
        if (host) [alert beginSheetModalForWindow:host completionHandler:done];
        else done([alert runModal]);
    });
}
