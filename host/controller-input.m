// GameController bridge for the native PearHost process.
#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <GameController/GameController.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import "PearControllerHID.h"
#include <stdatomic.h>
#include <dlfcn.h>

extern void PearHostLogMessage(const char *area, const char *message);

static GCController *keyboardController;
static IMP originalControllers;
static IMP originalCurrent;
static BOOL controllerIdentityEnabled;
static atomic_bool virtualAdvertised;
static _Thread_local BOOL queryingControllers;
static char controllerIdentityKey;
static BOOL replayEnabled;
static BOOL debugControllerCalls;
static atomic_uint controllerLogs;
static atomic_uint virtualInputLogs;
static char virtualProfileHandlerKey;
static char virtualPhysicalHandlerKey;
static char bridgedProfileKey;
static NSArray<GCController *> *hidControllers;
static PearHIDGamepadMonitor *hidMonitor;

static void logControllerCall(NSString *call) {
    if (debugControllerCalls && atomic_fetch_add(&controllerLogs, 1) < 70) {
        PearHostLogMessage("input", call.UTF8String);
    }
}

typedef void (^PearProfileHandler)(GCExtendedGamepad *, GCControllerElement *);

static void installProfileHandlerProperty(Class type, SEL get, SEL set, const void *key) {
    // Cocoa's mutable snapshot calls element handlers, but does not publish
    // profile-wide changes. Store that handler separately for our live pad.
    Method getter = class_getInstanceMethod(type, get);
    Method setter = class_getInstanceMethod(type, set);
    IMP originalGet = method_getImplementation(getter);
    IMP originalSet = method_getImplementation(setter);
    class_replaceMethod(type, get, imp_implementationWithBlock(^PearProfileHandler(GCExtendedGamepad *pad) {
        if (objc_getAssociatedObject(pad, &bridgedProfileKey)) return objc_getAssociatedObject(pad, key);
        return ((PearProfileHandler (*)(id, SEL))originalGet)(pad, get);
    }), method_getTypeEncoding(getter));
    class_replaceMethod(type, set, imp_implementationWithBlock(^(GCExtendedGamepad *pad, PearProfileHandler handler) {
        if (objc_getAssociatedObject(pad, &bridgedProfileKey)) {
            objc_setAssociatedObject(pad, key, handler, OBJC_ASSOCIATION_COPY);
            if (debugControllerCalls) PearHostLogMessage("input", handler ? "Virtual profile handler attached" : "Virtual profile handler cleared");
        } else {
            ((void (*)(id, SEL, PearProfileHandler))originalSet)(pad, set, handler);
        }
    }), method_getTypeEncoding(setter));
}

static void installVirtualProfileHandler(GCExtendedGamepad *profile) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        Class type = object_getClass(profile);
        installProfileHandlerProperty(type, @selector(valueChangedHandler), @selector(setValueChangedHandler:), &virtualProfileHandlerKey);
        installProfileHandlerProperty(type, @selector(valueDidChangeHandler), @selector(setValueDidChangeHandler:), &virtualPhysicalHandlerKey);
    });
    objc_setAssociatedObject(profile, &bridgedProfileKey, @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
}

static void publishVirtualChanges(GCController *controller, NSArray<GCControllerElement *> *changed) {
    GCExtendedGamepad *pad = controller.extendedGamepad;
    GCExtendedGamepadValueChangedHandler handler = pad.valueChangedHandler;
    void (^physicalHandler)(GCPhysicalInputProfile *, GCControllerElement *) = pad.valueDidChangeHandler;
    if ((!handler && !physicalHandler) || !changed.count) return;
    dispatch_async(controller.handlerQueue ?: dispatch_get_main_queue(), ^{
        for (GCControllerElement *element in changed) {
            if (handler) handler(pad, element);
            if (physicalHandler) physicalHandler(pad, element);
        }
    });
}

