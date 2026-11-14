// UIApplication and UIApplicationMain.
// Clean-room: launch sequence and delegate callbacks follow Apple's public
// "About the app launch sequence" and UIApplicationDelegate documentation.
#import "PearUIKitPrivate.h"
#import <dlfcn.h>
#import <mach-o/dyld.h>
#import <objc/message.h>

PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidFinishLaunchingNotification = @"UIApplicationDidFinishLaunchingNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidBecomeActiveNotification = @"UIApplicationDidBecomeActiveNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationWillResignActiveNotification = @"UIApplicationWillResignActiveNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidEnterBackgroundNotification = @"UIApplicationDidEnterBackgroundNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationWillEnterForegroundNotification = @"UIApplicationWillEnterForegroundNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationWillTerminateNotification = @"UIApplicationWillTerminateNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIApplicationDidReceiveMemoryWarningNotification = @"UIApplicationDidReceiveMemoryWarningNotification";

static UIApplication *gApp;
static NSMutableArray<UIWindow *> *gWindows;
static __weak UIWindow *gKeyWindow;
static UIApplicationState gState = UIApplicationStateInactive;
static BOOL gLaunched;

void PearUIKitRegisterWindow(UIWindow *window) {
    if (!gWindows) gWindows = [NSMutableArray array];
    [gWindows addObject:window];
}
void PearUIKitSetKeyWindow(UIWindow *window) { gKeyWindow = window; }
NSArray<UIWindow *> *PearUIKitWindows(void) { return [gWindows copy] ?: @[]; }
UIWindow *PearUIKitKeyWindow(void) { return gKeyWindow; }

@implementation UIApplication {
    __weak id _delegate;
    id _strongDelegate;  // UIApplicationMain owns the delegate it creates
    BOOL _idleTimerDisabled;
    NSInteger _badge;
    NSUInteger _nextTask;
}
+ (UIApplication *)sharedApplication { return gApp; }
- (instancetype)init {
    if ((self = [super init])) {
        if (!gApp) gApp = self;
    }
    return self;
}
- (id)delegate { return _delegate; }
- (void)setDelegate:(id)d { _delegate = d; }
- (UIWindow *)keyWindow { return gKeyWindow; }
- (NSArray<UIWindow *> *)windows { return PearUIKitWindows(); }
- (NSSet *)connectedScenes { return [NSSet set]; }
- (BOOL)supportsMultipleScenes { return NO; }
- (UIApplicationState)applicationState { return gState; }
- (BOOL)isIdleTimerDisabled { return _idleTimerDisabled; }
- (void)setIdleTimerDisabled:(BOOL)d {
    _idleTimerDisabled = d;
    PearBackendSetIdleTimerDisabled(d);
}
- (UIInterfaceOrientation)statusBarOrientation { return UIInterfaceOrientationLandscapeRight; }
- (BOOL)isStatusBarHidden { return YES; }
- (void)setStatusBarHidden:(BOOL)h {}
- (void)setStatusBarHidden:(BOOL)h withAnimation:(NSInteger)a {}
- (CGRect)statusBarFrame { return CGRectZero; }
- (NSInteger)applicationIconBadgeNumber { return _badge; }
- (void)setApplicationIconBadgeNumber:(NSInteger)n { _badge = n; }
- (NSTimeInterval)backgroundTimeRemaining { return 180; }
- (BOOL)isProtectedDataAvailable { return YES; }
- (BOOL)isRegisteredForRemoteNotifications { return NO; }
- (NSInteger)userInterfaceLayoutDirection { return 0; }
- (NSString *)preferredContentSizeCategory { return @"UICTContentSizeCategoryL"; }

