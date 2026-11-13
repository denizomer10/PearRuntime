// Original PearSilicon input adapter using public GameController APIs.
// Opt-in only. No operating-system source, copied driver, or touch synthesis.
//
// The game sees a hidden GCVirtualController. Keyboard/mouse events are mapped
// to controller elements from PEAR_VIRTUAL_GAMEPAD_MAP (built by the app), so
// nothing here depends on a particular keyboard layout or hard-coded game.
#import "PearRuntime.h"
#import <GameController/GameController.h>
#import <CoreGraphics/CoreGraphics.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import "PearPointerCaptureState.h"
#import "PearHostKeyNames.h"

// CoreGraphics cursor APIs are macOS APIs that Catalyst links through the host
// CoreGraphics. Declare them directly: the iOS-flavoured umbrella headers do
// not always expose CGRemoteOperation.h for the macabi target.
extern "C" {
    CGError CGAssociateMouseAndMouseCursorPosition(boolean_t connected);
    CGError CGDisplayHideCursor(CGDirectDisplayID display);
    CGError CGDisplayShowCursor(CGDirectDisplayID display);
    CGDirectDisplayID CGMainDisplayID(void);
}

static GCVirtualController *gVirtualPad;
static BOOL gPadReady;
static PearPointerCaptureState gCapture;
static NSMutableSet<NSString *> *gPressedKeys;
static NSMutableSet<NSString *> *gPressedMouseButtons;
static dispatch_source_t gPadTimer;
static id gPadEventMonitor;
static NSInteger gLastCaptureState = -1;

// Capture state. The cursor is captured by disassociating it from the mouse
// instead of relying on the game's pointer lock, which Catalyst often refuses
// for mouse events that arrive without a preceding UIKit gesture.
static BOOL gPadCaptured;
static BOOL gCaptureClickPassthrough;
static BOOL gCursorDisassociated;
static BOOL gCursorHidden;
static BOOL gCursorAPIFailed;

// Right stick motion. GCMouse samples are preferred; NSEvent deltas are a
// fallback when the game/catalyst combination stops forwarding GCMouse.
static float gMouseX, gMouseY;
static double gMouseSensitivity = 1.0;
static BOOL gMouseInvertY;
static double gLastMouseTime;
static double gLastGCMouseTime;
static NSUInteger gMouseReceived, gMouseAccepted, gMouseEventSamples;
static double gLastMouseDiagnostic;

// Mapping tables loaded from PEAR_VIRTUAL_GAMEPAD_MAP.
static NSDictionary<NSString *, NSArray<NSString *> *> *gKeyToControl;
static NSDictionary<NSString *, NSArray<NSString *> *> *gMouseToControl;
static NSDictionary<NSString *, NSString *> *gStickKeyToDirection;

BOOL PearVirtualGamepadEnabled(void) {
    return NSProcessInfo.processInfo.environment[@"PEAR_VIRTUAL_GAMEPAD"].boolValue;
}

static BOOL PearPadForeground(void) {
    Class cls = NSClassFromString(@"NSApplication");
    id app = [cls respondsToSelector:@selector(sharedApplication)]
        ? ((id (*)(id, SEL))objc_msgSend)(cls, @selector(sharedApplication)) : nil;
    BOOL active = app && [app respondsToSelector:@selector(isActive)] &&
        ((BOOL (*)(id, SEL))objc_msgSend)(app, @selector(isActive));
    if (!active) return NO;
    UIApplication *ui = UIApplication.sharedApplication;
    return !ui || ui.applicationState == UIApplicationStateActive;
}

#pragma mark - Mapping

static NSDictionary<NSString *, NSString *> *PearDefaultKeyToControl(void) {
    return @{
        @"space": @"a", @"shift": @"b", @"e": @"x", @"r": @"y",
        @"q": @"l1", @"f": @"r1",
        @"up": @"dpadUp", @"down": @"dpadDown", @"left": @"dpadLeft", @"right": @"dpadRight",
        @"enter": @"menu", @"tab": @"options",
    };
}

