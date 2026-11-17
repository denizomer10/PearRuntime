// UIApplication and UIApplicationMain.
// Clean-room: launch sequence and delegate callbacks follow Apple's public
// "About the app launch sequence" and UIApplicationDelegate documentation.
import Foundation
import ObjectiveC

private var gApp: UIApplication?
private var gWindows: [UIWindow] = []
private weak var gKeyWindow: UIWindow?
private var gState: UIApplicationState = .inactive
private var gLaunched = false
private var gMainNibObjects: [Any]?

@_cdecl("PearUIKitRegisterWindow")
public func PearUIKitRegisterWindow(_ window: UIWindow) {
    gWindows.append(window)
}

@_cdecl("PearUIKitSetKeyWindow")
public func PearUIKitSetKeyWindow(_ window: UIWindow?) {
    gKeyWindow = window
}

@_cdecl("PearUIKitWindows")
public func PearUIKitWindows() -> [UIWindow] {
    gWindows
}

@_cdecl("PearUIKitKeyWindow")
public func PearUIKitKeyWindow() -> UIWindow? {
    gKeyWindow
}

private func sendObjC(_ target: AnyObject, _ sel: Selector, _ arg1: AnyObject?) {
    guard let method = class_getInstanceMethod(object_getClass(target), sel) else { return }
    let imp = method_getImplementation(method)
    let fn = unsafeBitCast(imp, to: (@convention(c) (AnyObject, Selector, AnyObject?) -> Void).self)
    fn(target, sel, arg1)
}

@discardableResult
private func sendObjCBool(_ target: AnyObject, _ sel: Selector, _ arg1: AnyObject?, _ arg2: AnyObject?) -> Bool {
    guard let method = class_getInstanceMethod(object_getClass(target), sel) else { return false }
    let imp = method_getImplementation(method)
    let fn = unsafeBitCast(imp, to: (@convention(c) (AnyObject, Selector, AnyObject?, AnyObject?) -> ObjCBool).self)
    return fn(target, sel, arg1, arg2).boolValue
}

private func sendObjC2(_ target: AnyObject, _ sel: Selector, _ arg1: AnyObject?, _ arg2: AnyObject?) {
    guard let method = class_getInstanceMethod(object_getClass(target), sel) else { return }
    let imp = method_getImplementation(method)
    let fn = unsafeBitCast(imp, to: (@convention(c) (AnyObject, Selector, AnyObject?, AnyObject?) -> Void).self)
    fn(target, sel, arg1, arg2)
}

@objc(UIApplication)
open class UIApplication: UIResponder {
    private weak var _delegate: AnyObject?
    private var _strongDelegate: AnyObject? // UIApplicationMain owns the delegate it creates
    private var _idleTimerDisabled = false
    private var _badge = 0
    private var _nextTask: UInt = 0

    @objc(sharedApplication)
    public class func sharedApplication() -> UIApplication? { gApp }

    public required override init() {
        super.init()
        if gApp == nil { gApp = self }
    }

    @objc open var delegate: AnyObject? {
        get { _delegate }
        set { _delegate = newValue }
    }

    @objc open var keyWindow: UIWindow? { gKeyWindow }
    @objc open var windows: [UIWindow] { gWindows }
    @objc open var connectedScenes: Set<AnyHashable> { [] }
    @objc open var supportsMultipleScenes: Bool { false }
    @objc open var applicationState: UIApplicationState { gState }

    @objc(isIdleTimerDisabled) open func isIdleTimerDisabled() -> Bool { _idleTimerDisabled }
    @objc(setIdleTimerDisabled:) open func setIdleTimerDisabled(_ disabled: Bool) {
        _idleTimerDisabled = disabled
        PearBackendSetIdleTimerDisabled(disabled)
    }

    @objc open var statusBarOrientation: UIInterfaceOrientation { .landscapeRight }
    @objc(isStatusBarHidden) open func isStatusBarHidden() -> Bool { true }
    @objc(setStatusBarHidden:) open func setStatusBarHidden(_ hidden: Bool) {}
    @objc(setStatusBarHidden:withAnimation:) open func setStatusBarHidden(_ hidden: Bool, withAnimation animation: Int) {}
    @objc open var statusBarFrame: CGRect { .zero }

    @objc open var applicationIconBadgeNumber: Int {
        get { _badge }
        set { _badge = newValue }
    }