- (BOOL)canOpenURL:(NSURL *)url {
    NSString *s = url.scheme.lowercaseString;
    return [s isEqualToString:@"http"] || [s isEqualToString:@"https"] || [s isEqualToString:@"mailto"];
}
- (BOOL)openURL:(NSURL *)url { return [self canOpenURL:url] && PearBackendOpenURL(url); }
- (void)openURL:(NSURL *)url options:(NSDictionary *)options completionHandler:(void (^)(BOOL))completion {
    BOOL ok = [self openURL:url];
    if (completion) dispatch_async(dispatch_get_main_queue(), ^{ completion(ok); });
}
- (void)registerForRemoteNotifications {
    // No APNs on this host: report the documented failure path.
    id d = _delegate;
    SEL fail = @selector(application:didFailToRegisterForRemoteNotificationsWithError:);
    if ([d respondsToSelector:fail]) {
        NSError *e = [NSError errorWithDomain:NSCocoaErrorDomain code:3010
                                     userInfo:@{NSLocalizedDescriptionKey : @"remote notifications are not supported"}];
        dispatch_async(dispatch_get_main_queue(), ^{
            ((void (*)(id, SEL, id, id))objc_msgSend)(d, fail, self, e);
        });
    }
}
- (void)unregisterForRemoteNotifications {}
- (void)registerUserNotificationSettings:(id)settings {}
- (NSUInteger)beginBackgroundTaskWithExpirationHandler:(void (^)(void))h { return ++_nextTask; }
- (NSUInteger)beginBackgroundTaskWithName:(NSString *)n expirationHandler:(void (^)(void))h { return ++_nextTask; }
- (void)endBackgroundTask:(NSUInteger)identifier {}
- (void)setMinimumBackgroundFetchInterval:(NSTimeInterval)i {}
- (BOOL)sendAction:(SEL)action to:(id)target from:(id)sender forEvent:(UIEvent *)event {
    if (!target || ![target respondsToSelector:action]) return NO;
    ((void (*)(id, SEL, id))objc_msgSend)(target, action, sender);
    return YES;
}
- (void)beginReceivingRemoteControlEvents {}
- (void)endReceivingRemoteControlEvents {}
- (void)pear_retainDelegate:(id)d {
    _strongDelegate = d;
    _delegate = d;
}
@end

#import <objc/message.h>

static void callDelegate(SEL sel) {
    id d = gApp.delegate;
    if ([d respondsToSelector:sel]) ((void (*)(id, SEL, id))objc_msgSend)(d, sel, gApp);
}

static void post(NSNotificationName name) {
    [NSNotificationCenter.defaultCenter postNotificationName:name object:gApp];
}

void PearUIKitApplicationDidBecomeActive(void) {
    if (!gLaunched || gState == UIApplicationStateActive) return;
    gState = UIApplicationStateActive;
    callDelegate(@selector(applicationDidBecomeActive:));
    post(UIApplicationDidBecomeActiveNotification);
}
void PearUIKitApplicationWillResignActive(void) {
    if (!gLaunched || gState != UIApplicationStateActive) return;
    gState = UIApplicationStateInactive;
    callDelegate(@selector(applicationWillResignActive:));
    post(UIApplicationWillResignActiveNotification);
}
void PearUIKitApplicationDidEnterBackground(void) {
    if (!gLaunched) return;
    PearUIKitApplicationWillResignActive();
    gState = UIApplicationStateBackground;
    callDelegate(@selector(applicationDidEnterBackground:));
    post(UIApplicationDidEnterBackgroundNotification);
}
void PearUIKitApplicationWillEnterForeground(void) {
    if (!gLaunched || gState != UIApplicationStateBackground) return;
    gState = UIApplicationStateInactive;
    callDelegate(@selector(applicationWillEnterForeground:));
    post(UIApplicationWillEnterForegroundNotification);
}
void PearUIKitApplicationWillTerminate(void) {
    callDelegate(@selector(applicationWillTerminate:));
    post(UIApplicationWillTerminateNotification);
}

static Class classNamed(NSString *name, Class fallback) {
    if (!name.length) return fallback;
    Class c = NSClassFromString(name);
    if (!c) PearHostLog("app", "class %s not found", name.UTF8String);
    return c ?: fallback;
}

