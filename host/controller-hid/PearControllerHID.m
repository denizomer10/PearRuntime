#import "PearControllerHID.h"
#import <IOKit/hid/IOHIDManager.h>
#import <IOKit/hid/IOHIDUsageTables.h>

@interface PearHIDGamepad ()
@property(nonatomic, readwrite, copy) NSString *name;
@property(nonatomic, readwrite, strong) GCController *controller;
@property(nonatomic) BOOL switchLayout;
@property(nonatomic) BOOL separateTriggerAxes;
@end

@implementation PearHIDGamepad
@end

@interface PearHIDGamepadMonitor ()
@property(nonatomic) IOHIDManagerRef manager;
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, PearHIDGamepad *> *devices;
- (void)addDevice:(IOHIDDeviceRef)device;
- (void)removeDevice:(IOHIDDeviceRef)device;
- (void)acceptValue:(IOHIDValueRef)value;
@end

static NSNumber *deviceID(IOHIDDeviceRef device) {
    uint64_t value;
    if (IORegistryEntryGetRegistryEntryID(IOHIDDeviceGetService(device), &value) != KERN_SUCCESS) return nil;
    return @(value);
}

static void deviceAdded(void *context, IOReturn result, void *sender, IOHIDDeviceRef device) {
    if (result == kIOReturnSuccess) [(__bridge PearHIDGamepadMonitor *)context addDevice:device];
}

static void deviceRemoved(void *context, IOReturn result, void *sender, IOHIDDeviceRef device) {
    [(__bridge PearHIDGamepadMonitor *)context removeDevice:device];
}

static void valueChanged(void *context, IOReturn result, void *sender, IOHIDValueRef value) {
    if (result == kIOReturnSuccess) [(__bridge PearHIDGamepadMonitor *)context acceptValue:value];
}

static float normalized(IOHIDElementRef element, CFIndex value) {
    double minimum = IOHIDElementGetLogicalMin(element);
    double maximum = IOHIDElementGetLogicalMax(element);
    if (maximum <= minimum) return 0;
    return fmax(0, fmin(1, ((double)value - minimum) / (maximum - minimum)));
}

static float stickValue(IOHIDElementRef element, CFIndex value) {
    float axis = 2 * normalized(element, value) - 1;
    const float deadZone = .12f;
    return copysignf(fmaxf(0, fabsf(axis) - deadZone) / (1 - deadZone), axis);
}

static GCControllerButtonInput *buttonForUsage(PearHIDGamepad *device, uint32_t usage) {
    GCExtendedGamepad *pad = device.controller.extendedGamepad;
    if (device.switchLayout) {
        switch (usage) {
            case 5: return pad.buttonOptions;
            case 6: return pad.buttonHome;
            case 7: return pad.buttonMenu;
            case 8: return pad.leftThumbstickButton;
            case 9: return pad.rightThumbstickButton;
            case 10: return pad.leftShoulder;
            case 11: return pad.rightShoulder;
        }
    }
    switch (usage) {
        case 1: return pad.buttonA;
        case 2: return pad.buttonB;
        case 3: return pad.buttonX;
        case 4: return pad.buttonY;
    }
    if (device.switchLayout) return nil;
    switch (usage) {
        case 5: return pad.leftShoulder;
        case 6: return pad.rightShoulder;
        case 7: return pad.leftTrigger;
        case 8: return pad.rightTrigger;
        case 9: return pad.buttonOptions;
        case 10: return pad.buttonMenu;
        case 11: return pad.leftThumbstickButton;
        case 12: return pad.rightThumbstickButton;
        case 13: return pad.buttonHome;
        default: return nil;
    }
}

static GCControllerElement *setButton(GCControllerButtonInput *button, float amount) {
    if (!button || button.value == amount) return nil;
    [button setValue:amount];
    return button;
}

static GCControllerElement *setDirection(GCControllerDirectionPad *direction, float x, float y) {
    if (direction.xAxis.value == x && direction.yAxis.value == y) return nil;
    [direction setValueForXAxis:x yAxis:y];
    return direction;
}

