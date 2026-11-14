// PearHost UIKit: an independent implementation of the UIKit interface that
// iOS apps link against, hosted on AppKit (macOS) today and on a portable
// backend later (Linux). Declarations here are written from the public UIKit
// interface: names, selectors, enum values and struct layouts are ABI facts
// the app binary depends on. No Apple implementation is used or copied.
#pragma once

#import <Foundation/Foundation.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreGraphics/CoreGraphics.h>

#include "../../core/PearHost.h"

#define PEAR_UIKIT_EXPORT __attribute__((visibility("default")))

NS_ASSUME_NONNULL_BEGIN

// ---- ABI value types (layouts and raw values fixed by the iOS ABI) ----------

typedef struct UIEdgeInsets { CGFloat top, left, bottom, right; } UIEdgeInsets;

typedef NS_ENUM(NSInteger, UIUserInterfaceIdiom) {
    UIUserInterfaceIdiomUnspecified = -1,
    UIUserInterfaceIdiomPhone = 0,
    UIUserInterfaceIdiomPad = 1,
    UIUserInterfaceIdiomTV = 2,
    UIUserInterfaceIdiomCarPlay = 3,
    UIUserInterfaceIdiomMac = 5,
};

typedef NS_ENUM(NSInteger, UIInterfaceOrientation) {
    UIInterfaceOrientationUnknown = 0,
    UIInterfaceOrientationPortrait = 1,
    UIInterfaceOrientationPortraitUpsideDown = 2,
    UIInterfaceOrientationLandscapeLeft = 4,
    UIInterfaceOrientationLandscapeRight = 3,
};

typedef NS_ENUM(NSInteger, UIDeviceOrientation) {
    UIDeviceOrientationUnknown = 0,
    UIDeviceOrientationPortrait = 1,
    UIDeviceOrientationPortraitUpsideDown = 2,
    UIDeviceOrientationLandscapeLeft = 3,
    UIDeviceOrientationLandscapeRight = 4,
    UIDeviceOrientationFaceUp = 5,
    UIDeviceOrientationFaceDown = 6,
};

typedef NS_OPTIONS(NSUInteger, UIInterfaceOrientationMask) {
    UIInterfaceOrientationMaskPortrait = 1 << 1,
    UIInterfaceOrientationMaskLandscapeLeft = 1 << 4,
    UIInterfaceOrientationMaskLandscapeRight = 1 << 3,
    UIInterfaceOrientationMaskPortraitUpsideDown = 1 << 2,
    UIInterfaceOrientationMaskLandscape = (1 << 4) | (1 << 3),
    UIInterfaceOrientationMaskAll = 30,
    UIInterfaceOrientationMaskAllButUpsideDown = 26,
};

typedef NS_ENUM(NSInteger, UIApplicationState) {
    UIApplicationStateActive = 0,
    UIApplicationStateInactive = 1,
    UIApplicationStateBackground = 2,
};

typedef NS_ENUM(NSInteger, UIDeviceBatteryState) {
    UIDeviceBatteryStateUnknown = 0,
    UIDeviceBatteryStateUnplugged = 1,
    UIDeviceBatteryStateCharging = 2,
    UIDeviceBatteryStateFull = 3,
};

typedef NS_ENUM(NSInteger, UITouchPhase) {
    UITouchPhaseBegan = 0,
    UITouchPhaseMoved = 1,
    UITouchPhaseStationary = 2,
    UITouchPhaseEnded = 3,
    UITouchPhaseCancelled = 4,
};

typedef NS_ENUM(NSInteger, UITouchType) {
    UITouchTypeDirect = 0,
    UITouchTypeIndirect = 1,
    UITouchTypePencil = 2,
    UITouchTypeIndirectPointer = 3,
};

typedef NS_ENUM(NSInteger, UIEventType) {
    UIEventTypeTouches = 0,
    UIEventTypeMotion = 1,
    UIEventTypeRemoteControl = 2,
    UIEventTypePresses = 3,
    UIEventTypeScroll = 10,
    UIEventTypeHover = 11,
    UIEventTypeTransform = 14,
};

typedef NS_ENUM(NSInteger, UIUserInterfaceStyle) {
    UIUserInterfaceStyleUnspecified = 0,
    UIUserInterfaceStyleLight = 1,
    UIUserInterfaceStyleDark = 2,
};