    @objc open var backgroundTimeRemaining: TimeInterval { 180 }
    @objc(isProtectedDataAvailable) open func isProtectedDataAvailable() -> Bool { true }
    @objc(isRegisteredForRemoteNotifications) open func isRegisteredForRemoteNotifications() -> Bool { false }
    @objc open var userInterfaceLayoutDirection: Int { 0 }
    @objc open var preferredContentSizeCategory: String { "UICTContentSizeCategoryL" }

    @objc(canOpenURL:)
    open func canOpenURL(_ url: URL) -> Bool {
        let scheme = url.scheme?.lowercased()
        return scheme == "http" || scheme == "https" || scheme == "mailto"
    }

    @objc(openURL:)
    open func openURL(_ url: URL) -> Bool {
        canOpenURL(url) && PearBackendOpenURL(url)
    }

    @objc(openURL:options:completionHandler:)
    open func openURL(_ url: URL, options: [String: Any], completionHandler completion: ((Bool) -> Void)?) {
        let ok = openURL(url)
        if let completion {
            DispatchQueue.main.async { completion(ok) }
        }
    }

    @objc open func registerForRemoteNotifications() {
        // No APNs on this host: report the documented failure path.
        guard let delegate = _delegate else { return }
        let fail = NSSelectorFromString("application:didFailToRegisterForRemoteNotificationsWithError:")
        if delegate.responds(to: fail) {
            let err = NSError(
                domain: NSCocoaErrorDomain,
                code: 3010,
                userInfo: [NSLocalizedDescriptionKey: "remote notifications are not supported"]
            )
            DispatchQueue.main.async {
                sendObjC2(delegate, fail, self, err)
            }
        }
    }

    @objc open func unregisterForRemoteNotifications() {}
    @objc(registerUserNotificationSettings:) open func registerUserNotificationSettings(_ settings: Any?) {}

    @objc(beginBackgroundTaskWithExpirationHandler:)
    open func beginBackgroundTask(expirationHandler handler: (() -> Void)?) -> UInt {
        _nextTask += 1
        return _nextTask
    }

    @objc(beginBackgroundTaskWithName:expirationHandler:)
    open func beginBackgroundTask(withName name: String?, expirationHandler handler: (() -> Void)?) -> UInt {
        _nextTask += 1
        return _nextTask
    }

    @objc(endBackgroundTask:) open func endBackgroundTask(_ identifier: UInt) {}
    @objc(setMinimumBackgroundFetchInterval:) open func setMinimumBackgroundFetchInterval(_ interval: TimeInterval) {}

    @objc(sendAction:to:from:forEvent:)
    open func sendAction(_ action: Selector, to target: AnyObject?, from sender: AnyObject?, for event: UIEvent?) -> Bool {
        guard let target, target.responds(to: action) else { return false }
        sendObjC(target, action, sender)
        return true
    }

    @objc open func beginReceivingRemoteControlEvents() {}
    @objc open func endReceivingRemoteControlEvents() {}

    @objc(pear_retainDelegate:)
    public func pear_retainDelegate(_ delegate: AnyObject?) {
        _strongDelegate = delegate
        _delegate = delegate
    }
}

private func callDelegate(_ selName: String) {
    guard let app = gApp, let delegate = app.delegate else { return }
    let sel = NSSelectorFromString(selName)
    if delegate.responds(to: sel) {
        sendObjC(delegate, sel, app)
    }
}

private func postNotification(_ name: NSNotification.Name) {
    NotificationCenter.default.post(name: name, object: gApp)
}

@_cdecl("PearUIKitApplicationDidBecomeActive")
public func PearUIKitApplicationDidBecomeActive() {
    guard gLaunched, gState != .active else { return }
    gState = .active
    callDelegate("applicationDidBecomeActive:")
    postNotification(NSNotification.Name("UIApplicationDidBecomeActiveNotification"))
}

@_cdecl("PearUIKitApplicationWillResignActive")
public func PearUIKitApplicationWillResignActive() {
    guard gLaunched, gState == .active else { return }
    gState = .inactive
    callDelegate("applicationWillResignActive:")
    postNotification(NSNotification.Name("UIApplicationWillResignActiveNotification"))
}

@_cdecl("PearUIKitApplicationDidEnterBackground")
public func PearUIKitApplicationDidEnterBackground() {
    guard gLaunched else { return }
    PearUIKitApplicationWillResignActive()
    gState = .background
    callDelegate("applicationDidEnterBackground:")
    postNotification(NSNotification.Name("UIApplicationDidEnterBackgroundNotification"))
}