static GCControllerElement *updateDesktop(PearHIDGamepad *device, IOHIDElementRef element, CFIndex value) {
    GCExtendedGamepad *pad = device.controller.extendedGamepad;
    uint32_t usage = IOHIDElementGetUsage(element);
    if (usage == kHIDUsage_GD_Hatswitch) {
        static const int x[] = {0, 1, 1, 1, 0, -1, -1, -1};
        static const int y[] = {1, 1, 0, -1, -1, -1, 0, 1};
        CFIndex position = value - IOHIDElementGetLogicalMin(element);
        return position >= 0 && position < 8 && value <= IOHIDElementGetLogicalMax(element)
            ? setDirection(pad.dpad, x[position], y[position]) : setDirection(pad.dpad, 0, 0);
    }
    if (usage >= kHIDUsage_GD_DPadUp && usage <= kHIDUsage_GD_DPadLeft) {
        GCControllerButtonInput *button = @{ @(kHIDUsage_GD_DPadUp): pad.dpad.up, @(kHIDUsage_GD_DPadDown): pad.dpad.down,
            @(kHIDUsage_GD_DPadRight): pad.dpad.right, @(kHIDUsage_GD_DPadLeft): pad.dpad.left }[@(usage)];
        return setButton(button, value != 0);
    }

    float axis = stickValue(element, value);
    switch (usage) {
        case kHIDUsage_GD_X: return setDirection(pad.leftThumbstick, axis, pad.leftThumbstick.yAxis.value);
        case kHIDUsage_GD_Y: return setDirection(pad.leftThumbstick, pad.leftThumbstick.xAxis.value, -axis);
        case kHIDUsage_GD_Z:
            if (device.separateTriggerAxes) return setDirection(pad.rightThumbstick, axis, pad.rightThumbstick.yAxis.value);
            return setButton(pad.leftTrigger, normalized(element, value));
        case kHIDUsage_GD_Rx:
            if (device.separateTriggerAxes) return setButton(pad.leftTrigger, normalized(element, value));
            return setDirection(pad.rightThumbstick, axis, pad.rightThumbstick.yAxis.value);
        case kHIDUsage_GD_Ry:
            if (device.separateTriggerAxes) return setButton(pad.rightTrigger, normalized(element, value));
            return setDirection(pad.rightThumbstick, pad.rightThumbstick.xAxis.value, -axis);
        case kHIDUsage_GD_Rz:
            if (device.separateTriggerAxes) return setDirection(pad.rightThumbstick, pad.rightThumbstick.xAxis.value, -axis);
            return setButton(pad.rightTrigger, normalized(element, value));
        default: return nil;
    }
}

@implementation PearHIDGamepadMonitor

- (BOOL)start {
    if (self.manager) return YES;
    self.devices = [NSMutableDictionary dictionary];
    IOHIDManagerRef manager = IOHIDManagerCreate(kCFAllocatorDefault, kIOHIDOptionsTypeNone);
    if (!manager) return NO;
    self.manager = manager;
    IOHIDManagerSetDeviceMatchingMultiple(manager, (__bridge CFArrayRef)@[
        @{@kIOHIDDeviceUsagePageKey: @(kHIDPage_GenericDesktop), @kIOHIDDeviceUsageKey: @(kHIDUsage_GD_GamePad)},
        @{@kIOHIDDeviceUsagePageKey: @(kHIDPage_GenericDesktop), @kIOHIDDeviceUsageKey: @(kHIDUsage_GD_Joystick)}
    ]);
    IOHIDManagerRegisterDeviceMatchingCallback(manager, deviceAdded, (__bridge void *)self);
    IOHIDManagerRegisterDeviceRemovalCallback(manager, deviceRemoved, (__bridge void *)self);
    IOHIDManagerRegisterInputValueCallback(manager, valueChanged, (__bridge void *)self);
    IOHIDManagerScheduleWithRunLoop(manager, CFRunLoopGetMain(), kCFRunLoopCommonModes);
    if (IOHIDManagerOpen(manager, kIOHIDOptionsTypeNone) == kIOReturnSuccess) return YES;
    [self close];
    return NO;
}