static BOOL updateDirection(GCControllerDirectionPad *direction, float x, float y) {
    if (direction.xAxis.value == x && direction.yAxis.value == y) return NO;
    [direction setValueForXAxis:x yAxis:y];
    return YES;
}

static void updateVirtualLook(float x, float y) {
    GCExtendedGamepad *pad = keyboardController.extendedGamepad;
    if (updateDirection(pad.rightThumbstick, x, y)) publishVirtualChanges(keyboardController, @[pad.rightThumbstick]);
}

static NSArray<GCController *> *physicalControllers(void) {
    if (queryingControllers) return @[];
    queryingControllers = YES;
    NSArray<GCController *> *physical = ((id (*)(id, SEL))originalControllers)(GCController.class, @selector(controllers));
    queryingControllers = NO;
    return physical;
}

static void identifyControllerClass(Class type) {
    if (!controllerIdentityEnabled || !type) return;
    @synchronized (GCController.class) {
        static NSMutableSet<NSString *> *classes;
        if (!classes) classes = [NSMutableSet set];
        NSString *name = NSStringFromClass(type);
        if ([classes containsObject:name]) return;
        [classes addObject:name];

        for (NSString *selectorName in @[@"vendorName", @"productCategory"]) {
            SEL selector = NSSelectorFromString(selectorName);
            Method method = class_getInstanceMethod(type, selector);
            if (!method) continue;
            IMP original = method_getImplementation(method);
            BOOL category = [selectorName isEqualToString:@"productCategory"];
            IMP replacement = imp_implementationWithBlock(^NSString *(GCController *controller) {
                if (objc_getAssociatedObject(controller, &controllerIdentityKey)) {
                    return category ? GCProductCategoryDualSense : @"DualSense Wireless Controller";
                }
                return ((id (*)(id, SEL))original)(controller, selector);
            });
            class_replaceMethod(type, selector, replacement, method_getTypeEncoding(method));
        }
    }
}