static NSDictionary<NSString *, NSString *> *PearDefaultMouseToControl(void) {
    return @{@"1": @"l2", @"0": @"r2"};
}

static NSDictionary<NSString *, NSString *> *PearDefaultStickKeys(void) {
    return @{@"up": @"w", @"down": @"s", @"left": @"a", @"right": @"d"};
}

static BOOL PearIsDpadControl(NSString *control) {
    return [control isEqualToString:@"dpadUp"] || [control isEqualToString:@"dpadDown"] ||
        [control isEqualToString:@"dpadLeft"] || [control isEqualToString:@"dpadRight"];
}

static BOOL PearIsKnownControl(NSString *control) {
    static NSSet<NSString *> *known;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        known = [NSSet setWithArray:@[
            @"a", @"b", @"x", @"y", @"l1", @"r1", @"l2", @"r2",
            @"dpadUp", @"dpadDown", @"dpadLeft", @"dpadRight",
            @"menu", @"options", @"l3", @"r3",
        ]];
    });
    return [known containsObject:control];
}

// Accept old single-control profiles and new lossless multi-control profiles.
static NSArray<NSString *> *PearPadControls(id value) {
    NSArray *values = [value isKindOfClass:NSString.class] ? @[value] :
        ([value isKindOfClass:NSArray.class] ? value : @[]);
    NSMutableOrderedSet *controls = [NSMutableOrderedSet orderedSet];
    for (id item in values)
        if ([item isKindOfClass:NSString.class] && PearIsKnownControl(item)) [controls addObject:item];
    return controls.array;
}

static void PearLoadPadMapping(void) {
    NSMutableDictionary<NSString *, NSArray<NSString *> *> *keys = [NSMutableDictionary dictionary];
    NSMutableDictionary<NSString *, NSArray<NSString *> *> *mouse = [NSMutableDictionary dictionary];
    NSMutableDictionary<NSString *, NSString *> *stick = [NSMutableDictionary dictionary];
    gMouseSensitivity = 1.0;
    gMouseInvertY = NO;

    NSString *raw = NSProcessInfo.processInfo.environment[@"PEAR_VIRTUAL_GAMEPAD_MAP"];
    NSDictionary *root = nil;
    if (raw.length) {
        NSData *data = [raw dataUsingEncoding:NSUTF8StringEncoding];
        NSError *error = nil;
        id parsed = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:&error] : nil;
        if ([parsed isKindOfClass:NSDictionary.class]) {
            root = parsed;
        } else {
            PearLog(@"[PearRuntime][virtual-pad] invalid PEAR_VIRTUAL_GAMEPAD_MAP: %@", error.localizedDescription ?: @"not JSON");
        }
    }

    if (root) {
        NSDictionary *buttons = root[@"buttons"];
        if ([buttons isKindOfClass:NSDictionary.class]) {
            for (NSString *key in buttons) {
                id value = buttons[key];
                if ([key isKindOfClass:NSString.class] && key.length > 0) {
                    keys[key.lowercaseString] = PearPadControls(value);
                }
            }
        }
        NSDictionary *mouseButtons = root[@"mouseButtons"];
        if ([mouseButtons isKindOfClass:NSDictionary.class]) {
            for (NSString *key in mouseButtons) {
                id value = mouseButtons[key];
                if ([key isKindOfClass:NSString.class] &&
                    [key rangeOfCharacterFromSet:NSCharacterSet.decimalDigitCharacterSet.invertedSet].location == NSNotFound && key.length) {
                    NSString *number = [NSString stringWithFormat:@"%ld", (long)[key integerValue]];
                    mouse[number] = PearPadControls(value);
                }
            }
        }
        NSDictionary *leftStick = root[@"leftStick"];
        if ([leftStick isKindOfClass:NSDictionary.class]) {
            for (NSString *direction in leftStick) {
                id key = leftStick[direction];
                if ([direction isKindOfClass:NSString.class] && [key isKindOfClass:NSString.class] &&
                    [key length] > 0 &&
                    ([direction isEqualToString:@"up"] || [direction isEqualToString:@"down"] ||
                     [direction isEqualToString:@"left"] || [direction isEqualToString:@"right"])) {
                    stick[direction] = [key lowercaseString];
                }
            }
        }
        NSNumber *sensitivity = root[@"sensitivity"];
        if ([sensitivity isKindOfClass:NSNumber.class]) {
            gMouseSensitivity = fmin(fmax(sensitivity.doubleValue, 0.2), 3.0);
        }
        id invert = root[@"invertY"];
        if ([invert isKindOfClass:NSNumber.class]) gMouseInvertY = [invert boolValue];
    } else if (!raw.length) {
        for (NSString *key in PearDefaultKeyToControl()) keys[key] = @[PearDefaultKeyToControl()[key]];
        for (NSString *key in PearDefaultMouseToControl()) mouse[key] = @[PearDefaultMouseToControl()[key]];
        stick = [PearDefaultStickKeys() mutableCopy];
    }

    // A valid-but-empty map is intentional (the user cleared every binding).
    gKeyToControl = keys;
    gMouseToControl = mouse;
    gStickKeyToDirection = stick;
    PearLog(@"[PearRuntime][virtual-pad] mapping keys=%lu mouse=%lu stick=%lu sensitivity=%.2f invertY=%d",
            (unsigned long)keys.count, (unsigned long)mouse.count, (unsigned long)stick.count,
            gMouseSensitivity, gMouseInvertY);
}

