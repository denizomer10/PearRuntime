// ScreenShim.mm
//
// Makes `UIScreen.main` describe the app's *window* rather than the whole Mac
// display.
//
// An iOS app windowed on macOS still sees the full display through
// `UIScreen.main`: on a 2560×1440 screen `bounds` reports 2560×1440 while the
// actual window is, say, 1816×1210. Games that size their framebuffer and
// viewport from `UIScreen` then render a 2560×1440 image into a smaller window,
// so the content overflows and looks oversized.
//
// This replaces `UIScreen`'s geometry accessors so they follow the key window
// live — which also means the values track window resizes. Disable with
// PEAR_NO_SCREEN_SHIM.
//
// Render scale: `scale`/`nativeScale` report the display's real backing scale
// (2.0 on a Retina Mac) multiplied by PEAR_RENDER_SCALE, and `nativeBounds` is
// the window size in pixels at that scale. Earlier builds reported 1.0, so
// every engine that sizes its render target as `bounds * scale` rendered a
// quarter of the window's pixels and the compositor upscaled it (blurry, low
// internal resolution). PEAR_RENDER_SCALE accepts:
//   native (default)  the display's backing scale
//   0.25 … 1          that fraction of native (performance), never below 1x
//   points            1.0, the old behaviour
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import <stdatomic.h>
#import <os/lock.h>
#import <time.h>
#import "PearRuntime.h"

/// Set by the deprecated `UIViewController.interfaceOrientation` override when
/// an app actually reads it. Only those apps cache the launch-time orientation
/// and need the one-shot notification nudge below.
static _Atomic BOOL gInterfaceOrientationRead = NO;

static UIWindow *PearMainWindow(void) {
    return PearPrimaryWindow();
}

/// A legacy GL surface can be larger than its window. When the GL shim fits it,
/// it publishes that logical size here so UIScreen and the view agree; touch
/// hit-testing then uses the same coordinate space as the rendered frame. Once
/// a fixed drawable exists its size is authoritative: a Mac window resize (full
/// screen in particular) must not rotate or rescale the coordinate space the
/// engine laid its controls out in.
static CGSize gReportedOverride = {0, 0};

void PearSetScreenOverride(CGSize size) {
    gReportedOverride = size;
}

void PearClearScreenOverride(void) {
    gReportedOverride = CGSizeZero;
}

/// The screen geometry to report: the window's bounds, with the display's scale.
static CGRect PearReportedBounds(void) {
    if (gReportedOverride.width > 0 && gReportedOverride.height > 0) {
        return CGRectMake(0, 0, gReportedOverride.width, gReportedOverride.height);
    }

    // Engines query UIScreen every frame, often from their render thread.
    // Walking the scene/window tree each time costs main-thread time and is
    // not thread-safe, so the window size is cached briefly: the main thread
    // refreshes it at most every 50 ms (fast enough to follow a live resize),
    // other threads read the last main-thread value.
    static os_unfair_lock cacheLock = OS_UNFAIR_LOCK_INIT;
    static CGSize cachedSize = {0, 0};
    static uint64_t cachedAt = 0;
    const uint64_t now = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
    const BOOL main = [NSThread isMainThread];
    os_unfair_lock_lock(&cacheLock);
    CGSize cached = cachedSize;
    uint64_t age = now - cachedAt;
    os_unfair_lock_unlock(&cacheLock);
    if (cached.width > 0 && cached.height > 0 && (!main || age < 50 * NSEC_PER_MSEC)) {
        return CGRectMake(0, 0, cached.width, cached.height);
    }

    UIWindow *window = PearMainWindow();
    if (window && window.bounds.size.width > 0 && window.bounds.size.height > 0) {
        CGRect bounds = window.bounds;
        if (main) {
            os_unfair_lock_lock(&cacheLock);
            cachedSize = bounds.size;
            cachedAt = now;
            os_unfair_lock_unlock(&cacheLock);
        }
        return bounds;
    }

    // Before the first scene/window is connected, UIKit can report an empty
    // screen. Engines that build a Metal render target from that size create a
    // zero-width/zero-height texture and abort. Report a conservative
    // landscape surface until the real window is available; the value follows
    // the window immediately afterwards.
    static BOOL loggedFallback = NO;
    if (!loggedFallback) {
        loggedFallback = YES;
        PearLog(@"[PearRuntime][screen] no window yet; reporting 1024x768 fallback");
    }
    return CGRectMake(0, 0, 1024, 768);
}