static void identifyController(GCController *controller) {
    if (!controllerIdentityEnabled || !controller ||
        objc_getAssociatedObject(controller, &controllerIdentityKey) || !controller.extendedGamepad) return;
    identifyControllerClass(object_getClass(controller));
    objc_setAssociatedObject(controller, &controllerIdentityKey, @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
}

static NSArray<GCController *> *guestControllers(void) {
    NSArray<GCController *> *physical = physicalControllers();
    for (GCController *pad in physical) identifyController(pad);
    @synchronized (GCController.class) {
        if (hidControllers.count) physical = [physical arrayByAddingObjectsFromArray:hidControllers];
    }
    if (atomic_load(&virtualAdvertised) && ![physical containsObject:keyboardController]) {
        return [physical arrayByAddingObject:keyboardController];
    }
    return physical;
}

static NSArray<GCController *> *controllers(id self, SEL cmd) {
    NSArray *result = guestControllers();
    if (debugControllerCalls) logControllerCall([NSString stringWithFormat:@"+[GCController controllers] -> %lu (virtual ready=%d)",
        (unsigned long)result.count, atomic_load(&virtualAdvertised)]);
    return result;
}

static GCController *currentController(id self, SEL cmd) {
    GCController *current = ((id (*)(id, SEL))originalCurrent)(self, cmd) ?: physicalControllers().firstObject;
    identifyController(current);
    if (debugControllerCalls) logControllerCall(@"+[GCController current]");
    if (!current) {
        @synchronized (GCController.class) {
            current = hidControllers.firstObject;
        }
    }
    return current ?: (atomic_load(&virtualAdvertised) ? keyboardController : nil);
}

static void startHIDFallback(void) {
    hidMonitor = [PearHIDGamepadMonitor new];
    hidMonitor.connectionChanged = ^(PearHIDGamepad *gamepad, BOOL connected) {
        GCController *controller = gamepad.controller;
        if (connected) {
            installVirtualProfileHandler(controller.extendedGamepad);
            identifyController(controller);
        }
        @synchronized (GCController.class) {
            NSMutableArray *updated = [hidControllers mutableCopy] ?: [NSMutableArray array];
            if (connected) [updated addObject:controller];
            else [updated removeObjectIdenticalTo:controller];
            hidControllers = [updated copy];
        }
        PearHostLogMessage("input", [NSString stringWithFormat:@"HID %@: %@ (%@)",
            connected ? @"connected" : @"disconnected", gamepad.name, controller.vendorName].UTF8String);
        [NSNotificationCenter.defaultCenter postNotificationName:connected ? GCControllerDidConnectNotification :
            GCControllerDidDisconnectNotification object:controller];
    };
    hidMonitor.inputChanged = ^(PearHIDGamepad *gamepad, GCControllerElement *element) {
        publishVirtualChanges(gamepad.controller, @[element]);
        if (debugControllerCalls && [element isKindOfClass:GCControllerButtonInput.class] &&
            atomic_fetch_add(&virtualInputLogs, 1) < 48) {
            GCControllerButtonInput *button = (GCControllerButtonInput *)element;
            PearHostLogMessage("input", [NSString stringWithFormat:@"HID input: %@ %@ %.2f",
                gamepad.name, button.aliases.anyObject, button.value].UTF8String);
        }
    };
    if (![hidMonitor start]) {
        hidMonitor = nil;
        if (debugControllerCalls) PearHostLogMessage("input", "HID monitor unavailable");
    }
}

static IMP originalSnapshot;

static BOOL inspectSnapshot(GCController *controller, SEL selector) {
    BOOL value = ((BOOL (*)(id, SEL))originalSnapshot)(controller, selector);
    if (debugControllerCalls && controller == keyboardController) {
        Dl_info caller = {0};
        dladdr(__builtin_return_address(0), &caller);
        logControllerCall([NSString stringWithFormat:@"-[GCController isSnapshot] -> %d (%s)", value,
            caller.dli_fname ? strrchr(caller.dli_fname, '/') ? strrchr(caller.dli_fname, '/') + 1 : caller.dli_fname : "?"]);
    }
    return value;
}

static void installControllerDiscovery(void) {
    Method method = class_getClassMethod(GCController.class, @selector(controllers));
    originalControllers = method_getImplementation(method);
    method_setImplementation(method, (IMP)controllers);
    method = class_getClassMethod(GCController.class, @selector(current));
    if (method) {
        originalCurrent = method_getImplementation(method);
        method_setImplementation(method, (IMP)currentController);
    }

    // Unity's NSBundle lookup can look for an iOS flat framework executable.
    Method classLookup = class_getInstanceMethod(NSBundle.class, @selector(classNamed:));
    IMP originalLookup = method_getImplementation(classLookup);
    method_setImplementation(classLookup, imp_implementationWithBlock(^Class(NSBundle *bundle, NSString *name) {
        Class found = ((Class (*)(id, SEL, NSString *))originalLookup)(bundle, @selector(classNamed:), name);
        if (![bundle.bundleURL.lastPathComponent isEqualToString:@"GameController.framework"]) return found;
        if (debugControllerCalls) logControllerCall([NSString stringWithFormat:@"GameController classNamed:%@ -> %@",
            name, found ? NSStringFromClass(found) : @"fallback"]);
        if (found || ![name hasPrefix:@"GC"]) return found;
        Class candidate = NSClassFromString(name);
        return candidate && [[[NSBundle bundleForClass:candidate] bundleIdentifier] isEqualToString:@"com.apple.GameController"]
            ? candidate : Nil;
    }));
}

static NSArray<GCController *> *connectedBeforeRegistration(NSNotificationCenter *center, NSString *name, id object) {
    if (!replayEnabled || center != NSNotificationCenter.defaultCenter ||
        ![name isEqualToString:GCControllerDidConnectNotification]) return @[];
    NSArray<GCController *> *controllers = guestControllers();
    if (!object) return controllers;
    return [controllers containsObject:object] ? @[object] : @[];
}

static void replayConnections(NSArray<GCController *> *previous, void (^deliver)(NSNotification *)) {
    if (!previous.count) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSArray *connected = guestControllers();
        for (GCController *pad in previous) {
            if ([connected containsObject:pad]) {
                deliver([NSNotification notificationWithName:GCControllerDidConnectNotification object:pad]);
            }
        }
    });
}