static NSString *PearElementForControl(NSString *control) {
    static NSDictionary<NSString *, NSString *> *map;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        map = @{
            @"a": GCInputButtonA, @"b": GCInputButtonB,
            @"x": GCInputButtonX, @"y": GCInputButtonY,
            @"l1": GCInputLeftShoulder, @"r1": GCInputRightShoulder,
            @"l2": GCInputLeftTrigger, @"r2": GCInputRightTrigger,
            @"menu": GCInputButtonMenu, @"options": GCInputButtonOptions,
            @"l3": GCInputLeftThumbstickButton, @"r3": GCInputRightThumbstickButton,
        };
    });
    return map[control];
}

static NSSet<NSString *> *PearPadConfigurationElements(void) {
    NSMutableSet<NSString *> *elements = [NSMutableSet setWithArray:@[
        GCInputButtonA, GCInputButtonB, GCInputButtonX, GCInputButtonY,
        GCInputLeftShoulder, GCInputRightShoulder, GCInputLeftTrigger, GCInputRightTrigger,
        GCInputButtonMenu, GCInputDirectionPad, GCInputLeftThumbstick, GCInputRightThumbstick,
    ]];
    NSMutableArray<NSString *> *controls = [NSMutableArray array];
    for (NSArray *values in gKeyToControl.allValues) [controls addObjectsFromArray:values];
    for (NSArray *values in gMouseToControl.allValues) [controls addObjectsFromArray:values];
    for (NSString *control in controls) {
        NSString *element = PearElementForControl(control);
        if (element) [elements addObject:element];
    }
    return elements;
}

#pragma mark - Cursor capture

static void PearPadRestoreCursor(void) {
    if (gCursorHidden) {
        CGDisplayShowCursor(CGMainDisplayID());
        gCursorHidden = NO;
    }
    if (gCursorDisassociated) {
        CGAssociateMouseAndMouseCursorPosition(true);
        gCursorDisassociated = NO;
    }
    gPadCaptured = NO;
}