typedef NS_OPTIONS(NSUInteger, UIViewAutoresizing) {
    UIViewAutoresizingNone = 0,
    UIViewAutoresizingFlexibleLeftMargin = 1 << 0,
    UIViewAutoresizingFlexibleWidth = 1 << 1,
    UIViewAutoresizingFlexibleRightMargin = 1 << 2,
    UIViewAutoresizingFlexibleTopMargin = 1 << 3,
    UIViewAutoresizingFlexibleHeight = 1 << 4,
    UIViewAutoresizingFlexibleBottomMargin = 1 << 5,
};

typedef NS_ENUM(NSInteger, UIModalPresentationStyle) {
    UIModalPresentationFullScreen = 0,
    UIModalPresentationAutomatic = -2,
};

typedef NS_OPTIONS(NSUInteger, UIRectEdge) {
    UIRectEdgeNone = 0,
    UIRectEdgeAll = 15,
};

// ---- Classes -----------------------------------------------------------------
// Every UIKit class is part of the ABI the app binds to: export them all.
#pragma GCC visibility push(default)

@class UIView, UIWindow, UIViewController, UIScreen, UIEvent, UITouch, UITraitCollection;

@interface UIColor : NSObject
+ (UIColor *)colorWithRed:(CGFloat)r green:(CGFloat)g blue:(CGFloat)b alpha:(CGFloat)a;
+ (UIColor *)colorWithWhite:(CGFloat)w alpha:(CGFloat)a;
+ (UIColor *)clearColor;
+ (UIColor *)blackColor;
+ (UIColor *)whiteColor;
@property (nonatomic, readonly) CGColorRef CGColor;
@end

@interface UITraitCollection : NSObject <NSCopying>
@property (nonatomic, readonly) UIUserInterfaceIdiom userInterfaceIdiom;
@property (nonatomic, readonly) CGFloat displayScale;
@property (nonatomic, readonly) UIUserInterfaceStyle userInterfaceStyle;
@property (nonatomic, readonly) NSInteger horizontalSizeClass;
@property (nonatomic, readonly) NSInteger verticalSizeClass;
@property (nonatomic, readonly) NSInteger forceTouchCapability;
@end

@interface UIResponder : NSObject
@property (nonatomic, readonly, nullable) UIResponder *nextResponder;
@property (nonatomic, readonly) BOOL canBecomeFirstResponder;
@property (nonatomic, readonly) BOOL canResignFirstResponder;
@property (nonatomic, readonly) BOOL isFirstResponder;
- (BOOL)becomeFirstResponder;
- (BOOL)resignFirstResponder;
- (void)touchesBegan:(NSSet<UITouch *> *)touches withEvent:(nullable UIEvent *)event;
- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(nullable UIEvent *)event;
- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(nullable UIEvent *)event;
- (void)touchesCancelled:(NSSet<UITouch *> *)touches withEvent:(nullable UIEvent *)event;
@end

@interface UIScreen : NSObject
+ (UIScreen *)mainScreen;
+ (NSArray<UIScreen *> *)screens;
@property (nonatomic, readonly) CGRect bounds;
@property (nonatomic, readonly) CGRect nativeBounds;
@property (nonatomic, readonly) CGFloat scale;
@property (nonatomic, readonly) CGFloat nativeScale;
@property (nonatomic, readonly) NSInteger maximumFramesPerSecond;
@property (nonatomic) CGFloat brightness;
@property (nonatomic, readonly) UITraitCollection *traitCollection;
- (nullable CADisplayLink *)displayLinkWithTarget:(id)target selector:(SEL)sel;
@end

@interface UIDevice : NSObject
+ (UIDevice *)currentDevice;
@property (nonatomic, readonly) NSString *name;
@property (nonatomic, readonly) NSString *model;
@property (nonatomic, readonly) NSString *localizedModel;
@property (nonatomic, readonly) NSString *systemName;
@property (nonatomic, readonly) NSString *systemVersion;
@property (nonatomic, readonly) UIDeviceOrientation orientation;
@property (nonatomic, readonly, nullable) NSUUID *identifierForVendor;
@property (nonatomic, readonly) UIUserInterfaceIdiom userInterfaceIdiom;
@property (nonatomic, getter=isBatteryMonitoringEnabled) BOOL batteryMonitoringEnabled;
@property (nonatomic, readonly) UIDeviceBatteryState batteryState;
@property (nonatomic, readonly) float batteryLevel;
@property (nonatomic, readonly, getter=isMultitaskingSupported) BOOL multitaskingSupported;
- (void)beginGeneratingDeviceOrientationNotifications;
- (void)endGeneratingDeviceOrientationNotifications;
@end