- (void)close {
    if (!_manager) return;
    IOHIDManagerUnscheduleFromRunLoop(_manager, CFRunLoopGetMain(), kCFRunLoopCommonModes);
    IOHIDManagerClose(_manager, kIOHIDOptionsTypeNone);
    CFRelease(_manager);
    _manager = NULL;
}

- (void)dealloc { [self close]; }

- (NSArray<PearHIDGamepad *> *)gamepads { return self.devices.allValues; }

- (void)addDevice:(IOHIDDeviceRef)device {
    NSNumber *identifier = deviceID(device);
    if (!identifier || self.devices[identifier] || [GCController supportsHIDDevice:device] ||
        [(__bridge NSNumber *)IOHIDDeviceGetProperty(device, CFSTR(kIOHIDGCSyntheticDeviceKey)) boolValue]) return;
    NSString *name = (__bridge NSString *)IOHIDDeviceGetProperty(device, CFSTR(kIOHIDProductKey));
    PearHIDGamepad *gamepad = [PearHIDGamepad new];
    gamepad.name = name.length ? name : @"HID Gamepad";
    gamepad.controller = [GCController controllerWithExtendedGamepad];
    if (!gamepad.controller) return;
    NSNumber *vendor = (__bridge NSNumber *)IOHIDDeviceGetProperty(device, CFSTR(kIOHIDVendorIDKey));
    gamepad.switchLayout = vendor.unsignedIntValue == 0x057e && [name hasSuffix:@" (Finally)"];

    // Z/Rz signed axes with unsigned Rx/Ry triggers are used by the Nintendo
    // bridge. A descriptor with X/Y + Z/Rz and no Rx/Ry is also common.
    NSArray *elements = CFBridgingRelease(IOHIDDeviceCopyMatchingElements(device,
        (__bridge CFDictionaryRef)@{@kIOHIDElementUsagePageKey: @(kHIDPage_GenericDesktop)}, kIOHIDOptionsTypeNone));
    BOOL hasRx = NO, hasSignedZ = NO;
    for (id matching in elements) {
        IOHIDElementRef element = (__bridge IOHIDElementRef)matching;
        if (IOHIDElementGetType(element) != kIOHIDElementTypeInput_Misc) continue;
        uint32_t usage = IOHIDElementGetUsage(element);
        if (usage == kHIDUsage_GD_Rx && IOHIDElementGetLogicalMin(element) < 0) hasRx = YES;
        if (usage == kHIDUsage_GD_Z && IOHIDElementGetLogicalMin(element) < 0) hasSignedZ = YES;
    }
    gamepad.separateTriggerAxes = gamepad.switchLayout || (hasSignedZ && !hasRx);
    self.devices[identifier] = gamepad;
    if (self.connectionChanged) self.connectionChanged(gamepad, YES);
}

- (void)removeDevice:(IOHIDDeviceRef)device {
    NSNumber *identifier = deviceID(device);
    PearHIDGamepad *gamepad = identifier ? self.devices[identifier] : nil;
    if (!gamepad) return;
    [self.devices removeObjectForKey:identifier];
    if (self.connectionChanged) self.connectionChanged(gamepad, NO);
}

- (void)acceptValue:(IOHIDValueRef)value {
    IOHIDElementRef element = IOHIDValueGetElement(value);
    NSNumber *identifier = deviceID(IOHIDElementGetDevice(element));
    PearHIDGamepad *gamepad = identifier ? self.devices[identifier] : nil;
    if (!gamepad || IOHIDElementIsRelative(element)) return;
    GCControllerElement *changed = nil;
    uint32_t page = IOHIDElementGetUsagePage(element);
    if (page == kHIDPage_Button) {
        changed = setButton(buttonForUsage(gamepad, IOHIDElementGetUsage(element)),
            IOHIDValueGetIntegerValue(value) != 0);
    } else if (page == kHIDPage_GenericDesktop) {
        changed = updateDesktop(gamepad, element, IOHIDValueGetIntegerValue(value));
    }
    if (changed && self.inputChanged) self.inputChanged(gamepad, changed);
}

@end