static void PearPadSetCursorCaptured(BOOL captured) {
    if (captured) {
        if (!gCursorDisassociated && !gCursorAPIFailed) {
            CGError error = CGAssociateMouseAndMouseCursorPosition(false);
            gCursorDisassociated = error == kCGErrorSuccess;
            if (!gCursorDisassociated) {
                gCursorAPIFailed = YES;
                PearLog(@"[PearRuntime][virtual-pad] cursor capture API failed (%d); using window-local mouse deltas", (int)error);
            } else {
                PearLog(@"[PearRuntime][virtual-pad] cursor disassociated");
            }
        }
        if (gCursorDisassociated && !gCursorHidden) {
            CGDisplayHideCursor(CGMainDisplayID());
            gCursorHidden = YES;
        }
        gPadCaptured = YES;
    } else {
        BOOL wasCaptured = gPadCaptured;
        PearPadRestoreCursor();
        if (wasCaptured) PearLog(@"[PearRuntime][virtual-pad] cursor released");
    }
}

#pragma mark - Controller output

static void PearPadSetControl(NSString *control, float value) {
    if (!gPadReady || !control) return;
    NSString *element = PearElementForControl(control);
    if (!element) return;
    if (@available(macCatalyst 17.0, *)) {
        @try {
            [gVirtualPad setValue:value forButtonElement:element];
        } @catch (NSException *exception) {
            static NSMutableSet<NSString *> *warned;
            if (!warned) warned = [NSMutableSet set];
            if (![warned containsObject:element]) {
                [warned addObject:element];
                PearLog(@"[PearRuntime][virtual-pad] element %@ rejected: %@", element, exception.reason);
            }
        }
    }
}

static BOOL PearPadPressed(NSString *key) {
    return key.length > 0 && [gPressedKeys containsObject:key.lowercaseString];
}

static void PearPadUpdateLeftStick(void) {
    if (!gPadReady) return;
    float x = (PearPadPressed(gStickKeyToDirection[@"right"]) ? 1.0f : 0.0f) -
              (PearPadPressed(gStickKeyToDirection[@"left"]) ? 1.0f : 0.0f);
    float y = (PearPadPressed(gStickKeyToDirection[@"up"]) ? 1.0f : 0.0f) -
              (PearPadPressed(gStickKeyToDirection[@"down"]) ? 1.0f : 0.0f);
    float length = hypotf(x, y);
    if (length > 1) { x /= length; y /= length; }
    if (@available(macCatalyst 17.0, *)) {
        [gVirtualPad setPosition:CGPointMake(x, y) forDirectionPadElement:GCInputLeftThumbstick];
    }
}

static NSSet<NSString *> *PearPadPressedControls(void) {
    NSMutableSet *controls = [NSMutableSet set];
    for (NSString *key in gPressedKeys) [controls addObjectsFromArray:gKeyToControl[key] ?: @[]];
    for (NSString *button in gPressedMouseButtons) [controls addObjectsFromArray:gMouseToControl[button] ?: @[]];
    return controls;
}

static void PearPadUpdateDpad(void) {
    if (!gPadReady) return;
    NSSet *controls = PearPadPressedControls();
    float x = ([controls containsObject:@"dpadRight"] ? 1 : 0) - ([controls containsObject:@"dpadLeft"] ? 1 : 0);
    float y = ([controls containsObject:@"dpadUp"] ? 1 : 0) - ([controls containsObject:@"dpadDown"] ? 1 : 0);
    if (@available(macCatalyst 17.0, *))
        [gVirtualPad setPosition:CGPointMake(x, y) forDirectionPadElement:GCInputDirectionPad];
}

static NSSet<NSString *> *PearPadMappedControls(void) {
    NSMutableSet<NSString *> *controls = [NSMutableSet set];
    for (NSArray *values in gKeyToControl.allValues) [controls addObjectsFromArray:values];
    for (NSArray *values in gMouseToControl.allValues) [controls addObjectsFromArray:values];
    return controls;
}

