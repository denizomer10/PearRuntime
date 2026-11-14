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
// Starts PEAR_HOST_INPUT_SCRIPT, if set, once launch has finished.
void PearBackendStartInputScript(void);
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

// Keyboard: hardware keys become UIPress events for the first responder (or the
// key window's root view); while a text responder (UIKeyInput) is first
// responder, text and edit commands go to it instead.
typedef NS_ENUM(NSInteger, PearEditCommand) {
    PearEditDeleteBackward,
    PearEditNewline,
    PearEditMoveLeft,
    PearEditMoveRight,
    PearEditCancel,
};
void PearUIKitDeliverKey(NSInteger hidUsage, NSString *chars, NSString *charsNoMods, NSInteger modifiers, BOOL down,
                         NSTimeInterval timestamp);
BOOL PearUIKitTextInputActive(void);
void PearUIKitDeliverText(NSString *text);
void PearUIKitDeliverEditCommand(PearEditCommand command);
UIResponder *_Nullable PearUIKitFirstResponder(void);

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

@interface UIKey ()
- (instancetype)initWithCharacters:(NSString *)c ignoringModifiers:(NSString *)ci modifiers:(NSInteger)m keyCode:(NSInteger)k;
@end
@interface UIPress ()
- (instancetype)initWithKey:(UIKey *)key type:(NSInteger)type phase:(NSInteger)phase timestamp:(NSTimeInterval)t;
@end
@interface UIPressesEvent ()
- (instancetype)initWithPresses:(NSSet *)presses timestamp:(NSTimeInterval)t;
@end
@interface UITextView ()
- (void)pear_moveCursor:(NSInteger)delta;
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