@interface UIView : UIResponder
+ (Class)layerClass;
- (instancetype)initWithFrame:(CGRect)frame;
- (nullable instancetype)initWithCoder:(NSCoder *)coder;
@property (nonatomic, readonly) CALayer *layer;
@property (nonatomic) CGRect frame;
@property (nonatomic) CGRect bounds;
@property (nonatomic) CGPoint center;
@property (nonatomic, readonly, nullable) UIView *superview;
@property (nonatomic, readonly) NSArray<UIView *> *subviews;
@property (nonatomic, readonly, nullable) UIWindow *window;
@property (nonatomic) CGFloat contentScaleFactor;
@property (nonatomic, getter=isMultipleTouchEnabled) BOOL multipleTouchEnabled;
@property (nonatomic, getter=isUserInteractionEnabled) BOOL userInteractionEnabled;
@property (nonatomic, getter=isHidden) BOOL hidden;
@property (nonatomic, getter=isOpaque) BOOL opaque;
@property (nonatomic) CGFloat alpha;
@property (nonatomic) NSInteger tag;
@property (nonatomic) UIViewAutoresizing autoresizingMask;
@property (nonatomic) BOOL clearsContextBeforeDrawing;
@property (nonatomic, nullable, copy) UIColor *backgroundColor;
@property (nonatomic, readonly) UIEdgeInsets safeAreaInsets;
@property (nonatomic, readonly) UITraitCollection *traitCollection;
- (void)addSubview:(UIView *)view;
- (void)insertSubview:(UIView *)view atIndex:(NSInteger)index;
- (void)removeFromSuperview;
- (void)bringSubviewToFront:(UIView *)view;
- (nullable UIView *)viewWithTag:(NSInteger)tag;
- (void)setNeedsLayout;
- (void)layoutIfNeeded;
- (void)layoutSubviews;
- (void)setNeedsDisplay;
- (void)didMoveToWindow;
- (void)didMoveToSuperview;
- (void)willMoveToWindow:(nullable UIWindow *)newWindow;
- (void)willMoveToSuperview:(nullable UIView *)newSuperview;
- (nullable UIView *)hitTest:(CGPoint)point withEvent:(nullable UIEvent *)event;
- (BOOL)pointInside:(CGPoint)point withEvent:(nullable UIEvent *)event;
- (CGPoint)convertPoint:(CGPoint)point toView:(nullable UIView *)view;
- (CGPoint)convertPoint:(CGPoint)point fromView:(nullable UIView *)view;
- (CGSize)sizeThatFits:(CGSize)size;
- (void)sizeToFit;
@end

@interface UIViewController : UIResponder
- (instancetype)initWithNibName:(nullable NSString *)nibName bundle:(nullable NSBundle *)bundle;
@property (nonatomic, null_resettable, strong) UIView *view;
@property (nonatomic, readonly, nullable) UIView *viewIfLoaded;
@property (nonatomic, readonly, getter=isViewLoaded) BOOL viewLoaded;
@property (nonatomic, nullable, copy) NSString *title;
@property (nonatomic) UIModalPresentationStyle modalPresentationStyle;
@property (nonatomic) UIRectEdge edgesForExtendedLayout;
@property (nonatomic, readonly, nullable) UIViewController *presentedViewController;
@property (nonatomic, readonly, nullable) UIViewController *presentingViewController;
@property (nonatomic, readonly) UITraitCollection *traitCollection;
- (void)loadView;
- (void)viewDidLoad;
- (void)viewWillAppear:(BOOL)animated;
- (void)viewDidAppear:(BOOL)animated;
- (void)viewWillDisappear:(BOOL)animated;
- (void)viewDidDisappear:(BOOL)animated;
- (void)viewWillLayoutSubviews;
- (void)viewDidLayoutSubviews;
- (void)presentViewController:(UIViewController *)vc animated:(BOOL)animated completion:(void (^__nullable)(void))completion;
- (void)dismissViewControllerAnimated:(BOOL)animated completion:(void (^__nullable)(void))completion;
- (BOOL)prefersStatusBarHidden;
- (BOOL)prefersHomeIndicatorAutoHidden;
- (BOOL)prefersPointerLocked;
- (BOOL)shouldAutorotate;
- (UIInterfaceOrientationMask)supportedInterfaceOrientations;
- (UIRectEdge)preferredScreenEdgesDeferringSystemGestures;
- (void)setNeedsUpdateOfPrefersPointerLocked;
- (void)setNeedsUpdateOfHomeIndicatorAutoHidden;
- (void)setNeedsUpdateOfScreenEdgesDeferringSystemGestures;
- (void)setNeedsStatusBarAppearanceUpdate;
@end