static void PearPadNeutral(void) {
    [gPressedKeys removeAllObjects];
    [gPressedMouseButtons removeAllObjects];
    gMouseX = gMouseY = 0;
    if (!gPadReady) return;
    for (NSString *control in PearPadMappedControls()) {
        if (PearIsDpadControl(control)) continue;
        PearPadSetControl(control, 0);
    }
    if (@available(macCatalyst 17.0, *)) {
        [gVirtualPad setPosition:CGPointZero forDirectionPadElement:GCInputDirectionPad];
        [gVirtualPad setPosition:CGPointZero forDirectionPadElement:GCInputLeftThumbstick];
        [gVirtualPad setPosition:CGPointZero forDirectionPadElement:GCInputRightThumbstick];
    }
}

static void PearPadSetActive(BOOL active) {
    gCapture.setActive(active);
    if (!active) {
        gCaptureClickPassthrough = NO;
        PearPadNeutral();
        PearPadSetCursorCaptured(NO);
        return;
    }
    if (gCapture.wantsLock()) PearPadSetCursorCaptured(YES);
}

#pragma mark - Event input

static BOOL PearPadClickInContent(id event) {
    id window = [event window];
    if (!window || ![window respondsToSelector:@selector(isKeyWindow)] ||
        ![window respondsToSelector:@selector(contentView)]) return NO;
    if (!((BOOL (*)(id, SEL))objc_msgSend)(window, @selector(isKeyWindow))) return NO;
    id view = [window contentView];
    if (![view respondsToSelector:@selector(convertPoint:fromView:)] ||
        ![view respondsToSelector:@selector(bounds)]) return NO;
    NSPoint location = ((NSPoint (*)(id, SEL))objc_msgSend)(event, @selector(locationInWindow));
    NSPoint point = ((NSPoint (*)(id, SEL, NSPoint, id))objc_msgSend)(view, @selector(convertPoint:fromView:), location, nil);
    NSRect bounds = ((NSRect (*)(id, SEL))objc_msgSend)(view, @selector(bounds));
    return NSPointInRect(point, bounds);
}

static void PearPadMouseButton(NSInteger button, BOOL pressed) {
    NSArray *controls = gMouseToControl[[NSString stringWithFormat:@"%ld", (long)button]];
    NSString *buttonName = [NSString stringWithFormat:@"%ld", (long)button];
    if (pressed) [gPressedMouseButtons addObject:buttonName]; else [gPressedMouseButtons removeObject:buttonName];
    NSSet *active = PearPadPressedControls();
    for (NSString *control in controls)
        if (!PearIsDpadControl(control)) PearPadSetControl(control, [active containsObject:control] ? 1 : 0);
    PearPadUpdateDpad();
    if (getenv("PEAR_DEBUG_POINTER")) {
        PearLog(@"[PearRuntime][virtual-pad] mouse button=%ld pressed=%d control=%@",
                (long)button, pressed, controls);
    }
}

static void PearPadKey(NSString *key, BOOL pressed) {
    if (!PearVirtualGamepadEnabled() || key.length == 0) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!gPadReady || !PearPadForeground()) return;
        NSString *name = key.lowercaseString;
        if ([name isEqualToString:@"escape"]) {
            if (gCapture.escape(pressed)) {
                PearPadNeutral();
                PearPadSetCursorCaptured(NO);
            }
            return;
        }
        if ([gPressedKeys containsObject:name] == pressed) return; // Ignore key repeat / duplicate delivery.
        if (pressed) [gPressedKeys addObject:name]; else [gPressedKeys removeObject:name];
        NSArray *controls = gKeyToControl[name];
        NSSet *active = PearPadPressedControls();
        for (NSString *control in controls)
            if (!PearIsDpadControl(control)) PearPadSetControl(control, [active containsObject:control] ? 1 : 0);
        PearPadUpdateDpad();
        PearPadUpdateLeftStick();
        if (getenv("PEAR_DEBUG_POINTER")) {
            PearLog(@"[PearRuntime][virtual-pad] key=%@ pressed=%d control=%@",
                    name, pressed, controls ?: @[]);
        }
    });
}