/// Display backing scale (as UIKit reports it for the window), cached so
/// render threads never touch UIKit; refreshed on the main thread and when the
/// window moves between displays.
static IMP gOriginalScaleIMP = NULL;
static _Atomic double gDisplayScale = 0;

/// Multiplier from PEAR_RENDER_SCALE, parsed once. 0 means "points" (1.0).
static double PearRenderScaleFactor(void) {
    static double factor = 1.0;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        const char *value = getenv("PEAR_RENDER_SCALE");
        if (!value || !*value || strcasecmp(value, "native") == 0) return;
        if (strcasecmp(value, "points") == 0 || strcasecmp(value, "legacy") == 0) {
            factor = 0;
            return;
        }
        double parsed = atof(value);
        if (parsed >= 0.25 && parsed <= 1.0) factor = parsed;
    });
    return factor;
}

static double PearDisplayScale(id screen) {
    double cached = atomic_load(&gDisplayScale);
    const BOOL main = [NSThread isMainThread];
    if (cached > 0 && !main) return cached;
    double scale = 0;
    if (main) {
        UIWindow *window = PearMainWindow();
        if (window) scale = window.traitCollection.displayScale;
    }
    // Before the window exists, Catalyst's UIScreen reports a nominal 2.0 even
    // on a 1x display. An engine that sized its drawable from that value would
    // then disagree with the window (and with touch coordinates) once the real
    // scale appears, so ask AppKit for the actual display instead.
    if (scale <= 0 && main) {
        Class nsScreen = NSClassFromString(@"NSScreen");
        id mainScreen = [nsScreen respondsToSelector:@selector(mainScreen)]
            ? ((id (*)(id, SEL))objc_msgSend)(nsScreen, @selector(mainScreen)) : nil;
        SEL backing = NSSelectorFromString(@"backingScaleFactor");
        if ([mainScreen respondsToSelector:backing]) {
            scale = ((CGFloat (*)(id, SEL))objc_msgSend)(mainScreen, backing);
        }
    }
    if (scale <= 0 && gOriginalScaleIMP && screen) {
        scale = ((CGFloat (*)(id, SEL))gOriginalScaleIMP)(screen, @selector(scale));
    }
    if (scale <= 0) scale = cached > 0 ? cached : 2.0;
    static _Atomic double logged = 0;
    if (scale != cached) atomic_store(&gDisplayScale, scale);
    if (scale != atomic_exchange(&logged, scale)) {
        double factor = PearRenderScaleFactor();
        PearLog(@"[PearRuntime][screen] display backing scale %.2f, render scale %.2f",
                scale, factor == 0 ? 1.0 : MAX(1.0, scale * factor));
    }
    return scale;
}

/// The scale reported to the app: the display's backing scale times the
/// configured render factor, never below 1.
static CGFloat PearReportedScale(id screen) {
    double factor = PearRenderScaleFactor();
    if (factor == 0) return 1.0;
    return (CGFloat)MAX(1.0, PearDisplayScale(screen) * factor);
}

