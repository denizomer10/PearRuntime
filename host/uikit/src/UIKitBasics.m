// UIColor, UITraitCollection, UIScreen, UIDevice, UIResponder, UITouch, UIEvent.
// Clean-room: behavior from public UIKit documentation and headers.
#import "PearUIKitPrivate.h"
#include <sys/sysctl.h>

// ---- UIColor -----------------------------------------------------------------

@implementation UIColor {
    CGColorRef _color;
}
+ (UIColor *)colorWithRed:(CGFloat)r green:(CGFloat)g blue:(CGFloat)b alpha:(CGFloat)a {
    UIColor *c = [self new];
    c->_color = CGColorCreateSRGB(r, g, b, a);
    return c;
}
+ (UIColor *)colorWithWhite:(CGFloat)w alpha:(CGFloat)a { return [self colorWithRed:w green:w blue:w alpha:a]; }
+ (UIColor *)clearColor { return [self colorWithWhite:0 alpha:0]; }
+ (UIColor *)blackColor { return [self colorWithWhite:0 alpha:1]; }
+ (UIColor *)whiteColor { return [self colorWithWhite:1 alpha:1]; }
- (instancetype)initWithRed:(CGFloat)r green:(CGFloat)g blue:(CGFloat)b alpha:(CGFloat)a {
    if ((self = [super init])) _color = CGColorCreateSRGB(r, g, b, a);
    return self;
}
- (CGColorRef)CGColor { return _color; }
- (id)copyWithZone:(NSZone *)zone { return self; }
- (void)dealloc { if (_color) CGColorRelease(_color); }
@end

// ---- UITraitCollection ---------------------------------------------------------

@implementation UITraitCollection
+ (UITraitCollection *)pear_current { return [self new]; }
+ (UITraitCollection *)currentTraitCollection { return [self pear_current]; }
- (UIUserInterfaceIdiom)userInterfaceIdiom { return UIDevice.currentDevice.userInterfaceIdiom; }
- (CGFloat)displayScale { return UIScreen.mainScreen.scale; }
- (UIUserInterfaceStyle)userInterfaceStyle { return UIUserInterfaceStyleDark; }
- (NSInteger)horizontalSizeClass { return 2; }  // regular
- (NSInteger)verticalSizeClass { return 2; }
- (NSInteger)forceTouchCapability { return 1; }  // unavailable
- (id)copyWithZone:(NSZone *)zone { return self; }
@end

// ---- UIScreen ------------------------------------------------------------------

static UIScreen *gMainScreen;

@implementation UIScreen {
    CGSize _size;
    CGFloat _scale;
    CGFloat _brightness;
}
+ (UIScreen *)mainScreen {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        gMainScreen = [UIScreen new];
        PearHostScreenInfo info = PearBackendScreenInfo();
        gMainScreen->_size = info.pointSize;
        gMainScreen->_scale = info.scale;
        gMainScreen->_brightness = 1.0;
    });
    return gMainScreen;
}
+ (NSArray<UIScreen *> *)screens { return @[ [self mainScreen] ]; }
- (CGRect)bounds { return CGRectMake(0, 0, _size.width, _size.height); }
- (CGRect)nativeBounds {
    // Documented as portrait-up, in pixels.
    CGFloat w = _size.width * _scale, h = _size.height * _scale;
    return CGRectMake(0, 0, MIN(w, h), MAX(w, h));
}
- (CGRect)applicationFrame { return self.bounds; }
- (CGFloat)scale { return _scale; }
- (CGFloat)nativeScale { return _scale; }
- (NSInteger)maximumFramesPerSecond { return PearBackendScreenInfo().maxFPS; }
- (CGFloat)brightness { return _brightness; }
- (void)setBrightness:(CGFloat)b { _brightness = b; }
- (NSInteger)displayGamut { return 1; }  // P3
- (UITraitCollection *)traitCollection { return [UITraitCollection pear_current]; }
- (id)coordinateSpace { return self; }
- (id)fixedCoordinateSpace { return self; }
- (UIScreen *)mirroredScreen { return nil; }
- (BOOL)isCaptured { return NO; }
- (UIEdgeInsets)overscanCompensationInsets { return (UIEdgeInsets){0, 0, 0, 0}; }
- (CADisplayLink *)displayLinkWithTarget:(id)target selector:(SEL)sel { return PearBackendDisplayLink(target, sel); }
- (void)pear_update:(CGSize)size scale:(CGFloat)scale {
    _size = size;
    _scale = scale;
}
@end

void PearUIKitScreenDidChange(CGSize pointSize, CGFloat scale) {
    [[UIScreen mainScreen] pear_update:pointSize scale:scale];
}

// ---- UIDevice ------------------------------------------------------------------

static NSString *sysctlString(const char *name) {
    char buf[256] = {0};
    size_t len = sizeof buf;
    if (sysctlbyname(name, buf, &len, NULL, 0) != 0) return nil;
    return [NSString stringWithUTF8String:buf];
}

