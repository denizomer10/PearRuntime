// Internal interface between the UIKit layer and the host backend.
//
// The backend owns native windows, the screen, the run loop and raw input. The
// UIKit layer above it is platform neutral. backend/appkit implements this for
// macOS; a Linux backend (SDL/Wayland) implements the same functions.
#pragma once
#import "PearUIKit.h"

NS_ASSUME_NONNULL_BEGIN

typedef struct PearHostScreenInfo {
    CGSize pointSize;        // logical size apps see as UIScreen.bounds
    CGFloat scale;           // backing scale
    NSInteger maxFPS;
} PearHostScreenInfo;

// ---- backend -> implemented per platform ----------------------------------
void PearBackendInitialize(void);
PearHostScreenInfo PearBackendScreenInfo(void);
void *PearBackendCreateWindow(UIWindow *window, CGRect frame);
void PearBackendSetRootLayer(void *handle, CALayer *layer);
void PearBackendShowWindow(void *handle, BOOL key);
CADisplayLink *_Nullable PearBackendDisplayLink(id target, SEL selector);
void PearBackendInstallQuartzCoreCompat(void);
BOOL PearBackendOpenURL(NSURL *url);
void PearBackendRun(void (^didFinishLaunching)(void));
void PearBackendSetIdleTimerDisabled(BOOL disabled);
// Shows a native alert; reply receives the chosen button index (-1 if none).
void PearBackendPresentAlert(NSString *title, NSString *message, NSArray<NSString *> *buttons, void (^reply)(NSInteger index));

// ---- UIKit layer, called by the backend -------------------------------------
// Pointer input in window points (top-left origin).
void PearUIKitDeliverPointer(UIWindow *window, UITouchPhase phase, CGPoint point, NSTimeInterval timestamp);
void PearUIKitWindowDidResize(UIWindow *window, CGSize pointSize, CGFloat scale);
void PearUIKitApplicationDidBecomeActive(void);
void PearUIKitApplicationWillResignActive(void);
void PearUIKitApplicationDidEnterBackground(void);
void PearUIKitApplicationWillEnterForeground(void);
void PearUIKitApplicationWillTerminate(void);

// ---- shared state -----------------------------------------------------------
void PearUIKitRegisterWindow(UIWindow *window);
void PearUIKitSetKeyWindow(UIWindow *_Nullable window);
NSArray<UIWindow *> *PearUIKitWindows(void);
UIWindow *_Nullable PearUIKitKeyWindow(void);
void PearUIKitScreenDidChange(CGSize pointSize, CGFloat scale);

@interface UITouch ()
- (instancetype)initWithView:(UIView *)view window:(UIWindow *)window location:(CGPoint)p timestamp:(NSTimeInterval)t;
- (void)pear_moveTo:(CGPoint)p phase:(UITouchPhase)phase timestamp:(NSTimeInterval)t;
@end

@interface UIEvent ()
- (instancetype)initWithTouches:(NSSet<UITouch *> *)touches timestamp:(NSTimeInterval)t;
@end

@interface UITraitCollection ()
+ (UITraitCollection *)pear_current;
@end

@interface UIView ()
- (void)pear_setWindow:(nullable UIWindow *)window;
- (void)pear_setViewController:(nullable UIViewController *)vc;
@end

@interface UIAlertController ()
- (void)pear_presentWithCompletion:(nullable void (^)(void))completion;
@end

@interface UINib ()
- (NSArray *)pear_instantiateWithOwner:(nullable id)owner options:(nullable NSDictionary *)options
                        visibleWindows:(NSArray *_Nullable *_Nullable)visible;
@end

@interface UIViewController ()
- (void)pear_setPresenting:(nullable UIViewController *)vc;
@end

NS_ASSUME_NONNULL_END