static void installConnectReplay(void) {
    Method method = class_getInstanceMethod(NSNotificationCenter.class, @selector(addObserver:selector:name:object:));
    IMP original = method_getImplementation(method);
    method_setImplementation(method, imp_implementationWithBlock(
        ^(NSNotificationCenter *center, id target, SEL callback, NSString *name, id object) {
            NSArray *previous = connectedBeforeRegistration(center, name, object);
            ((void (*)(id, SEL, id, SEL, id, id))original)(center, @selector(addObserver:selector:name:object:),
                                                         target, callback, name, object);
            if (!previous.count) return;
            __weak id observer = target;
            replayConnections(previous, ^(NSNotification *note) {
                id live = observer;
                if (live) ((void (*)(id, SEL, id))objc_msgSend)(live, callback, note);
            });
        }));

    Method blockMethod = class_getInstanceMethod(NSNotificationCenter.class, @selector(addObserverForName:object:queue:usingBlock:));
    IMP originalBlock = method_getImplementation(blockMethod);
    method_setImplementation(blockMethod, imp_implementationWithBlock(
        ^id(NSNotificationCenter *center, NSString *name, id object, NSOperationQueue *queue,
            void (^callback)(NSNotification *)) {
            NSArray *previous = connectedBeforeRegistration(center, name, object);
            id token = ((id (*)(id, SEL, id, id, id, id))originalBlock)(
                center, @selector(addObserverForName:object:queue:usingBlock:), name, object, queue, callback);
            __weak id observer = token;
            replayConnections(previous, ^(NSNotification *note) {
                if (!observer) return;
                if (queue) [queue addOperationWithBlock:^{ if (observer) callback(note); }];
                else callback(note);
            });
            return token;
        }));
}