@implementation UIDevice {
    BOOL _batteryMonitoring;
    NSUUID *_vendorID;
}
+ (UIDevice *)currentDevice {
    static UIDevice *device;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ device = [UIDevice new]; });
    return device;
}
- (NSString *)name { return @"iPad"; }  // iOS 16+ returns the generic model name to apps
- (NSString *)model { return @"iPad"; }
- (NSString *)localizedModel { return @"iPad"; }
- (NSString *)systemName { return @"iPadOS"; }
- (NSString *)systemVersion {
    const char *v = getenv("PEAR_HOST_IOS_VERSION");
    return v ? @(v) : @"17.0";
}
- (UIDeviceOrientation)orientation { return UIDeviceOrientationLandscapeLeft; }
- (BOOL)isGeneratingDeviceOrientationNotifications { return NO; }
- (void)beginGeneratingDeviceOrientationNotifications {}
- (void)endGeneratingDeviceOrientationNotifications {}
- (UIUserInterfaceIdiom)userInterfaceIdiom { return UIUserInterfaceIdiomPad; }
- (NSUUID *)identifierForVendor {
    if (!_vendorID) {
        // Stable per app and per Mac: derived from the hardware UUID and bundle id.
        NSString *seed = [NSString stringWithFormat:@"%@|%@", sysctlString("kern.uuid") ?: @"",
                                                    NSBundle.mainBundle.bundleIdentifier ?: @""];
        unsigned char bytes[16] = {0};
        NSData *d = [seed dataUsingEncoding:NSUTF8StringEncoding];
        const unsigned char *p = d.bytes;
        for (NSUInteger i = 0; i < d.length; i++) bytes[i % 16] = (unsigned char)(bytes[i % 16] * 31 + p[i]);
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        _vendorID = [[NSUUID alloc] initWithUUIDBytes:bytes];
    }
    return _vendorID;
}
- (BOOL)isBatteryMonitoringEnabled { return _batteryMonitoring; }
- (void)setBatteryMonitoringEnabled:(BOOL)e { _batteryMonitoring = e; }
- (UIDeviceBatteryState)batteryState {
    return _batteryMonitoring ? UIDeviceBatteryStateFull : UIDeviceBatteryStateUnknown;
}
- (float)batteryLevel { return _batteryMonitoring ? 1.0f : -1.0f; }
- (BOOL)isMultitaskingSupported { return YES; }
- (BOOL)isProximityMonitoringEnabled { return NO; }
- (void)setProximityMonitoringEnabled:(BOOL)e {}
- (BOOL)proximityState { return NO; }
- (void)playInputClick {}
// Read through key-value coding by apps that probe haptics support
// (observed in GTA VC: valueForKey:@"_feedbackSupportLevel"). 0 = no haptic
// engine, which is true for a Mac.
- (NSInteger)_feedbackSupportLevel { return 0; }
@end

// ---- UIResponder ---------------------------------------------------------------

static __weak UIResponder *gFirstResponder;
UIResponder *PearUIKitFirstResponder(void) { return gFirstResponder; }

@implementation UIResponder
- (UIResponder *)nextResponder { return nil; }
- (BOOL)canBecomeFirstResponder { return NO; }
- (BOOL)canResignFirstResponder { return YES; }
- (BOOL)isFirstResponder { return gFirstResponder == self; }
- (BOOL)becomeFirstResponder {
    if (gFirstResponder == self) return YES;
    if (![self canBecomeFirstResponder]) return NO;
    if (gFirstResponder && ![gFirstResponder resignFirstResponder]) return NO;
    gFirstResponder = self;
    return YES;
}
- (BOOL)resignFirstResponder {
    if (gFirstResponder == self) gFirstResponder = nil;
    return YES;
}
- (void)touchesBegan:(NSSet *)t withEvent:(UIEvent *)e { [self.nextResponder touchesBegan:t withEvent:e]; }
- (void)touchesMoved:(NSSet *)t withEvent:(UIEvent *)e { [self.nextResponder touchesMoved:t withEvent:e]; }
- (void)touchesEnded:(NSSet *)t withEvent:(UIEvent *)e { [self.nextResponder touchesEnded:t withEvent:e]; }
- (void)touchesCancelled:(NSSet *)t withEvent:(UIEvent *)e { [self.nextResponder touchesCancelled:t withEvent:e]; }
- (void)pressesBegan:(NSSet *)p withEvent:(id)e { [self.nextResponder pressesBegan:p withEvent:e]; }
- (void)pressesEnded:(NSSet *)p withEvent:(id)e { [self.nextResponder pressesEnded:p withEvent:e]; }
- (void)pressesChanged:(NSSet *)p withEvent:(id)e { [self.nextResponder pressesChanged:p withEvent:e]; }
- (void)pressesCancelled:(NSSet *)p withEvent:(id)e { [self.nextResponder pressesCancelled:p withEvent:e]; }
- (void)motionBegan:(NSInteger)m withEvent:(id)e {}
- (void)motionEnded:(NSInteger)m withEvent:(id)e {}
- (void)remoteControlReceivedWithEvent:(id)e {}
- (NSArray *)keyCommands { return nil; }
- (id)inputView { return nil; }
- (id)inputAccessoryView { return nil; }
- (id)undoManager { return nil; }
- (void)reloadInputViews {}
- (BOOL)canPerformAction:(SEL)action withSender:(id)sender { return [self respondsToSelector:action]; }
@end

