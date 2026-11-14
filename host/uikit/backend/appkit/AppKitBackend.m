// macOS backend for PearHost UIKit: native windows, screen metrics, display
// link, run loop and pointer input on AppKit. Written from the public AppKit
// and QuartzCore interfaces.
#import <AppKit/AppKit.h>
#import <IOKit/pwr_mgt/IOPMLib.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import "../../src/PearUIKitPrivate.h"

NSInteger PearHIDUsageForMacKeyCode(unsigned short code);

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
    [w makeFirstResponder:v];  // key events must reach the host view
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
// ---- keyboard ----
// Text: while a UIKit text responder is first responder, AppKit's key binding
// system turns key downs into text and edit commands for it.
// Keys: otherwise every key down/up becomes a UIPress with its HID usage
// (games such as Minecraft read keyboards through pressesBegan:withEvent:).
- (void)keyDown:(NSEvent *)e {
    if (PearUIKitTextInputActive()) {
        [self interpretKeyEvents:@[ e ]];
        return;
    }
    if (e.isARepeat) return;
    [self pear_key:e down:YES];
}
- (void)keyUp:(NSEvent *)e {
    if (PearUIKitTextInputActive()) return;
    [self pear_key:e down:NO];
}
- (void)flagsChanged:(NSEvent *)e {
    NSInteger usage = PearHIDUsageForMacKeyCode(e.keyCode);
    NSEventModifierFlags bit = 0;
    switch (usage) {
    case 0xE1: case 0xE5: bit = NSEventModifierFlagShift; break;
    case 0xE0: case 0xE4: bit = NSEventModifierFlagControl; break;
    case 0xE2: case 0xE6: bit = NSEventModifierFlagOption; break;
    case 0xE3: case 0xE7: bit = NSEventModifierFlagCommand; break;
    case 0x39: bit = NSEventModifierFlagCapsLock; break;
    default: return;
    }
    BOOL down = (e.modifierFlags & bit) != 0;
    PearUIKitDeliverKey(usage, @"", @"", (NSInteger)(e.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask), down,
                        e.timestamp);
}
- (void)pear_key:(NSEvent *)e down:(BOOL)down {
    NSInteger usage = PearHIDUsageForMacKeyCode(e.keyCode);
    if (!usage) return;
    // UIKeyModifierFlags use the same bit positions as NSEventModifierFlags.
    PearUIKitDeliverKey(usage, e.characters ?: @"", e.charactersIgnoringModifiers ?: @"",
                        (NSInteger)(e.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask), down, e.timestamp);
}

// NSResponder text entry points reached through -interpretKeyEvents:.
- (void)insertText:(id)text {
    NSString *s = [text isKindOfClass:[NSAttributedString class]] ? [text string] : text;
    if (s.length) PearUIKitDeliverText(s);
}
- (void)doCommandBySelector:(SEL)sel {
    if (getenv("PEAR_HOST_TRACE_INPUT") && *getenv("PEAR_HOST_TRACE_INPUT") == '1')
        PearHostLog("input", "AppKit command %s", sel_getName(sel));
    if (sel == @selector(deleteBackward:)) PearUIKitDeliverEditCommand(PearEditDeleteBackward);
    else if (sel == @selector(insertNewline:) || sel == @selector(insertLineBreak:)) PearUIKitDeliverEditCommand(PearEditNewline);
    else if (sel == @selector(moveLeft:)) PearUIKitDeliverEditCommand(PearEditMoveLeft);
    else if (sel == @selector(moveRight:)) PearUIKitDeliverEditCommand(PearEditMoveRight);
    else if (sel == @selector(cancelOperation:)) PearUIKitDeliverEditCommand(PearEditCancel);
    else if (sel == @selector(insertTab:)) PearUIKitDeliverText(@"\t");
}
@end