@_cdecl("PearUIKitApplicationWillEnterForeground")
public func PearUIKitApplicationWillEnterForeground() {
    guard gLaunched, gState == .background else { return }
    gState = .inactive
    callDelegate("applicationWillEnterForeground:")
    postNotification(NSNotification.Name("UIApplicationWillEnterForegroundNotification"))
}

@_cdecl("PearUIKitApplicationWillTerminate")
public func PearUIKitApplicationWillTerminate() {
    callDelegate("applicationWillTerminate:")
    postNotification(NSNotification.Name("UIApplicationWillTerminateNotification"))
}

private func classNamed(_ name: String?, fallback: AnyClass?) -> AnyClass? {
    guard let name, !name.isEmpty else { return fallback }
    if let cls = NSClassFromString(name) { return cls }
    PearHostLogMessage("app", "class \(name) not found")
    return fallback
}

@_cdecl("UIApplicationMain")
public func UIApplicationMain(
    _ argc: Int32,
    _ argv: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?,
    _ principalClassName: NSString?,
    _ delegateClassName: NSString?
) -> Int32 {
    autoreleasepool {
        let pName = principalClassName as String?
        let dName = delegateClassName as String?
        PearHostLogMessage("app", "UIApplicationMain principal=\(pName ?? "(default)") delegate=\(dName ?? "(none)")")
        PearBackendInitialize()

        let appClass = (classNamed(pName, fallback: UIApplication.self) as? UIApplication.Type) ?? UIApplication.self
        let app = appClass.init()
        gApp = app

        var delegate: AnyObject?
        if let dName, !dName.isEmpty, let delClass = classNamed(dName, fallback: nil) as? NSObject.Type {
            delegate = delClass.init()
        } else if let proto = NSProtocolFromString("UIApplicationDelegate"), app.conforms(to: proto) {
            delegate = app
        }
        app.pear_retainDelegate(delegate)

        let info = Bundle.main.infoDictionary
        let mainNib = (info?["NSMainNibFile~ipad"] as? String) ?? (info?["NSMainNibFile"] as? String)
        if (dName == nil || dName?.isEmpty == true), info?["UIMainStoryboardFile"] != nil {
            PearMissingReport(PearMissingBehavior, "UIApplicationMain", "main storyboard launch", nil)
        }

        PearBackendRun {
            PearUIKitRunCatchingExceptions {
                // Documented: the main nib is loaded with the application as owner
                // before launch; it usually connects the app delegate.
                var nibWindows: [UIWindow] = []
                if let mainNib, !mainNib.isEmpty, let nib = UINib.nib(withNibName: mainNib, bundle: nil) {
                    var outWindows: NSArray? = []
                    gMainNibObjects = nib.pear_instantiate(withOwner: app, options: nil, visibleWindows: &outWindows)
                    if let arr = outWindows as? [UIWindow] {
                        nibWindows = arr
                    }
                }
                let del = app.delegate
                if del == nil {
                    PearHostLogMessage("app", "no application delegate after launch setup")
                }
                let options: NSDictionary = [:]
                let will = NSSelectorFromString("application:willFinishLaunchingWithOptions:")
                let did = NSSelectorFromString("application:didFinishLaunchingWithOptions:")
                let sceneSel = NSSelectorFromString("application:configurationForConnectingSceneSession:options:")
                let didOld = NSSelectorFromString("applicationDidFinishLaunching:")

                if let del {
                    if del.responds(to: will) {
                        _ = sendObjCBool(del, will, app, options)
                    }
                    if del.responds(to: sceneSel) {
                        PearHostLogMessage("app", "delegate supports scenes; launching without a scene session")
                    }
                    if del.responds(to: did) {
                        _ = sendObjCBool(del, did, app, options)
                    } else if del.responds(to: didOld) {
                        sendObjC(del, didOld, app)
                    }
                }
                for w in nibWindows where !w.isKeyWindow() {
                    w.makeKeyAndVisible()
                }
                gLaunched = true
                postNotification(NSNotification.Name("UIApplicationDidFinishLaunchingNotification"))
                PearHostLogMessage("app", "launch complete; \(gWindows.count) window(s)")
                PearUIKitApplicationDidBecomeActive()
                PearBackendStartInputScript()
            }
        }
    }
    return 0
}