@interface UIWindow : UIView
@property (nonatomic, nullable, strong) UIViewController *rootViewController;
@property (nonatomic, strong) UIScreen *screen;
@property (nonatomic, readonly, getter=isKeyWindow) BOOL keyWindow;
@property (nonatomic) CGFloat windowLevel;
- (void)makeKeyAndVisible;
- (void)makeKeyWindow;
@end

@interface UIAlertAction : NSObject <NSCopying>
+ (instancetype)actionWithTitle:(nullable NSString *)title style:(NSInteger)style handler:(void (^__nullable)(UIAlertAction *action))handler;
@property (nonatomic, readonly, nullable) NSString *title;
@property (nonatomic, readonly) NSInteger style;
@property (nonatomic, getter=isEnabled) BOOL enabled;
@end

@interface UIAlertController : UIViewController
+ (instancetype)alertControllerWithTitle:(nullable NSString *)title message:(nullable NSString *)message preferredStyle:(NSInteger)style;
@property (nonatomic, copy, nullable) NSString *message;
@property (nonatomic, readonly) NSInteger preferredStyle;
@property (nonatomic, readonly) NSArray<UIAlertAction *> *actions;
@property (nonatomic, strong, nullable) UIAlertAction *preferredAction;
- (void)addAction:(UIAlertAction *)action;
@end

@interface UIApplication : UIResponder
+ (UIApplication *)sharedApplication;
@property (nonatomic, nullable, weak) id delegate;
@property (nonatomic, readonly, nullable) UIWindow *keyWindow;
@property (nonatomic, readonly) NSArray<UIWindow *> *windows;
@property (nonatomic, readonly) UIApplicationState applicationState;
@property (nonatomic, getter=isIdleTimerDisabled) BOOL idleTimerDisabled;
@property (nonatomic, readonly) UIInterfaceOrientation statusBarOrientation;
@property (nonatomic) NSInteger applicationIconBadgeNumber;
@property (nonatomic, readonly) NSTimeInterval backgroundTimeRemaining;
@property (nonatomic, readonly) BOOL isProtectedDataAvailable;
- (BOOL)openURL:(NSURL *)url;
- (BOOL)canOpenURL:(NSURL *)url;
- (void)openURL:(NSURL *)url options:(NSDictionary *)options completionHandler:(void (^__nullable)(BOOL success))completion;
- (void)registerForRemoteNotifications;
- (void)unregisterForRemoteNotifications;
- (NSUInteger)beginBackgroundTaskWithExpirationHandler:(void (^__nullable)(void))handler;
- (NSUInteger)beginBackgroundTaskWithName:(nullable NSString *)name expirationHandler:(void (^__nullable)(void))handler;
- (void)endBackgroundTask:(NSUInteger)identifier;
- (BOOL)sendAction:(SEL)action to:(nullable id)target from:(nullable id)sender forEvent:(nullable UIEvent *)event;
@end

@interface UITouch : NSObject
@property (nonatomic, readonly) NSTimeInterval timestamp;
@property (nonatomic, readonly) UITouchPhase phase;
@property (nonatomic, readonly) NSUInteger tapCount;
@property (nonatomic, readonly) UITouchType type;
@property (nonatomic, readonly) CGFloat force;
@property (nonatomic, readonly) CGFloat maximumPossibleForce;
@property (nonatomic, readonly, nullable) UIView *view;
@property (nonatomic, readonly, nullable) UIWindow *window;
- (CGPoint)locationInView:(nullable UIView *)view;
- (CGPoint)previousLocationInView:(nullable UIView *)view;
@end

@interface UIEvent : NSObject
@property (nonatomic, readonly) UIEventType type;
@property (nonatomic, readonly) NSTimeInterval timestamp;
- (nullable NSSet<UITouch *> *)allTouches;
@end

#pragma GCC visibility pop

// UIApplicationMain and the C entry points
PEAR_UIKIT_EXPORT int UIApplicationMain(int argc, char *_Nullable argv[_Nonnull], NSString *_Nullable principalClassName,
                                        NSString *_Nullable delegateClassName);
PEAR_UIKIT_EXPORT NSData *_Nullable UIImagePNGRepresentation(id _Nullable image);

NS_ASSUME_NONNULL_END