// macOS virtual key code (Carbon kVK_*, public HIToolbox/Events.h) -> USB HID
// keyboard usage (public HID Usage Tables, page 0x07), which is what
// UIKey.keyCode reports.
NSInteger PearHIDUsageForMacKeyCode(unsigned short code) {
    static const unsigned char map[128] = {
        [0x00] = 0x04, [0x0B] = 0x05, [0x08] = 0x06, [0x02] = 0x07, [0x0E] = 0x08, [0x03] = 0x09, [0x05] = 0x0A,
        [0x04] = 0x0B, [0x22] = 0x0C, [0x26] = 0x0D, [0x28] = 0x0E, [0x25] = 0x0F, [0x2E] = 0x10, [0x2D] = 0x11,
        [0x1F] = 0x12, [0x23] = 0x13, [0x0C] = 0x14, [0x0F] = 0x15, [0x01] = 0x16, [0x11] = 0x17, [0x20] = 0x18,
        [0x09] = 0x19, [0x0D] = 0x1A, [0x07] = 0x1B, [0x10] = 0x1C, [0x06] = 0x1D,                    // A-Z
        [0x12] = 0x1E, [0x13] = 0x1F, [0x14] = 0x20, [0x15] = 0x21, [0x17] = 0x22, [0x16] = 0x23, [0x1A] = 0x24,
        [0x1C] = 0x25, [0x19] = 0x26, [0x1D] = 0x27,                                                   // 1-0
        [0x24] = 0x28, [0x35] = 0x29, [0x33] = 0x2A, [0x30] = 0x2B, [0x31] = 0x2C, [0x1B] = 0x2D, [0x18] = 0x2E,
        [0x21] = 0x2F, [0x1E] = 0x30, [0x2A] = 0x31, [0x29] = 0x33, [0x27] = 0x34, [0x32] = 0x35, [0x2B] = 0x36,
        [0x2F] = 0x37, [0x2C] = 0x38, [0x39] = 0x39, [0x0A] = 0x64,
        [0x7A] = 0x3A, [0x78] = 0x3B, [0x63] = 0x3C, [0x76] = 0x3D, [0x60] = 0x3E, [0x61] = 0x3F, [0x62] = 0x40,
        [0x64] = 0x41, [0x65] = 0x42, [0x6D] = 0x43, [0x67] = 0x44, [0x6F] = 0x45,                    // F1-F12
        [0x73] = 0x4A, [0x74] = 0x4B, [0x75] = 0x4C, [0x77] = 0x4D, [0x79] = 0x4E,                    // home..pgdn
        [0x7C] = 0x4F, [0x7B] = 0x50, [0x7D] = 0x51, [0x7E] = 0x52,                                   // arrows
        [0x47] = 0x53, [0x4B] = 0x54, [0x43] = 0x55, [0x4E] = 0x56, [0x45] = 0x57, [0x4C] = 0x58, [0x53] = 0x59,
        [0x54] = 0x5A, [0x55] = 0x5B, [0x56] = 0x5C, [0x57] = 0x5D, [0x58] = 0x5E, [0x59] = 0x5F, [0x5B] = 0x60,
        [0x5C] = 0x61, [0x52] = 0x62, [0x41] = 0x63, [0x51] = 0x67,                                   // keypad
        [0x3B] = 0xE0, [0x38] = 0xE1, [0x3A] = 0xE2, [0x37] = 0xE3, [0x3E] = 0xE4, [0x3C] = 0xE5, [0x3D] = 0xE6,
        [0x36] = 0xE7,                                                                                 // modifiers
    };
    return code < 128 ? map[code] : 0;
}

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

// ---- scripted input (tests) ------------------------------------------------------
//
// PEAR_HOST_INPUT_SCRIPT=<file> drives the app through the same paths as real
// input, for automated runs. One command per line:
//   wait <seconds>           tap <x> <y>   (0..1 of the window, top-left origin)
//   key <name|char>          hold <name|char> <seconds>
//   type <text>              log <message>
//   call <target> <selector> [args...]   send a message (tests only); target is
//        delegate | rootvc | rootview | responder | <ClassName> (class method);
//        args: "string", integers, YES/NO, nil. Logs whether the target responds.
// Key names: return escape delete tab space up down left right, or one character.

static unsigned short macKeyCodeForName(NSString *name, NSString **chars) {
    NSDictionary *named = @{@"return" : @[ @0x24, @"\r" ], @"escape" : @[ @0x35, @"\x1b" ], @"delete" : @[ @0x33, @"\x7f" ],
                            @"tab" : @[ @0x30, @"\t" ], @"space" : @[ @0x31, @" " ],
                            // Arrow keys carry AppKit's function-key characters.
                            @"up" : @[ @0x7E, @"\uF700" ], @"down" : @[ @0x7D, @"\uF701" ],
                            @"left" : @[ @0x7B, @"\uF702" ], @"right" : @[ @0x7C, @"\uF703" ]};
    NSArray *e = named[name.lowercaseString];
    if (e) {
        *chars = e[1];
        return (unsigned short)[e[0] unsignedShortValue];
    }
    *chars = name;
    static NSString *letters = @"asdfhgzxcv_bqweryt123465=97-80]ou[ip_lj'k;\\,/nm.";
    NSString *lower = name.lowercaseString;
    NSRange r = lower.length == 1 ? [letters rangeOfString:lower] : NSMakeRange(NSNotFound, 0);
    return r.location == NSNotFound ? 0xFFFF : (unsigned short)r.location;
}

static void scriptKey(NSString *name, BOOL down) {
    NSString *chars = @"";
    unsigned short code = macKeyCodeForName(name, &chars);
    NSWindow *w = gPrimaryView.window;
    NSEvent *e = [NSEvent keyEventWithType:down ? NSEventTypeKeyDown : NSEventTypeKeyUp location:NSZeroPoint modifierFlags:0
                                 timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:w.windowNumber context:nil
                                characters:chars charactersIgnoringModifiers:chars isARepeat:NO
                                   keyCode:code == 0xFFFF ? 0 : code];
    if (down) [gPrimaryView keyDown:e];
    else [gPrimaryView keyUp:e];
}