void PearVirtualGamepadKey(NSString *key, BOOL pressed) {
    // Prefer local native events when available, so GCKeyboard and AppKit
    // cannot release each other's held keys. GC remains the fallback host.
    if (!gPadEventMonitor) PearPadKey(key, pressed);
}

void PearVirtualGamepadMouse(float dx, float dy) {
    if (!PearVirtualGamepadEnabled()) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        gMouseReceived++;
        if (!gPadReady || !gPadCaptured || !PearPadForeground()) return;
        if (!isfinite(dx) || !isfinite(dy)) return;
        gMouseAccepted++;
        gLastGCMouseTime = NSProcessInfo.processInfo.systemUptime;
        float scale = 0.035f * (float)gMouseSensitivity;
        float vertical = gMouseInvertY ? -dy : dy;
        gMouseX = fmaxf(-1, fminf(1, gMouseX + dx * scale));
        gMouseY = fmaxf(-1, fminf(1, gMouseY + vertical * scale));
        gLastMouseTime = gLastGCMouseTime;
    });
}

/// NSEvent fallback for hosts where GCMouse does not deliver while the cursor
/// is disassociated. Ignored for a quarter second after a GCMouse sample.
void PearVirtualGamepadMouseEvent(float dx, float dy) {
    if (!PearVirtualGamepadEnabled()) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        gMouseEventSamples++;
        if (!gPadReady || !gPadCaptured || !PearPadForeground()) return;
        if (!isfinite(dx) || !isfinite(dy)) return;
        double now = NSProcessInfo.processInfo.systemUptime;
        if (now - gLastGCMouseTime < 0.25) return;
        gMouseAccepted++;
        float scale = 0.035f * (float)gMouseSensitivity;
        float vertical = gMouseInvertY ? -dy : dy;
        gMouseX = fmaxf(-1, fminf(1, gMouseX + dx * scale));
        gMouseY = fmaxf(-1, fminf(1, gMouseY + vertical * scale));
        gLastMouseTime = now;
    });
}

#pragma mark - Install

static void PearConnectVirtualPad(NSSet<NSString *> *elements, BOOL allowFallback) {
    if (@available(macCatalyst 17.0, *)) {
        GCVirtualControllerConfiguration *configuration = [GCVirtualControllerConfiguration new];
        configuration.elements = elements;
        configuration.hidden = YES;
        gVirtualPad = [[GCVirtualController alloc] initWithConfiguration:configuration];
        [gVirtualPad connectWithReplyHandler:^(NSError *error) {
            dispatch_async(dispatch_get_main_queue(), ^{
                if (error) {
                    BOOL hasOptional = [elements containsObject:GCInputButtonOptions] ||
                        [elements containsObject:GCInputLeftThumbstickButton] ||
                        [elements containsObject:GCInputRightThumbstickButton];
                    if (allowFallback && hasOptional) {
                        NSMutableSet<NSString *> *reduced = [elements mutableCopy];
                        [reduced minusSet:[NSSet setWithObjects:GCInputButtonOptions,
                                           GCInputLeftThumbstickButton, GCInputRightThumbstickButton, nil]];
                        PearLog(@"[PearRuntime][virtual-pad] retry without optional elements: %@", error.localizedDescription);
                        gVirtualPad = nil;
                        PearConnectVirtualPad(reduced, NO);
                        return;
                    }
                    PearLog(@"[PearRuntime][virtual-pad] connection failed: %@", error.localizedDescription);
                    return;
                }
                gPadReady = gVirtualPad.controller != nil;
                PearLog(@"[PearRuntime][virtual-pad] connected=%d elements=%lu", gPadReady, (unsigned long)elements.count);
                if (gPadReady) PearPadSetActive(PearPadForeground());
            });
        }];
    }
}