static NSString *eventKey(NSEvent *event) {
    // Carbon's ANSI virtual key codes are physical positions, like the editor.
    static NSString *const keys[] = {
        [kVK_ANSI_A] = @"a", [kVK_ANSI_B] = @"b", [kVK_ANSI_C] = @"c", [kVK_ANSI_D] = @"d",
        [kVK_ANSI_E] = @"e", [kVK_ANSI_F] = @"f", [kVK_ANSI_G] = @"g", [kVK_ANSI_H] = @"h",
        [kVK_ANSI_I] = @"i", [kVK_ANSI_J] = @"j", [kVK_ANSI_K] = @"k", [kVK_ANSI_L] = @"l",
        [kVK_ANSI_M] = @"m", [kVK_ANSI_N] = @"n", [kVK_ANSI_O] = @"o", [kVK_ANSI_P] = @"p",
        [kVK_ANSI_Q] = @"q", [kVK_ANSI_R] = @"r", [kVK_ANSI_S] = @"s", [kVK_ANSI_T] = @"t",
        [kVK_ANSI_U] = @"u", [kVK_ANSI_V] = @"v", [kVK_ANSI_W] = @"w", [kVK_ANSI_X] = @"x",
        [kVK_ANSI_Y] = @"y", [kVK_ANSI_Z] = @"z", [kVK_ANSI_0] = @"0", [kVK_ANSI_1] = @"1",
        [kVK_ANSI_2] = @"2", [kVK_ANSI_3] = @"3", [kVK_ANSI_4] = @"4", [kVK_ANSI_5] = @"5",
        [kVK_ANSI_6] = @"6", [kVK_ANSI_7] = @"7", [kVK_ANSI_8] = @"8", [kVK_ANSI_9] = @"9",
        [kVK_ANSI_Equal] = @"equal", [kVK_ANSI_Minus] = @"minus", [kVK_ANSI_Grave] = @"grave",
        [kVK_ANSI_LeftBracket] = @"openbracket", [kVK_ANSI_RightBracket] = @"closebracket",
        [kVK_ANSI_Semicolon] = @"semicolon", [kVK_ANSI_Quote] = @"quote", [kVK_ANSI_Backslash] = @"backslash",
        [kVK_ANSI_Comma] = @"comma", [kVK_ANSI_Period] = @"period", [kVK_ANSI_Slash] = @"slash",
        [kVK_Return] = @"enter", [kVK_ANSI_KeypadEnter] = @"enter", [kVK_Tab] = @"tab",
        [kVK_Delete] = @"backspace", [kVK_ForwardDelete] = @"delete", [kVK_Space] = @"space",
        [kVK_Escape] = @"escape", [kVK_Home] = @"home", [kVK_End] = @"end", [kVK_PageUp] = @"pageup",
        [kVK_PageDown] = @"pagedown", [kVK_LeftArrow] = @"left", [kVK_RightArrow] = @"right",
        [kVK_UpArrow] = @"up", [kVK_DownArrow] = @"down",
        [kVK_Shift] = @"shift", [kVK_RightShift] = @"shift", [kVK_Control] = @"control",
        [kVK_RightControl] = @"control", [kVK_Option] = @"option", [kVK_RightOption] = @"option",
        [kVK_Command] = @"command", [kVK_RightCommand] = @"command", [kVK_CapsLock] = @"capslock",
        [kVK_F1] = @"f1", [kVK_F2] = @"f2", [kVK_F3] = @"f3", [kVK_F4] = @"f4",
        [kVK_F5] = @"f5", [kVK_F6] = @"f6", [kVK_F7] = @"f7", [kVK_F8] = @"f8",
        [kVK_F9] = @"f9", [kVK_F10] = @"f10", [kVK_F11] = @"f11", [kVK_F12] = @"f12"
    };
    if (event.keyCode < sizeof(keys) / sizeof(*keys) && keys[event.keyCode]) return keys[event.keyCode];
    if (event.type == NSEventTypeFlagsChanged) return nil;
    NSString *name = event.charactersIgnoringModifiers.lowercaseString;
    return name.length == 1 ? name : nil;
}

static BOOL screenMouseCaptured;

static void restorePearHostPointer(void) {
    if (!screenMouseCaptured) return;
    screenMouseCaptured = NO;
    CGAssociateMouseAndMouseCursorPosition(true);
    [NSCursor unhide];
}

@interface PearHostKeyboardPad : NSObject
@property(nonatomic, strong) NSDictionary<NSString *, NSArray<NSString *> *> *keys;
@property(nonatomic, strong) NSDictionary<NSString *, NSArray<NSString *> *> *mouse;
@property(nonatomic, strong) NSDictionary<NSString *, NSString *> *stick;
@property(nonatomic, strong) NSDictionary<NSString *, GCControllerButtonInput *> *buttons;
@property(nonatomic, strong) NSMutableSet<NSString *> *heldKeys;
@property(nonatomic, strong) NSMutableSet<NSString *> *heldMouse;
@property(nonatomic, strong) NSTimer *lookTimer;
@property(nonatomic) double sensitivity;
@property(nonatomic) BOOL invertY;
@property(nonatomic) BOOL captureShortcutDown;
- (void)start;
@end

@implementation PearHostKeyboardPad

