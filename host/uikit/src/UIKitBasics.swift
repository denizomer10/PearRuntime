// UIColor, UITraitCollection, UIScreen, UIDevice, UIResponder, UITouch, UIEvent.
// Clean-room: behavior from public UIKit documentation and headers.
import CoreGraphics
import Darwin
import Foundation
import QuartzCore

// ---- UIColor -----------------------------------------------------------------

@objc(UIColor)
open class UIColor: NSObject, NSCopying {
    private let _color: CGColor

    @objc(colorWithRed:green:blue:alpha:)
    public class func color(red r: CGFloat, green g: CGFloat, blue b: CGFloat, alpha a: CGFloat) -> UIColor {
        UIColor(red: r, green: g, blue: b, alpha: a)
    }

    @objc(colorWithWhite:alpha:)
    public class func color(white w: CGFloat, alpha a: CGFloat) -> UIColor {
        UIColor(red: w, green: w, blue: w, alpha: a)
    }

    @objc public class func clearColor() -> UIColor { color(white: 0, alpha: 0) }
    @objc public class func blackColor() -> UIColor { color(white: 0, alpha: 1) }
    @objc public class func whiteColor() -> UIColor { color(white: 1, alpha: 1) }

    @objc(initWithRed:green:blue:alpha:)
    public init(red r: CGFloat, green g: CGFloat, blue b: CGFloat, alpha a: CGFloat) {
        _color = CGColor(srgbRed: r, green: g, blue: b, alpha: a)
        super.init()
    }

    @objc(CGColor)
    public var cgColor: CGColor { _color }

    @objc(copyWithZone:)
    public func copy(with zone: NSZone? = nil) -> Any { self }
}

// ---- UITraitCollection ---------------------------------------------------------

@objc(UITraitCollection)
open class UITraitCollection: NSObject, NSCopying {
    @objc(pear_current)
    public class func pear_current() -> UITraitCollection { UITraitCollection() }

    @objc(currentTraitCollection)
    public class func currentTraitCollection() -> UITraitCollection { pear_current() }

    @objc public var userInterfaceIdiom: UIUserInterfaceIdiom { UIDevice.currentDevice().userInterfaceIdiom }
    @objc public var displayScale: CGFloat { UIScreen.mainScreen().scale }
    @objc public var userInterfaceStyle: UIUserInterfaceStyle { .dark }
    @objc public var horizontalSizeClass: Int { 2 } // regular
    @objc public var verticalSizeClass: Int { 2 }
    @objc public var forceTouchCapability: Int { 1 } // unavailable
    @objc public var displayGamut: Int { 1 } // P3

    @objc(copyWithZone:)
    public func copy(with zone: NSZone? = nil) -> Any { self }
}

// ---- UIScreen ------------------------------------------------------------------

@objc(UIScreen)
open class UIScreen: NSObject {
    private static let sharedMainScreen: UIScreen = {
        let screen = UIScreen()
        let info = PearBackendScreenInfo()
        screen._size = info.pointSize
        screen._scale = info.scale
        screen._brightness = 1.0
        return screen
    }()

    private var _size: CGSize = .zero
    private var _scale: CGFloat = 1.0
    private var _brightness: CGFloat = 1.0

    @objc(mainScreen)
    public class func mainScreen() -> UIScreen { sharedMainScreen }

    @objc public class func screens() -> [UIScreen] { [mainScreen()] }

    @objc public var bounds: CGRect { CGRect(origin: .zero, size: _size) }

    @objc public var nativeBounds: CGRect {
        // Documented as portrait-up, in pixels.
        let w = _size.width * _scale
        let h = _size.height * _scale
        return CGRect(x: 0, y: 0, width: min(w, h), height: max(w, h))
    }

    @objc public var applicationFrame: CGRect { bounds }
    @objc public var scale: CGFloat { _scale }
    @objc public var nativeScale: CGFloat { _scale }
    @objc public var maximumFramesPerSecond: Int { PearBackendScreenInfo().maxFPS }

    @objc public var brightness: CGFloat {
        get { _brightness }
        set { _brightness = newValue }
    }