void PearInstallVirtualGamepad(void) {
    if (!PearVirtualGamepadEnabled()) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        if (@available(macCatalyst 17.0, *)) {
            if (![GCVirtualControllerConfiguration instancesRespondToSelector:@selector(setHidden:)] ||
                ![GCVirtualController instancesRespondToSelector:@selector(setPosition:forDirectionPadElement:)]) {
                PearLog(@"[PearRuntime][virtual-pad] hidden virtual-controller API unavailable");
                return;
            }
            gPressedKeys = [NSMutableSet set];
            gPressedMouseButtons = [NSMutableSet set];
            gCapture.requested = false;
            PearLoadPadMapping();
            atexit(PearPadRestoreCursor);
            PearConnectVirtualPad(PearPadConfigurationElements(), YES);

            gPadTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
            dispatch_source_set_timer(gPadTimer, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), NSEC_PER_SEC / 60, NSEC_PER_SEC / 600);
            dispatch_source_set_event_handler(gPadTimer, ^{
                if (!gPadReady) return;
                BOOL foreground = PearPadForeground();
                if (gCapture.suspended == foreground) PearPadSetActive(foreground);

                NSInteger captureState = (foreground ? 1 : 0) | (gPadCaptured ? 2 : 0) |
                    (gCapture.requested ? 4 : 0);
                if (captureState != gLastCaptureState && getenv("PEAR_DEBUG_POINTER")) {
                    PearLog(@"[PearRuntime][virtual-pad] capture requested=%d foreground=%d captured=%d cursorAssoc=%d",
                            gCapture.requested, foreground, gPadCaptured, gCursorDisassociated);
                }
                gLastCaptureState = captureState;
                if (!foreground) return;

                double now = NSProcessInfo.processInfo.systemUptime;
                if (!gPadCaptured || now - gLastMouseTime > 0.1) gMouseX = gMouseY = 0;
                if (@available(macCatalyst 17.0, *)) {
                    [gVirtualPad setPosition:CGPointMake(gMouseX, gMouseY)
                        forDirectionPadElement:GCInputRightThumbstick];
                }
                if (getenv("PEAR_DEBUG_POINTER") && (gMouseReceived + gMouseEventSamples) > 0 &&
                    now - gLastMouseDiagnostic >= 1) {
                    id stick = nil;
                    if (@available(macCatalyst 17.0, *)) stick = gVirtualPad.controller.extendedGamepad.rightThumbstick;
                    PearLog(@"[PearRuntime][virtual-pad] mouse gc=%lu event=%lu accepted=%lu right=(%.3f,%.3f) readback=(%.3f,%.3f) captured=%d",
                            (unsigned long)gMouseReceived, (unsigned long)gMouseEventSamples,
                            (unsigned long)gMouseAccepted, gMouseX, gMouseY,
                            stick ? ((GCControllerDirectionPad *)stick).xAxis.value : 0.0f,
                            stick ? ((GCControllerDirectionPad *)stick).yAxis.value : 0.0f,
                            gPadCaptured);
                    gMouseReceived = gMouseAccepted = gMouseEventSamples = 0;
                    gLastMouseDiagnostic = now;
                }
                gMouseX *= 0.7f; gMouseY *= 0.7f;
            });
            dispatch_resume(gPadTimer);

            Class eventClass = NSClassFromString(@"NSEvent");
            SEL monitor = NSSelectorFromString(@"addLocalMonitorForEventsMatchingMask:handler:");
            if ([eventClass respondsToSelector:monitor]) {
                // left/right/other mouse down+up, mouseMoved, Escape down+up.
                NSUInteger mask = (1u << 1) | (1u << 2) | (1u << 3) | (1u << 4) |
                    (1u << 5) | (1u << 6) | (1u << 7) | (1u << 27) | (1u << 10) | (1u << 11) | (1u << 12) | (1u << 25) | (1u << 26);
                id (^handler)(id) = ^id(id event) {
                    if (!gPadReady || !PearPadForeground()) return event;
                    NSInteger type = ((NSInteger (*)(id, SEL))objc_msgSend)(event, @selector(type));

                    if (type == 10 || type == 11 || type == 12) {
                        unsigned short code = ((unsigned short (*)(id, SEL))objc_msgSend)(event, @selector(keyCode));
                        NSString *name = PearHostKeyName(code);
                        NSUInteger flags = ((NSUInteger (*)(id, SEL))objc_msgSend)(event, @selector(modifierFlags));
                        if (type == 12) {
                            // AppKit supplies aggregate left/right modifier state.
                            NSDictionary *modifiers = @{@"shift": @(1u << 17), @"control": @(1u << 18),
                                @"option": @(1u << 19), @"command": @(1u << 20), @"capslock": @(1u << 16)};
                            NSNumber *mask = modifiers[name ?: @""];
                            if (mask) PearPadKey(name, (flags & mask.unsignedIntegerValue) != 0);
                        } else if (name) {
                            PearPadKey(name, type == 10);
                        }
                        // Preserve system shortcuts such as Command-Tab.
                        BOOL mapped = gKeyToControl[name ?: @""].count > 0 || [gStickKeyToDirection.allValues containsObject:name ?: @""];
                        return mapped && !(flags & (1u << 20)) && type != 12 ? nil : event;
                    }

                    if (type == 5 || type == 6 || type == 7 || type == 27) { // native movement and drag
                        if (gPadCaptured) {
                            double dx = ((double (*)(id, SEL))objc_msgSend)(event, @selector(deltaX));
                            double dy = ((double (*)(id, SEL))objc_msgSend)(event, @selector(deltaY));
                            PearVirtualGamepadMouseEvent((float)dx, (float)dy);
                        }
                        return event;
                    }

                    BOOL down = (type == 1 || type == 3 || type == 25);
                    BOOL up = (type == 2 || type == 4 || type == 26);
                    if (!down && !up) return event;

                    if (type == 2 && gCaptureClickPassthrough) { // left up of the arming click
                        gCaptureClickPassthrough = NO;
                        return event;
                    }

                    if (!gPadCaptured) {
                        if (type == 1 && PearPadClickInContent(event)) {
                            gCapture.captureFromClick();
                            PearPadSetActive(YES);
                            gCaptureClickPassthrough = YES;
                            // Let UIKit see the activating gesture so the click
                            // also reaches the game's menu.
                            return event;
                        }
                        return event;
                    }

                    NSInteger button = 0;
                    if (type == 3 || type == 4) button = 1;
                    else if (type == 25 || type == 26) {
                        button = ((NSInteger (*)(id, SEL))objc_msgSend)(event, @selector(buttonNumber));
                    }
                    PearPadMouseButton(button, down);
                    return nil;
                };
                gPadEventMonitor = ((id (*)(id, SEL, NSUInteger, id))objc_msgSend)(eventClass, monitor, mask, handler);
            }

            for (NSString *name in @[UIApplicationWillResignActiveNotification, @"NSApplicationWillResignActiveNotification"])
                [NSNotificationCenter.defaultCenter addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) { PearPadSetActive(NO); }];
            for (NSString *name in @[UIApplicationDidBecomeActiveNotification, @"NSApplicationDidBecomeActiveNotification"])
                [NSNotificationCenter.defaultCenter addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) { PearPadSetActive(YES); }];
            for (NSString *name in @[UIApplicationWillTerminateNotification, @"NSApplicationWillTerminateNotification"])
                [NSNotificationCenter.defaultCenter addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) { PearPadSetActive(NO); PearPadRestoreCursor(); }];
        } else {
            PearLog(@"[PearRuntime][virtual-pad] requires Mac Catalyst 17 or newer");
        }
    });
}