PEAR_UIKIT_EXPORT int UIApplicationMain(int argc, char *argv[], NSString *principalClassName, NSString *delegateClassName) {
    @autoreleasepool {
        PearHostLog("app", "UIApplicationMain principal=%s delegate=%s", principalClassName.UTF8String ?: "(default)",
                    delegateClassName.UTF8String ?: "(none)");
        PearBackendInitialize();

        Class appClass = classNamed(principalClassName, [UIApplication class]);
        UIApplication *app = [[appClass alloc] init];
        gApp = app;

        id delegate = nil;
        if (delegateClassName.length) delegate = [[classNamed(delegateClassName, Nil) alloc] init];
        else if ([app conformsToProtocol:NSProtocolFromString(@"UIApplicationDelegate")]) delegate = app;
        [app pear_retainDelegate:delegate];
        NSDictionary *info = NSBundle.mainBundle.infoDictionary;
        NSString *mainNib = info[@"NSMainNibFile~ipad"] ?: info[@"NSMainNibFile"];
        if (!delegateClassName.length && info[@"UIMainStoryboardFile"])
            PearMissingReport(PearMissingBehavior, "UIApplicationMain", "main storyboard launch", NULL);

        PearBackendRun(^{
          @try {
            // Documented: the main nib is loaded with the application as owner
            // before launch; it usually connects the app delegate.
            NSArray *nibWindows = @[];
            if (mainNib.length) {
                UINib *nib = [UINib nibWithNibName:mainNib bundle:nil];
                // UIKit keeps the main nib's top-level objects for the life of the
                // app (the delegate outlet itself is not retained).
                static NSArray *mainNibObjects __attribute__((unused));
                mainNibObjects = [nib pear_instantiateWithOwner:app options:nil visibleWindows:&nibWindows];
            }
            id delegate = app.delegate;
            if (!delegate) PearHostLog("app", "no application delegate after launch setup");
            NSDictionary *options = @{};
            SEL will = @selector(application:willFinishLaunchingWithOptions:);
            SEL did = @selector(application:didFinishLaunchingWithOptions:);
            if ([delegate respondsToSelector:will])
                ((BOOL(*)(id, SEL, id, id))objc_msgSend)(delegate, will, app, options);
            if ([delegate respondsToSelector:@selector(application:configurationForConnectingSceneSession:options:)])
                PearHostLog("app", "delegate supports scenes; launching without a scene session");
            if ([delegate respondsToSelector:did]) {
                ((BOOL(*)(id, SEL, id, id))objc_msgSend)(delegate, did, app, options);
            } else if ([delegate respondsToSelector:@selector(applicationDidFinishLaunching:)]) {
                ((void (*)(id, SEL, id))objc_msgSend)(delegate, @selector(applicationDidFinishLaunching:), app);
            }
            for (UIWindow *w in nibWindows)
                if (!w.isKeyWindow) [w makeKeyAndVisible];
            gLaunched = YES;
            post(UIApplicationDidFinishLaunchingNotification);
            PearHostLog("app", "launch complete; %lu window(s)", (unsigned long)gWindows.count);
            PearUIKitApplicationDidBecomeActive();
            PearBackendStartInputScript();
          } @catch (NSException *e) {
            // AppKit swallows exceptions thrown from launch notifications; an
            // iOS app would have crashed here, so say so loudly and stop.
            PearHostLog("app", "launch raised %s: %s", e.name.UTF8String, e.reason.UTF8String);
            for (NSString *f in e.callStackSymbols) PearHostLog("app", "  %s", f.UTF8String);
            abort();
          }
        });
    }
    return 0;
}

PEAR_UIKIT_EXPORT NSData *UIImagePNGRepresentation(id image) {
    PearMissingReport(PearMissingCFunction, NULL, "UIImagePNGRepresentation", __builtin_return_address(0));
    return nil;
}

// Install the missing-call reporter on every class in this image as soon as the
// image loads, before the app sends its first message.
__attribute__((constructor)) static void PearUIKitLoad(void) {
    PearHostLog("", "PearHost UIKit %s (no iOSSupport) loading", PEAR_RUNTIME_VERSION);
    PearMissingInstallForImage((const void *)&PearUIKitLoad);
}