- (void)start {
    NSString *config = NSProcessInfo.processInfo.environment[@"PEAR_VIRTUAL_GAMEPAD_MAP"];
    NSDictionary *mapping = config ? [NSJSONSerialization JSONObjectWithData:[config dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL] : nil;
    if (![mapping isKindOfClass:NSDictionary.class]) mapping = nil;
    self.keys = [mapping[@"buttons"] isKindOfClass:NSDictionary.class] ? mapping[@"buttons"] : @{
        @"space": @[@"a"], @"shift": @[@"b"], @"e": @[@"x"], @"r": @[@"y"],
        @"q": @[@"l1"], @"f": @[@"r1"], @"up": @[@"dpadUp"], @"down": @[@"dpadDown"],
        @"left": @[@"dpadLeft"], @"right": @[@"dpadRight"], @"enter": @[@"menu"], @"tab": @[@"options"]
    };
    self.mouse = [mapping[@"mouseButtons"] isKindOfClass:NSDictionary.class] ? mapping[@"mouseButtons"] : @{@"0": @[@"r2"], @"1": @[@"l2"]};
    self.stick = [mapping[@"leftStick"] isKindOfClass:NSDictionary.class] ? mapping[@"leftStick"] :
        @{@"up": @"w", @"down": @"s", @"left": @"a", @"right": @"d"};
    self.sensitivity = [mapping[@"sensitivity"] isKindOfClass:NSNumber.class] ? fmin(3, fmax(.2, [mapping[@"sensitivity"] doubleValue])) : 1;
    self.invertY = [mapping[@"invertY"] isKindOfClass:NSNumber.class] && [mapping[@"invertY"] boolValue];
    self.heldKeys = [NSMutableSet set];
    self.heldMouse = [NSMutableSet set];

    GCExtendedGamepad *pad = keyboardController.extendedGamepad;
    NSMutableDictionary *buttons = [@{@"a": pad.buttonA, @"b": pad.buttonB, @"x": pad.buttonX, @"y": pad.buttonY,
        @"l1": pad.leftShoulder, @"r1": pad.rightShoulder, @"l2": pad.leftTrigger, @"r2": pad.rightTrigger,
        @"menu": pad.buttonMenu} mutableCopy];
    buttons[@"options"] = pad.buttonOptions;
    buttons[@"l3"] = pad.leftThumbstickButton;
    buttons[@"r3"] = pad.rightThumbstickButton;
    self.buttons = buttons;

    [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown | NSEventMaskKeyUp | NSEventMaskFlagsChanged |
        NSEventMaskLeftMouseDown | NSEventMaskLeftMouseUp | NSEventMaskRightMouseDown | NSEventMaskRightMouseUp |
        NSEventMaskOtherMouseDown | NSEventMaskOtherMouseUp | NSEventMaskMouseMoved | NSEventMaskLeftMouseDragged |
        NSEventMaskRightMouseDragged | NSEventMaskOtherMouseDragged handler:^NSEvent *(NSEvent *event) {
            return [self handleEvent:event];
        }];
    for (NSNotificationName notification in @[NSApplicationWillResignActiveNotification, NSApplicationWillTerminateNotification,
                                              NSWindowDidResignKeyNotification]) {
        [NSNotificationCenter.defaultCenter addObserverForName:notification object:nil queue:nil usingBlock:^(__unused NSNotification *note) {
            [self releaseInput];
        }];
    }
}

- (void)updatePad {
    NSMutableSet<NSString *> *pressed = [NSMutableSet set];
    for (NSString *key in self.heldKeys) {
        id controls = self.keys[key];
        if ([controls isKindOfClass:NSArray.class]) [pressed addObjectsFromArray:controls];
    }
    for (NSString *key in self.heldMouse) {
        id controls = self.mouse[key];
        if ([controls isKindOfClass:NSArray.class]) [pressed addObjectsFromArray:controls];
    }
    NSMutableArray<GCControllerElement *> *changed = [NSMutableArray array];
    for (NSString *name in self.buttons) {
        GCControllerButtonInput *button = self.buttons[name];
        float value = [pressed containsObject:name] ? 1 : 0;
        if (button.value != value) {
            [button setValue:value];
            [changed addObject:button];
        }
    }
    GCExtendedGamepad *pad = keyboardController.extendedGamepad;
    int dpadX = (int)[pressed containsObject:@"dpadRight"] - (int)[pressed containsObject:@"dpadLeft"];
    int dpadY = (int)[pressed containsObject:@"dpadUp"] - (int)[pressed containsObject:@"dpadDown"];
    if (updateDirection(pad.dpad, dpadX, dpadY)) [changed addObject:pad.dpad];
    float x = (int)[self holdsDirection:@"right"] - (int)[self holdsDirection:@"left"];
    float y = (int)[self holdsDirection:@"up"] - (int)[self holdsDirection:@"down"];
    float length = fmaxf(1, hypotf(x, y));
    if (updateDirection(pad.leftThumbstick, x / length, y / length)) [changed addObject:pad.leftThumbstick];
    publishVirtualChanges(keyboardController, changed);
    if (debugControllerCalls && changed.count && atomic_fetch_add(&virtualInputLogs, 1) < 48) {
        PearHostLogMessage("input", [NSString stringWithFormat:@"Virtual input: buttons=%@ left stick=(%.2f,%.2f) dpad=(%d,%d) handler=%d",
            pressed, x / length, y / length, dpadX, dpadY, pad.valueChangedHandler != nil].UTF8String);
    }
}

- (BOOL)holdsDirection:(NSString *)direction {
    id key = self.stick[direction];
    return [key isKindOfClass:NSString.class] && [self.heldKeys containsObject:key];
}

- (void)releaseInput {
    [self.heldMouse removeAllObjects];
    [self.heldKeys removeAllObjects];
    [self updatePad];
    [self.lookTimer invalidate];
    updateVirtualLook(0, 0);
    restorePearHostPointer();
}

- (NSEvent *)handleEvent:(NSEvent *)event {
    if (!NSApp.isActive || !NSApp.keyWindow) return event;
    switch (event.type) {
        case NSEventTypeKeyDown: case NSEventTypeKeyUp: case NSEventTypeFlagsChanged: {
            NSString *key = eventKey(event);
            if (!key) return event;
            if ([key isEqualToString:@"m"]) {
                if (event.type == NSEventTypeKeyUp && self.captureShortcutDown) {
                    self.captureShortcutDown = NO;
                    return nil;
                }
                NSEventModifierFlags modifiers = event.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask;
                if (event.type == NSEventTypeKeyDown && modifiers == (NSEventModifierFlagControl | NSEventModifierFlagOption)) {
                    if (!self.captureShortcutDown && !event.isARepeat) {
                        if (screenMouseCaptured) [self releaseInput];
                        else if (CGAssociateMouseAndMouseCursorPosition(false) == kCGErrorSuccess) {
                            screenMouseCaptured = YES;
                            [NSCursor hide];
                        }
                    }
                    self.captureShortcutDown = YES;
                    return nil;
                }
            }
            if ([key isEqualToString:@"escape"] && screenMouseCaptured && event.type == NSEventTypeKeyDown) {
                [self releaseInput];
                return nil;
            }
            if (event.type == NSEventTypeFlagsChanged) {
                NSDictionary *modifiers = @{@"shift": @(NSEventModifierFlagShift), @"control": @(NSEventModifierFlagControl),
                    @"option": @(NSEventModifierFlagOption), @"command": @(NSEventModifierFlagCommand), @"capslock": @(NSEventModifierFlagCapsLock)};
                for (NSString *modifier in modifiers) {
                    if (event.modifierFlags & [modifiers[modifier] unsignedIntegerValue]) [self.heldKeys addObject:modifier];
                    else [self.heldKeys removeObject:modifier];
                }
            } else if (event.type == NSEventTypeKeyDown) [self.heldKeys addObject:key];
            else [self.heldKeys removeObject:key];
            [self updatePad];
            return self.keys[key] || [self.stick.allValues containsObject:key] ? nil : event;
        }
        case NSEventTypeLeftMouseDown: case NSEventTypeRightMouseDown: case NSEventTypeOtherMouseDown:
        case NSEventTypeLeftMouseUp: case NSEventTypeRightMouseUp: case NSEventTypeOtherMouseUp: {
            BOOL down = event.type == NSEventTypeLeftMouseDown || event.type == NSEventTypeRightMouseDown || event.type == NSEventTypeOtherMouseDown;
            if (down && event.window != NSApp.keyWindow) return event;
            NSString *button = [@(event.buttonNumber) stringValue];
            if (!self.mouse[button]) return event;
            if (down) [self.heldMouse addObject:button];
            else [self.heldMouse removeObject:button];
            [self updatePad];
            return screenMouseCaptured ? nil : event;
        }
        default: {
            if (!screenMouseCaptured) return event;
            float x = fmin(1, fmax(-1, event.deltaX * .045 * self.sensitivity));
            float y = fmin(1, fmax(-1, event.deltaY * .045 * self.sensitivity * (self.invertY ? 1 : -1)));
            updateVirtualLook(x, y);
            [self.lookTimer invalidate];
            self.lookTimer = [NSTimer scheduledTimerWithTimeInterval:.075 repeats:NO block:^(__unused NSTimer *timer) {
                updateVirtualLook(0, 0);
            }];
            return nil;
        }
    }
}
@end

__attribute__((constructor))
static void installPearHostControllerInput(void) {
    NSDictionary *environment = NSProcessInfo.processInfo.environment;
    debugControllerCalls = [environment[@"PEAR_DEBUG_GC"] isEqualToString:@"1"];
    BOOL native = [environment[@"PEAR_CONTROLLER_NATIVE"] isEqualToString:@"1"];
    controllerIdentityEnabled = !native && ![environment[@"PEAR_CONTROLLER_COMPAT"] isEqualToString:@"0"];
    replayEnabled = !native && ![environment[@"PEAR_CONTROLLER_REPLAY"] isEqualToString:@"0"];
    BOOL virtualPadEnabled = [environment[@"PEAR_VIRTUAL_GAMEPAD"] isEqualToString:@"1"];
    if (!controllerIdentityEnabled && !replayEnabled && !virtualPadEnabled) return;

    installControllerDiscovery();
    [NSNotificationCenter.defaultCenter addObserverForName:GCControllerDidConnectNotification object:nil
        queue:nil usingBlock:^(NSNotification *notification) {
            if (![notification.object isKindOfClass:GCController.class]) return;
            GCController *pad = notification.object;
            identifyController(pad);
            PearHostLogMessage("input", [NSString stringWithFormat:@"GameController connected: %@", pad.vendorName].UTF8String);
        }];
    if (replayEnabled) installConnectReplay();
    if (!native && (controllerIdentityEnabled || replayEnabled)) dispatch_async(dispatch_get_main_queue(), ^{
        startHIDFallback();
    });
    if (virtualPadEnabled) {
        // Retain a mutable extended profile and expose its real Cocoa input
        // elements to the guest even though macOS doesn't offer GCVirtualController.
        keyboardController = [GCController controllerWithExtendedGamepad];
        identifyController(keyboardController);
        if (debugControllerCalls && keyboardController) {
            Class type = object_getClass(keyboardController);
            Method method = class_getInstanceMethod(type, @selector(isSnapshot));
            originalSnapshot = method_getImplementation(method);
            class_replaceMethod(type, @selector(isSnapshot), (IMP)inspectSnapshot, method_getTypeEncoding(method));
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            GCExtendedGamepad *profile = keyboardController.extendedGamepad;
            if (!profile) return;
            installVirtualProfileHandler(profile);
            atexit(restorePearHostPointer);
            [[PearHostKeyboardPad new] start];
            atomic_store(&virtualAdvertised, true);
            [NSNotificationCenter.defaultCenter postNotificationName:GCControllerDidConnectNotification object:keyboardController];
            PearHostLogMessage("input", "Keyboard/mouse controller ready");
        });
    }
}