/// The orientation implied by the app's actual window shape. Used instead of
/// the scene wrapper's stale portrait/landscape flag.
static UIInterfaceOrientation PearDesiredOrientation(void) {
    // A fixed GL drawable defines the app's coordinate space. Reporting the
    // window's landscape shape in full screen would make such an engine rotate
    // its hit-testing while the drawable stays portrait, so every control moves.
    if (gReportedOverride.width > 0 && gReportedOverride.height > 0) {
        CGSize size = gReportedOverride;
        if (size.width > size.height + 1) return UIInterfaceOrientationLandscapeLeft;
        if (size.height > size.width + 1) return UIInterfaceOrientationPortrait;
        return UIInterfaceOrientationUnknown;
    }
    UIWindow *window = PearMainWindow();
    if (!window) return UIInterfaceOrientationUnknown;
    CGSize size = window.bounds.size;
    if (size.width < 1 || size.height < 1) return UIInterfaceOrientationUnknown;
    if (size.width > size.height + 1) return UIInterfaceOrientationLandscapeLeft;
    if (size.height > size.width + 1) return UIInterfaceOrientationPortrait;
    return UIInterfaceOrientationUnknown;
}

static void PearInstallScreenShim(void) {
    if (NSProcessInfo.processInfo.environment[@"PEAR_NO_SCREEN_SHIM"]) return;
    Class screen = objc_getClass("UIScreen");
    if (!screen) return;

    Method boundsMethod = class_getInstanceMethod(screen, @selector(bounds));
    IMP originalBounds = method_getImplementation(boundsMethod);
    method_setImplementation(boundsMethod, imp_implementationWithBlock(^CGRect(id self) {
        CGRect reported = PearReportedBounds();
        if (!CGRectIsNull(reported)) return reported;
        return ((CGRect (*)(id, SEL))originalBounds)(self, @selector(bounds));
    }));

    Method scaleMethod = class_getInstanceMethod(screen, @selector(scale));
    gOriginalScaleIMP = method_getImplementation(scaleMethod);
    method_setImplementation(scaleMethod, imp_implementationWithBlock(^CGFloat(id self) {
        return PearReportedScale(self);
    }));

    Method nativeScaleMethod = class_getInstanceMethod(screen, @selector(nativeScale));
    method_setImplementation(nativeScaleMethod, imp_implementationWithBlock(^CGFloat(id self) {
        return PearReportedScale(self);
    }));

    // Pixels, as on a device: engines that size their backbuffer from
    // nativeBounds then match the window's real pixel count.
    Method nativeBoundsMethod = class_getInstanceMethod(screen, @selector(nativeBounds));
    method_setImplementation(nativeBoundsMethod, imp_implementationWithBlock(^CGRect(id self) {
        CGRect reported = PearReportedBounds();
        CGFloat scale = PearReportedScale(self);
        return CGRectMake(0, 0, round(reported.size.width * scale), round(reported.size.height * scale));
    }));

    // Moving the window between a Retina panel and a 1x external display
    // changes the backing scale; refresh the cache on the main thread.
    for (NSString *name in @[ @"NSWindowDidChangeBackingPropertiesNotification",
                              @"NSWindowDidChangeScreenNotification" ]) {
        [NSNotificationCenter.defaultCenter addObserverForName:name object:nil
            queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) {
                atomic_store(&gDisplayScale, 0);
                PearDisplayScale(nil);
            }];
    }

    PearLog(@"[PearRuntime][screen] shim installed (UIScreen follows the window at native backing scale)");

    if (NSProcessInfo.processInfo.environment[@"PEAR_NO_ORIENTATION_SHIM"]) {
        PearLog(@"[PearRuntime][screen] orientation shim disabled");
        return;
    }

    // The Mac scene wrapper reports portrait for a landscape window (and vice
    // versa on tall windows). Engines that swap their backbuffer dimensions
    // from the interface orientation then render transposed. Derive the
    // orientation from the actual window instead.
    Method statusMethod = class_getInstanceMethod(UIApplication.class,
                                                   @selector(statusBarOrientation));
    if (statusMethod) {
        IMP originalStatus = method_getImplementation(statusMethod);
        method_setImplementation(statusMethod, imp_implementationWithBlock(^UIInterfaceOrientation(id self) {
            if ([NSThread isMainThread]) {
                UIInterfaceOrientation desired = PearDesiredOrientation();
                if (desired != UIInterfaceOrientationUnknown) return desired;
            }
            return ((UIInterfaceOrientation (*)(id, SEL))originalStatus)(self, @selector(statusBarOrientation));
        }));
    }

    Class sceneClass = objc_getClass("UIWindowScene");
    Method sceneMethod = sceneClass
        ? class_getInstanceMethod(sceneClass, @selector(interfaceOrientation)) : NULL;
    if (sceneMethod) {
        IMP originalScene = method_getImplementation(sceneMethod);
        method_setImplementation(sceneMethod, imp_implementationWithBlock(^UIInterfaceOrientation(id self) {
            if ([NSThread isMainThread]) {
                UIInterfaceOrientation desired = PearDesiredOrientation();
                if (desired != UIInterfaceOrientationUnknown) return desired;
            }
            return ((UIInterfaceOrientation (*)(id, SEL))originalScene)(self, @selector(interfaceOrientation));
        }));
    }

    // Catalyst reports `UIDeviceOrientationUnknown`; a port that treats that
    // as portrait swaps its backbuffer dimensions and renders transposed.
    Method deviceMethod = class_getInstanceMethod(UIDevice.class, @selector(orientation));
    if (deviceMethod) {
        IMP originalDevice = method_getImplementation(deviceMethod);
        method_setImplementation(deviceMethod, imp_implementationWithBlock(^UIDeviceOrientation(id self) {
            if ([NSThread isMainThread]) {
                UIInterfaceOrientation desired = PearDesiredOrientation();
                if (desired == UIInterfaceOrientationLandscapeLeft ||
                    desired == UIInterfaceOrientationLandscapeRight) {
                    return UIDeviceOrientationLandscapeLeft;
                }
                if (desired == UIInterfaceOrientationPortrait) return UIDeviceOrientationPortrait;
            }
            return ((UIDeviceOrientation (*)(id, SEL))originalDevice)(self, @selector(orientation));
        }));
    }

    // Old UIKit code (and MonoGame's iOS backend) reads the view controller's
    // deprecated `interfaceOrientation`, which stays at its launch value.
    Method controllerMethod = class_getInstanceMethod(UIViewController.class,
                                                      @selector(interfaceOrientation));
    if (controllerMethod) {
        IMP originalController = method_getImplementation(controllerMethod);
        method_setImplementation(controllerMethod, imp_implementationWithBlock(^UIInterfaceOrientation(id self) {
            atomic_store(&gInterfaceOrientationRead, YES);
            if ([NSThread isMainThread]) {
                UIInterfaceOrientation desired = PearDesiredOrientation();
                if (desired != UIInterfaceOrientationUnknown) return desired;
            }
            return ((UIInterfaceOrientation (*)(id, SEL))originalController)(self, @selector(interfaceOrientation));
        }));
    }

    PearLog(@"[PearRuntime][screen] interface orientation follows the window");

    // The shim above only helps if the app re-reads the orientation. Titles
    // that cache it during launch keep the launch-time (portrait) value and
    // render a transposed backbuffer. Nudge the normal notification path once
    // the window exists — but only for apps that actually used the deprecated
    // controller property. Broadcasting an orientation change to every app
    // makes UIKit rebuild navigation bars mid-transition; apps that never read
    // the cached value (iMovie) then get stuck laying out nav-bar buttons.
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(2.0 * NSEC_PER_SEC)),
                   dispatch_get_main_queue(), ^{
        if (!atomic_load(&gInterfaceOrientationRead)) {
            PearLog(@"[PearRuntime][screen] orientation notification skipped (no cached read)");
            return;
        }
        [[NSNotificationCenter defaultCenter]
            postNotificationName:UIDeviceOrientationDidChangeNotification object:nil];
        [[NSNotificationCenter defaultCenter]
            postNotificationName:UIApplicationDidChangeStatusBarOrientationNotification object:nil];
        PearLog(@"[PearRuntime][screen] posted orientation change");
    });
}

__attribute__((constructor))
static void PearScreenShimInit(void) {
    PEAR_MODULE_GUARD();
    @autoreleasepool {
        PearInstallScreenShim();
    }
}