// ---- UITouch / UIEvent ------------------------------------------------------------

@implementation UITouch {
    CGPoint _location, _previous;  // in window coordinates
    __weak UIView *_view;
    __weak UIWindow *_window;
    NSTimeInterval _timestamp;
    UITouchPhase _phase;
}
- (instancetype)initWithView:(UIView *)view window:(UIWindow *)window location:(CGPoint)p timestamp:(NSTimeInterval)t {
    if ((self = [super init])) {
        _view = view;
        _window = window;
        _location = _previous = p;
        _timestamp = t;
        _phase = UITouchPhaseBegan;
    }
    return self;
}
- (void)pear_moveTo:(CGPoint)p phase:(UITouchPhase)phase timestamp:(NSTimeInterval)t {
    _previous = _location;
    _location = p;
    _phase = phase;
    _timestamp = t;
}
- (NSTimeInterval)timestamp { return _timestamp; }
- (UITouchPhase)phase { return _phase; }
- (NSUInteger)tapCount { return 1; }
- (UITouchType)type { return UITouchTypeDirect; }
- (CGFloat)force { return _phase == UITouchPhaseEnded ? 0 : 1; }
- (CGFloat)maximumPossibleForce { return 1; }
- (CGFloat)majorRadius { return 20; }
- (CGFloat)majorRadiusTolerance { return 5; }
- (UIView *)view { return _view; }
- (UIWindow *)window { return _window; }
- (NSArray *)gestureRecognizers { return @[]; }
- (CGPoint)locationInView:(UIView *)view {
    return view ? [view convertPoint:_location fromView:_window] : _location;
}
- (CGPoint)previousLocationInView:(UIView *)view {
    return view ? [view convertPoint:_previous fromView:_window] : _previous;
}
- (CGPoint)preciseLocationInView:(UIView *)view { return [self locationInView:view]; }
- (CGPoint)precisePreviousLocationInView:(UIView *)view { return [self previousLocationInView:view]; }
@end

@implementation UIEvent {
    NSSet *_touches;
    NSTimeInterval _timestamp;
}
- (instancetype)initWithTouches:(NSSet *)touches timestamp:(NSTimeInterval)t {
    if ((self = [super init])) {
        _touches = touches;
        _timestamp = t;
    }
    return self;
}
- (UIEventType)type { return UIEventTypeTouches; }
- (NSInteger)subtype { return 0; }
- (NSTimeInterval)timestamp { return _timestamp; }
- (NSSet *)allTouches { return _touches; }
- (NSSet *)touchesForView:(UIView *)view {
    return [_touches objectsPassingTest:^BOOL(UITouch *t, BOOL *stop) { return t.view == view; }];
}
- (NSSet *)touchesForWindow:(UIWindow *)window {
    return [_touches objectsPassingTest:^BOOL(UITouch *t, BOOL *stop) { return t.window == window; }];
}
- (NSArray *)coalescedTouchesForTouch:(UITouch *)touch { return touch ? @[ touch ] : nil; }
- (NSArray *)predictedTouchesForTouch:(UITouch *)touch { return @[]; }
@end

// Single pointer -> single touch. Multi-touch synthesis lives in the input
// module above this layer.
static UITouch *gActiveTouch;

void PearUIKitDeliverPointer(UIWindow *window, UITouchPhase phase, CGPoint point, NSTimeInterval timestamp) {
    if (phase == UITouchPhaseBegan) {
        UIView *target = [window hitTest:point withEvent:nil] ?: window;
        gActiveTouch = [[UITouch alloc] initWithView:target window:window location:point timestamp:timestamp];
    } else if (!gActiveTouch) {
        return;
    } else {
        [gActiveTouch pear_moveTo:point phase:phase timestamp:timestamp];
    }
    UITouch *touch = gActiveTouch;
    NSSet *set = [NSSet setWithObject:touch];
    UIEvent *event = [[UIEvent alloc] initWithTouches:set timestamp:timestamp];
    UIView *v = touch.view;
    switch (phase) {
    case UITouchPhaseBegan: [v touchesBegan:set withEvent:event]; break;
    case UITouchPhaseMoved: [v touchesMoved:set withEvent:event]; break;
    case UITouchPhaseEnded: [v touchesEnded:set withEvent:event]; break;
    case UITouchPhaseCancelled: [v touchesCancelled:set withEvent:event]; break;
    default: break;
    }
    if (phase == UITouchPhaseEnded || phase == UITouchPhaseCancelled) gActiveTouch = nil;
}