static void scriptCall(NSArray<NSString *> *args) {
    NSString *t = args[0];
    UIWindow *w = PearUIKitKeyWindow();
    id target = [t isEqualToString:@"delegate"] ? UIApplication.sharedApplication.delegate
                : [t isEqualToString:@"rootvc"] ? w.rootViewController
                : [t isEqualToString:@"rootview"] ? w.rootViewController.view
                : [t isEqualToString:@"responder"] ? (id)PearUIKitFirstResponder()
                : (id)NSClassFromString(t);
    SEL sel = NSSelectorFromString(args[1]);
    if (![target respondsToSelector:sel]) {
        PearHostLog("script", "call: %s (%s) does not respond to %s", t.UTF8String,
                    target ? class_getName([target class]) : "nil", args[1].UTF8String);
        return;
    }
    NSMethodSignature *sig = [target methodSignatureForSelector:sel];
    NSInvocation *inv = [NSInvocation invocationWithMethodSignature:sig];
    inv.target = target;
    inv.selector = sel;
    // Re-join quoted strings that contain spaces.
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
    PearHostLog("script", "call: -[%s %s] sent", class_getName([target class]), args[1].UTF8String);
}

static void onMain(void (^b)(void)) { dispatch_sync(dispatch_get_main_queue(), b); }

void PearBackendStartInputScript(void) {
    const char *path = getenv("PEAR_HOST_INPUT_SCRIPT");
    if (!path || !*path) return;
    NSString *text = [NSString stringWithContentsOfFile:@(path) encoding:NSUTF8StringEncoding error:nil];
    if (!text) {
        PearHostLog("script", "cannot read %s", path);
        return;
    }
    NSArray *lines = [text componentsSeparatedByCharactersInSet:NSCharacterSet.newlineCharacterSet];
    [NSThread detachNewThreadWithBlock:^{
        for (NSString *raw in lines) {
            NSString *line = [raw stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
            if (!line.length || [line hasPrefix:@"#"]) continue;
            NSRange sp = [line rangeOfString:@" "];
            NSString *cmd = sp.location == NSNotFound ? line : [line substringToIndex:sp.location];
            NSString *arg = sp.location == NSNotFound ? @"" : [line substringFromIndex:sp.location + 1];
            NSArray *args = [arg componentsSeparatedByString:@" "];
            PearHostLog("script", "%s", line.UTF8String);
            if ([cmd isEqualToString:@"wait"]) {
                [NSThread sleepForTimeInterval:arg.doubleValue];
            } else if ([cmd isEqualToString:@"tap"] && args.count >= 2) {
                double x = [args[0] doubleValue], y = [args[1] doubleValue];
                onMain(^{
                    UIWindow *w = gPrimaryView.uiWindow;
                    CGPoint p = CGPointMake(x * w.bounds.size.width, y * w.bounds.size.height);
                    NSTimeInterval t = NSProcessInfo.processInfo.systemUptime;
                    PearUIKitDeliverPointer(w, UITouchPhaseBegan, p, t);
                });
                [NSThread sleepForTimeInterval:0.08];
                onMain(^{
                    UIWindow *w = gPrimaryView.uiWindow;
                    CGPoint p = CGPointMake(x * w.bounds.size.width, y * w.bounds.size.height);
                    PearUIKitDeliverPointer(w, UITouchPhaseEnded, p, NSProcessInfo.processInfo.systemUptime);
                });
            } else if ([cmd isEqualToString:@"key"] || [cmd isEqualToString:@"hold"]) {
                NSString *k = args[0];
                double secs = [cmd isEqualToString:@"hold"] && args.count > 1 ? [args[1] doubleValue] : 0.05;
                onMain(^{ scriptKey(k, YES); });
                [NSThread sleepForTimeInterval:secs];
                onMain(^{ scriptKey(k, NO); });
            } else if ([cmd isEqualToString:@"type"]) {
                for (NSUInteger i = 0; i < arg.length; i++) {
                    NSString *c = [arg substringWithRange:NSMakeRange(i, 1)];
                    onMain(^{ scriptKey([c isEqualToString:@" "] ? @"space" : c, YES); });
                    onMain(^{ scriptKey([c isEqualToString:@" "] ? @"space" : c, NO); });
                    [NSThread sleepForTimeInterval:0.03];
                }
            } else if ([cmd isEqualToString:@"call"] && args.count >= 2) {
                onMain(^{ scriptCall(args); });
            } else if ([cmd isEqualToString:@"log"]) {
                // already logged
            } else {
                PearHostLog("script", "unknown command: %s", cmd.UTF8String);
            }
        }
        PearHostLog("script", "done");
    }];
}