    @objc public var displayGamut: Int { 1 } // P3
    @objc public var traitCollection: UITraitCollection { UITraitCollection.pear_current() }
    @objc public var coordinateSpace: AnyObject { self }
    @objc public var fixedCoordinateSpace: AnyObject { self }
    @objc public var mirroredScreen: UIScreen? { nil }
    @objc(isCaptured) public func isCaptured() -> Bool { false }
    @objc public var overscanCompensationInsets: UIEdgeInsets { UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 0) }

    @objc(displayLinkWithTarget:selector:)
    public func displayLink(withTarget target: Any, selector sel: Selector) -> CADisplayLink? {
        PearBackendDisplayLink(target, sel)
    }

    @objc(pear_update:scale:)
    public func pear_update(size: CGSize, scale: CGFloat) {
        _size = size
        _scale = scale
    }
}

@_cdecl("PearUIKitScreenDidChange")
public func PearUIKitScreenDidChange(_ pointSize: CGSize, _ scale: CGFloat) {
    UIScreen.mainScreen().pear_update(size: pointSize, scale: scale)
}

// ---- UIDevice ------------------------------------------------------------------

private func sysctlString(_ name: String) -> String? {
    var buf = [CChar](repeating: 0, count: 256)
    var len = buf.count
    guard sysctlbyname(name, &buf, &len, nil, 0) == 0 else { return nil }
    return String(cString: buf)
}

@objc(UIDevice)
open class UIDevice: NSObject {
    private static let sharedDevice = UIDevice()
    private var _batteryMonitoring = false
    private var _vendorID: UUID?

    @objc(currentDevice)
    public class func currentDevice() -> UIDevice { sharedDevice }

    @objc public var name: String { "iPad" } // iOS 16+ returns the generic model name to apps
    @objc public var model: String { "iPad" }
    @objc public var localizedModel: String { "iPad" }
    @objc public var systemName: String { "iPadOS" }

    @objc public var systemVersion: String {
        if let v = getenv("PEAR_HOST_IOS_VERSION") {
            return String(cString: v)
        }
        return "17.0"
    }

    @objc public var orientation: UIDeviceOrientation { .landscapeLeft }
    @objc(isGeneratingDeviceOrientationNotifications) public func isGeneratingDeviceOrientationNotifications() -> Bool { false }
    @objc public func beginGeneratingDeviceOrientationNotifications() {}
    @objc public func endGeneratingDeviceOrientationNotifications() {}
    @objc public var userInterfaceIdiom: UIUserInterfaceIdiom { .pad }

    @objc public var identifierForVendor: UUID? {
        if _vendorID == nil {
            // Stable per app and per Mac: derived from the hardware UUID and bundle id.
            let seed = "\(sysctlString("kern.uuid") ?? "")|\(Bundle.main.bundleIdentifier ?? "")"
            var bytes = [UInt8](repeating: 0, count: 16)
            let data = Data(seed.utf8)
            for (i, byte) in data.enumerated() {
                bytes[i % 16] = bytes[i % 16] &* 31 &+ byte
            }
            bytes[6] = (bytes[6] & 0x0F) | 0x40
            bytes[8] = (bytes[8] & 0x3F) | 0x80
            _vendorID = NSUUID(uuidBytes: bytes) as UUID
        }
        return _vendorID
    }

    @objc(isBatteryMonitoringEnabled) public func isBatteryMonitoringEnabled() -> Bool { _batteryMonitoring }
    @objc(setBatteryMonitoringEnabled:) public func setBatteryMonitoringEnabled(_ enabled: Bool) { _batteryMonitoring = enabled }

    @objc public var batteryState: UIDeviceBatteryState {
        _batteryMonitoring ? .full : .unknown
    }

    @objc public var batteryLevel: Float {
        _batteryMonitoring ? 1.0 : -1.0
    }

