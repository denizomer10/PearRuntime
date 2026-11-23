#import <Foundation/Foundation.h>
#import <GameController/GameController.h>

NS_ASSUME_NONNULL_BEGIN

/// Reads HID gamepads that macOS does not expose through GameController.
@interface PearHIDGamepad : NSObject
@property(nonatomic, readonly) NSString *name;
@property(nonatomic, readonly) GCController *controller;
@end

/// Create, start, and release on the main thread. All callbacks run there.
@interface PearHIDGamepadMonitor : NSObject
@property(nonatomic, readonly, copy) NSArray<PearHIDGamepad *> *gamepads;
@property(nonatomic, copy, nullable) void (^connectionChanged)(PearHIDGamepad *, BOOL connected);
@property(nonatomic, copy, nullable) void (^inputChanged)(PearHIDGamepad *, GCControllerElement *);
- (BOOL)start;
@end

NS_ASSUME_NONNULL_END