    @objc(isMultitaskingSupported) public func isMultitaskingSupported() -> Bool { true }
    @objc(isProximityMonitoringEnabled) public func isProximityMonitoringEnabled() -> Bool { false }
    @objc(setProximityMonitoringEnabled:) public func setProximityMonitoringEnabled(_ enabled: Bool) {}
    @objc public var proximityState: Bool { false }
    @objc public func playInputClick() {}

    // Read through key-value coding by apps that probe haptics support
    // (observed in GTA VC: valueForKey:@"_feedbackSupportLevel"). 0 = no haptic
    // engine, which is true for a Mac.
    @objc(_feedbackSupportLevel) public func _feedbackSupportLevel() -> Int { 0 }
}

// ---- UIResponder ---------------------------------------------------------------

private weak var gFirstResponder: UIResponder?

@_cdecl("PearUIKitFirstResponder")
public func PearUIKitFirstResponder() -> UIResponder? { gFirstResponder }

@objc(UIResponder)
open class UIResponder: NSObject {
    @objc open var nextResponder: UIResponder? { nil }
    @objc open var canBecomeFirstResponder: Bool { false }
    @objc open var canResignFirstResponder: Bool { true }
    @objc(isFirstResponder) open func isFirstResponder() -> Bool { gFirstResponder === self }

    @objc open func becomeFirstResponder() -> Bool {
        if gFirstResponder === self { return true }
        if !canBecomeFirstResponder { return false }
        if let current = gFirstResponder, !current.resignFirstResponder() { return false }
        gFirstResponder = self
        return true
    }

    @objc open func resignFirstResponder() -> Bool {
        if gFirstResponder === self { gFirstResponder = nil }
        return true
    }

    @objc(touchesBegan:withEvent:)
    open func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        nextResponder?.touchesBegan(touches, with: event)
    }

    @objc(touchesMoved:withEvent:)
    open func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        nextResponder?.touchesMoved(touches, with: event)
    }

    @objc(touchesEnded:withEvent:)
    open func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        nextResponder?.touchesEnded(touches, with: event)
    }

    @objc(touchesCancelled:withEvent:)
    open func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        nextResponder?.touchesCancelled(touches, with: event)
    }

    @objc(pressesBegan:withEvent:)
    open func pressesBegan(_ presses: Set<AnyHashable>, with event: Any?) {
        nextResponder?.pressesBegan(presses, with: event)
    }

    @objc(pressesEnded:withEvent:)
    open func pressesEnded(_ presses: Set<AnyHashable>, with event: Any?) {
        nextResponder?.pressesEnded(presses, with: event)
    }

    @objc(pressesChanged:withEvent:)
    open func pressesChanged(_ presses: Set<AnyHashable>, with event: Any?) {
        nextResponder?.pressesChanged(presses, with: event)
    }

    @objc(pressesCancelled:withEvent:)
    open func pressesCancelled(_ presses: Set<AnyHashable>, with event: Any?) {
        nextResponder?.pressesCancelled(presses, with: event)
    }

    @objc(motionBegan:withEvent:) open func motionBegan(_ motion: Int, with event: Any?) {}
    @objc(motionEnded:withEvent:) open func motionEnded(_ motion: Int, with event: Any?) {}
    @objc(remoteControlReceivedWithEvent:) open func remoteControlReceived(with event: Any?) {}
    @objc open var keyCommands: [Any]? { nil }
    @objc open var inputView: AnyObject? { nil }
    @objc open var inputAccessoryView: AnyObject? { nil }
    @objc open var undoManager: UndoManager? { nil }
    @objc open func reloadInputViews() {}

    @objc(canPerformAction:withSender:)
    open func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        responds(to: action)
    }
}

// ---- UITouch / UIEvent ---------------------------------------------------------

@objc(UITouch)
open class UITouch: NSObject {
    private var _location: CGPoint // in window coordinates
    private var _previous: CGPoint
    private weak var _view: UIView?
    private weak var _window: UIWindow?
    private var _timestamp: TimeInterval
    private var _phase: UITouchPhase

    @objc(initWithView:window:location:timestamp:)
    public init(view: UIView?, window: UIWindow?, location: CGPoint, timestamp: TimeInterval) {
        _view = view
        _window = window
        _location = location
        _previous = location
        _timestamp = timestamp
        _phase = .began
        super.init()
    }

    @objc(pear_moveTo:phase:timestamp:)
    public func pear_move(to point: CGPoint, phase: UITouchPhase, timestamp: TimeInterval) {
        _previous = _location
        _location = point
        _phase = phase
        _timestamp = timestamp
    }

    @objc public var timestamp: TimeInterval { _timestamp }
    @objc public var phase: UITouchPhase { _phase }
    @objc public var tapCount: UInt { 1 }
    @objc public var type: UITouchType { .direct }
    @objc public var force: CGFloat { _phase == .ended ? 0 : 1 }
    @objc public var maximumPossibleForce: CGFloat { 1 }
    @objc public var majorRadius: CGFloat { 20 }
    @objc public var majorRadiusTolerance: CGFloat { 5 }
    @objc public var view: UIView? { _view }
    @objc public var window: UIWindow? { _window }
    @objc public var gestureRecognizers: [Any] { [] }

    @objc(locationInView:)
    public func location(in view: UIView?) -> CGPoint {
        if let view {
            return view.convert(_location, from: _window)
        }
        return _location
    }

    @objc(previousLocationInView:)
    public func previousLocation(in view: UIView?) -> CGPoint {
        if let view {
            return view.convert(_previous, from: _window)
        }
        return _previous
    }

    @objc(preciseLocationInView:)
    public func preciseLocation(in view: UIView?) -> CGPoint { location(in: view) }

    @objc(precisePreviousLocationInView:)
    public func precisePreviousLocation(in view: UIView?) -> CGPoint { previousLocation(in: view) }
}

@objc(UIEvent)
open class UIEvent: NSObject {
    private let _touches: Set<UITouch>
    private let _timestamp: TimeInterval

    @objc(initWithTouches:timestamp:)
    public init(touches: Set<UITouch>, timestamp: TimeInterval) {
        _touches = touches
        _timestamp = timestamp
        super.init()
    }

    @objc open var type: UIEventType { .touches }
    @objc open var subtype: Int { 0 }
    @objc open var timestamp: TimeInterval { _timestamp }
    @objc open func allTouches() -> Set<UITouch>? { _touches }

    @objc(touchesForView:)
    open func touches(for view: UIView?) -> Set<UITouch>? {
        _touches.filter { $0.view === view }
    }

    @objc(touchesForWindow:)
    open func touches(for window: UIWindow?) -> Set<UITouch>? {
        _touches.filter { $0.window === window }
    }

    @objc(coalescedTouchesForTouch:)
    open func coalescedTouches(for touch: UITouch?) -> [UITouch]? {
        touch.map { [$0] }
    }

    @objc(predictedTouchesForTouch:)
    open func predictedTouches(for touch: UITouch?) -> [UITouch]? { [] }
}

// Single pointer -> single touch. Multi-touch synthesis lives in the input
// module above this layer.
private var gActiveTouch: UITouch?

@_cdecl("PearUIKitDeliverPointer")
public func PearUIKitDeliverPointer(_ window: UIWindow, _ phase: UITouchPhase, _ point: CGPoint, _ timestamp: TimeInterval) {
    if phase == .began {
        let target = window.hitTest(point, with: nil) ?? window
        gActiveTouch = UITouch(view: target, window: window, location: point, timestamp: timestamp)
    } else if let active = gActiveTouch {
        active.pear_move(to: point, phase: phase, timestamp: timestamp)
    } else {
        return
    }

    guard let touch = gActiveTouch else { return }
    let set: Set<UITouch> = [touch]
    let event = UIEvent(touches: set, timestamp: timestamp)
    let targetView = touch.view
    switch phase {
    case .began: targetView?.touchesBegan(set, with: event)
    case .moved: targetView?.touchesMoved(set, with: event)
    case .ended: targetView?.touchesEnded(set, with: event)
    case .cancelled: targetView?.touchesCancelled(set, with: event)
    default: break
    }
    if phase == .ended || phase == .cancelled {
        gActiveTouch = nil
    }
}
