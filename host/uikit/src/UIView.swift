// UIView, UIWindow, UIViewController.
// Clean-room: behavior from public UIKit documentation (view/layer
// relationship, responder chain, view controller lifecycle).
import CoreGraphics
import Foundation
import ObjectiveC
import QuartzCore

// ---- UIView --------------------------------------------------------------------

@objc(UIView)
open class UIView: UIResponder, CALayerDelegate {
    private var _layer: CALayer!
    private weak var _superview: UIView?
    private var _subviews: [UIView] = []
    private weak var _window: UIWindow?
    private weak var _viewController: UIViewController?
    private var _backgroundColor: UIColor?
    private var _tag: Int = 0
    var _autoresizing: UIViewAutoresizing = []
    private var _multipleTouch = false
    private var _userInteraction = true
    private var _clearsContext = true
    private var _needsLayout = false

    @objc(layerClass)
    open class func layerClass() -> AnyClass { CALayer.self }

    private func pearCommonInit(frame: CGRect) {
        _userInteraction = true
        _clearsContext = true
        let cls = type(of: self).layerClass() as! CALayer.Type
        _layer = cls.init()
        _layer.delegate = self
        _layer.anchorPoint = CGPoint(x: 0.5, y: 0.5)
        _layer.contentsScale = UIScreen.mainScreen().scale
        _layer.isGeometryFlipped = false
        self.frame = frame
    }

    @objc public override convenience init() {
        self.init(frame: .zero)
    }

    @objc(initWithFrame:)
    public init(frame: CGRect) {
        super.init()
        pearCommonInit(frame: frame)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        super.init()
        pearCommonInit(frame: .zero)
        pearDecodeView(with: coder)
    }

    // Keys as stored in the apps' compiled nibs (see tools/nibdump.py).
    private func pearDecodeView(with c: NSCoder) {
        if c.containsValue(forKey: "UIBounds") { bounds = c.decodeRect(forKey: "UIBounds") }
        if c.containsValue(forKey: "UICenter") { center = c.decodePoint(forKey: "UICenter") }
        if c.containsValue(forKey: "UIAutoresizingMask") {
            _autoresizing = UIViewAutoresizing(rawValue: UInt(c.decodeInteger(forKey: "UIAutoresizingMask")))
        }
        if c.containsValue(forKey: "UIOpaque") { setOpaque(c.decodeBool(forKey: "UIOpaque")) }
        if c.containsValue(forKey: "UIHidden") { setHidden(c.decodeBool(forKey: "UIHidden")) }
        if c.containsValue(forKey: "UIAlpha") { alpha = c.decodeDouble(forKey: "UIAlpha") }
        if c.containsValue(forKey: "UITag") { _tag = c.decodeInteger(forKey: "UITag") }
        if c.containsValue(forKey: "UIClearsContextBeforeDrawing") {
            _clearsContext = c.decodeBool(forKey: "UIClearsContextBeforeDrawing")
        }
        if c.containsValue(forKey: "UIMultipleTouchEnabled") {
            _multipleTouch = c.decodeBool(forKey: "UIMultipleTouchEnabled")
        }
        if c.containsValue(forKey: "UIUserInteractionDisabled") {
            _userInteraction = !c.decodeBool(forKey: "UIUserInteractionDisabled")
        }
        if let bg = c.decodeObject(forKey: "UIBackgroundColor") as? UIColor {
            backgroundColor = bg
        }
        if let subs = c.decodeObject(forKey: "UISubviews") as? [Any] {
            for case let sub as UIView in subs {
                addSubview(sub)
            }
        }
    }

    @objc open var layer: CALayer { _layer }

    // CALayerDelegate: suppress implicit animations, as UIKit does for its layers
    // outside animation blocks.
    @objc(actionForLayer:forKey:)
    open func action(for layer: CALayer, forKey event: String) -> CAAction? {
        NSNull()
    }

    @objc open var frame: CGRect {
        get { _layer.frame }
        set {
            let old = _layer.bounds.size
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            _layer.frame = newValue
            CATransaction.commit()
            if old != newValue.size {
                pearBoundsSizeChanged(from: old)
            }
        }
    }

    @objc open var bounds: CGRect {
        get { _layer.bounds }
        set {
            let old = _layer.bounds.size
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            _layer.bounds = newValue
            CATransaction.commit()
            if old != newValue.size {
                pearBoundsSizeChanged(from: old)
            }
        }
    }

    @objc open var center: CGPoint {
        get { _layer.position }
        set {
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            _layer.position = newValue
            CATransaction.commit()
        }
    }

    private func pearBoundsSizeChanged(from old: CGSize) {
        let now = _layer.bounds.size
        let dw = now.width - old.width
        let dh = now.height - old.height
        for v in _subviews {
            let m = v._autoresizing
            if m.isEmpty { continue }
            var f = v.frame
            if m.contains(.flexibleWidth) { f.size.width += dw }
            else if m.contains(.flexibleLeftMargin) { f.origin.x += dw }
            if m.contains(.flexibleHeight) { f.size.height += dh }
            else if m.contains(.flexibleTopMargin) { f.origin.y += dh }
            v.frame = f
        }
        setNeedsLayout()
    }

    @objc open var superview: UIView? { _superview }
    @objc open var subviews: [UIView] { _subviews }
    @objc open var window: UIWindow? { _window }

    open override var nextResponder: UIResponder? {
        _viewController ?? _superview
    }

    @objc(pear_setViewController:)
    public func pear_setViewController(_ vc: UIViewController?) {
        _viewController = vc
    }

    @objc(pear_setWindow:)
    public func pear_setWindow(_ window: UIWindow?) {
        if _window === window { return }
        willMove(toWindow: window)
        _window = window
        for v in _subviews { v.pear_setWindow(window) }
        didMoveToWindow()
    }

    @objc(insertSubview:atIndex:)
    open func insertSubview(_ view: UIView?, at index: Int) {
        guard let view, view !== self else { return }
        if view._superview != nil { view.removeFromSuperview() }
        view.willMove(toSuperview: self)
        let clamped = max(0, min(index, _subviews.count))
        _subviews.insert(view, at: clamped)
        _layer.insertSublayer(view.layer, at: UInt32(clamped))
        view._superview = self
        view.pear_setWindow(_window)
        view.didMoveToSuperview()
        didAddSubview(view)
        setNeedsLayout()
    }

    @objc(addSubview:)
    open func addSubview(_ view: UIView?) {
        insertSubview(view, at: _subviews.count)
    }

    @objc(bringSubviewToFront:)
    open func bringSubviewToFront(_ view: UIView?) {
        guard let view, view._superview === self else { return }
        _subviews.removeAll { $0 === view }
        _subviews.append(view)
        view.layer.removeFromSuperlayer()
        _layer.addSublayer(view.layer)
    }

    @objc(sendSubviewToBack:)
    open func sendSubviewToBack(_ view: UIView?) {
        guard let view, view._superview === self else { return }
        _subviews.removeAll { $0 === view }
        _subviews.insert(view, at: 0)
        view.layer.removeFromSuperlayer()
        _layer.insertSublayer(view.layer, at: 0)
    }

    @objc open func removeFromSuperview() {
        guard let parent = _superview else { return }
        parent.willRemoveSubview(self)
        willMove(toSuperview: nil)
        parent._subviews.removeAll { $0 === self }
        _layer.removeFromSuperlayer()
        _superview = nil
        pear_setWindow(nil)
        didMoveToSuperview()
    }

    @objc(didAddSubview:) open func didAddSubview(_ subview: UIView) {}
    @objc(willRemoveSubview:) open func willRemoveSubview(_ subview: UIView) {}
    @objc(willMoveToSuperview:) open func willMove(toSuperview newSuperview: UIView?) {}
    @objc open func didMoveToSuperview() {}
    @objc(willMoveToWindow:) open func willMove(toWindow newWindow: UIWindow?) {}
    @objc open func didMoveToWindow() {}

    @objc(isDescendantOfView:)
    open func isDescendant(of view: UIView) -> Bool {
        var cur: UIView? = self
        while let v = cur {
            if v === view { return true }
            cur = v._superview
        }
        return false
    }

    @objc(viewWithTag:)
    open func viewWithTag(_ tag: Int) -> UIView? {
        if _tag == tag { return self }
        for v in _subviews {
            if let hit = v.viewWithTag(tag) { return hit }
        }
        return nil
    }

    @objc open var tag: Int {
        get { _tag }
        set { _tag = newValue }
    }

    @objc open func setNeedsLayout() {
        _needsLayout = true
        DispatchQueue.main.async { [weak self] in
            self?.layoutIfNeeded()
        }
    }

    @objc open func layoutIfNeeded() {
        if !_needsLayout { return }
        _needsLayout = false
        let vc = _viewController
        vc?.viewWillLayoutSubviews()
        layoutSubviews()
        vc?.viewDidLayoutSubviews()
        for v in _subviews { v.layoutIfNeeded() }
    }

    @objc open func layoutSubviews() {}
    @objc open func setNeedsDisplay() { _layer.setNeedsDisplay() }
    @objc(setNeedsDisplayInRect:) open func setNeedsDisplay(_ rect: CGRect) { _layer.setNeedsDisplay(rect) }
    @objc(drawRect:) open func draw(_ rect: CGRect) {}
    @objc(sizeThatFits:) open func sizeThatFits(_ size: CGSize) -> CGSize { bounds.size }

    @objc open func sizeToFit() {
        var f = frame
        f.size = sizeThatFits(f.size)
        frame = f
    }

    @objc open var contentScaleFactor: CGFloat {
        get { _layer.contentsScale }
        set { _layer.contentsScale = newValue }
    }

    @objc(isMultipleTouchEnabled) open func isMultipleTouchEnabled() -> Bool { _multipleTouch }
    @objc(setMultipleTouchEnabled:) open func setMultipleTouchEnabled(_ enabled: Bool) { _multipleTouch = enabled }
    @objc(isExclusiveTouch) open func isExclusiveTouch() -> Bool { false }
    @objc(setExclusiveTouch:) open func setExclusiveTouch(_ exclusive: Bool) {}
    @objc(isUserInteractionEnabled) open func isUserInteractionEnabled() -> Bool { _userInteraction }
    @objc(setUserInteractionEnabled:) open func setUserInteractionEnabled(_ enabled: Bool) { _userInteraction = enabled }
    @objc(isHidden) open func isHidden() -> Bool { _layer.isHidden }
    @objc(setHidden:) open func setHidden(_ hidden: Bool) { _layer.isHidden = hidden }
    @objc(isOpaque) open func isOpaque() -> Bool { _layer.isOpaque }
    @objc(setOpaque:) open func setOpaque(_ opaque: Bool) { _layer.isOpaque = opaque }
    @objc open var alpha: CGFloat {
        get { CGFloat(_layer.opacity) }
        set { _layer.opacity = Float(newValue) }
    }
    @objc open var autoresizingMask: UIViewAutoresizing {
        get { _autoresizing }
        set { _autoresizing = newValue }
    }
    @objc open var autoresizesSubviews: Bool { get { true } set {} }
    @objc open var clearsContextBeforeDrawing: Bool {
        get { _clearsContext }
        set { _clearsContext = newValue }
    }
    @objc open var clipsToBounds: Bool {
        get { _layer.masksToBounds }
        set { _layer.masksToBounds = newValue }
    }
    @objc open var backgroundColor: UIColor? {
        get { _backgroundColor }
        set {
            _backgroundColor = newValue
            _layer.backgroundColor = newValue?.cgColor
        }
    }
    @objc open var contentMode: Int { get { 0 } set {} }
    @objc open var safeAreaInsets: UIEdgeInsets { UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 0) }
    @objc open var layoutMargins: UIEdgeInsets { UIEdgeInsets(top: 8, left: 8, bottom: 8, right: 8) }
    @objc open var traitCollection: UITraitCollection { UITraitCollection.pear_current() }
    @objc open var gestureRecognizers: [Any] { [] }

    @objc(addGestureRecognizer:)
    open func addGestureRecognizer(_ gestureRecognizer: Any?) {
        let caller = Thread.callStackReturnAddresses.dropFirst().first.map { UnsafeRawPointer(bitPattern: $0.uintValue) } ?? nil
        PearMissingReport(PearMissingBehavior, "UIView", "gesture recognizers are ignored", caller)
    }

    @objc(removeGestureRecognizer:) open func removeGestureRecognizer(_ gestureRecognizer: Any?) {}
    @objc open var translatesAutoresizingMaskIntoConstraints: Bool { get { true } set {} }

    // Hit testing and coordinate conversion (frames are layer frames; no transforms
    // beyond what CALayer applies).
    @objc(pointInside:withEvent:)
    open func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        bounds.contains(point)
    }

    @objc(hitTest:withEvent:)
    open func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        if isHidden() || !_userInteraction || alpha < 0.01 { return nil }
        if !self.point(inside: point, with: event) { return nil }
        for v in _subviews.reversed() {
            if let hit = v.hitTest(convert(point, to: v), with: event) {
                return hit
            }
        }
        return self
    }

    @objc(convertPoint:toView:)
    open func convert(_ point: CGPoint, to view: UIView?) -> CGPoint {
        guard let target = view?.layer ?? _window?.layer else { return point }
        return _layer.convert(point, to: target)
    }

    @objc(convertPoint:fromView:)
    open func convert(_ point: CGPoint, from view: UIView?) -> CGPoint {
        guard let source = view?.layer ?? _window?.layer else { return point }
        return _layer.convert(point, from: source)
    }

    @objc(convertRect:toView:)
    open func convert(_ rect: CGRect, to view: UIView?) -> CGRect {
        guard let target = view?.layer ?? _window?.layer else { return rect }
        return _layer.convert(rect, to: target)
    }

    @objc(convertRect:fromView:)
    open func convert(_ rect: CGRect, from view: UIView?) -> CGRect {
        guard let source = view?.layer ?? _window?.layer else { return rect }
        return _layer.convert(rect, from: source)
    }

    // Accessibility: no assistive technology is attached on this host.
    @objc open var isAccessibilityElement: Bool { get { false } set {} }
    @objc open var accessibilityLabel: String? { get { nil } set {} }
    @objc open var accessibilityTraits: UInt64 { get { 0 } set {} }
    @objc open var accessibilityElements: [Any]? { get { nil } set {} }
    @objc(endEditing:) open func endEditing(_ force: Bool) -> Bool { true }
}

// ---- UIScrollView (geometry only; scrolling gestures are not hosted yet) ----

@objc(UIScrollView)
open class UIScrollView: UIView {
    private var _offset: CGPoint = .zero
    private var _contentSize: CGSize = .zero
    private var _inset = UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
    private var _scrollDisabled = false
    private weak var _scrollDelegate: AnyObject?

    @objc open var contentOffset: CGPoint {
        get { _offset }
        set {
            _offset = newValue
            var b = bounds
            b.origin = newValue
            bounds = b
        }
    }

    @objc(setContentOffset:animated:)
    open func setContentOffset(_ contentOffset: CGPoint, animated: Bool) {
        self.contentOffset = contentOffset
    }

    @objc open var contentSize: CGSize {
        get { _contentSize }
        set { _contentSize = newValue }
    }

    @objc open var contentInset: UIEdgeInsets {
        get { _inset }
        set { _inset = newValue }
    }

    @objc(isScrollEnabled) open func isScrollEnabled() -> Bool { !_scrollDisabled }
    @objc(setScrollEnabled:) open func setScrollEnabled(_ enabled: Bool) { _scrollDisabled = !enabled }

    @objc open var delegate: AnyObject? {
        get { _scrollDelegate }
        set { _scrollDelegate = newValue }
    }

    @objc open var showsVerticalScrollIndicator: Bool { get { false } set {} }
    @objc open var showsHorizontalScrollIndicator: Bool { get { false } set {} }
    @objc open var bounces: Bool { get { false } set {} }
}

// ---- UIViewController -----------------------------------------------------------

@objc(UIViewController)
open class UIViewController: UIResponder {
    private var _view: UIView?
    private var _title: String?
    private var _modalStyle: UIModalPresentationStyle = .fullScreen
    private var _edges: UIRectEdge = .all
    private var _presented: UIViewController?
    private weak var _presenting: UIViewController?
    private var _nibName: String?
    private var _nibBundle: Bundle?

    @objc public override convenience init() {
        self.init(nibName: nil, bundle: nil)
    }

    @objc(initWithNibName:bundle:)
    public init(nibName: String?, bundle: Bundle?) {
        _edges = .all
        _nibName = nibName
        _nibBundle = bundle
        super.init()
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        _edges = .all
        _nibName = coder.decodeObject(forKey: "UINibName") as? String
        if coder.containsValue(forKey: "UIKeyEdgesForExtendedLayout") {
            _edges = UIRectEdge(rawValue: UInt(coder.decodeInteger(forKey: "UIKeyEdgesForExtendedLayout")))
        }
        super.init()
        if let v = coder.decodeObject(forKey: "UIView") as? UIView {
            self.view = v
        }
    }

    @objc open var nibName: String? { _nibName }
    @objc open var nibBundle: Bundle { _nibBundle ?? .main }

    // Documented default: load the named nib with self as owner (its "view" outlet
    // sets the view); without a nib, create a plain view.
    @objc open func loadView() {
        if let nibName = _nibName, !nibName.isEmpty,
           let nib = UINib.nib(withNibName: nibName, bundle: nibBundle) {
            nib.instantiate(withOwner: self, options: nil)
            if _view != nil { return }
        }
        self.view = UIView(frame: UIScreen.mainScreen().bounds)
    }

    @objc open var view: UIView! {
        get {
            if _view == nil {
                loadView()
                if _view == nil {
                    self.view = UIView(frame: UIScreen.mainScreen().bounds)
                }
                viewDidLoad()
            }
            return _view
        }
        set {
            _view?.pear_setViewController(nil)
            _view = newValue
            newValue?.pear_setViewController(self)
        }
    }

    @objc open var viewIfLoaded: UIView? { _view }
    @objc(isViewLoaded) open func isViewLoaded() -> Bool { _view != nil }
    @objc open func loadViewIfNeeded() { _ = view }

    open override var nextResponder: UIResponder? {
        _view?.superview ?? _view?.window
    }

    @objc open func viewDidLoad() {}
    @objc(viewWillAppear:) open func viewWillAppear(_ animated: Bool) {}
    @objc(viewDidAppear:) open func viewDidAppear(_ animated: Bool) {}
    @objc(viewWillDisappear:) open func viewWillDisappear(_ animated: Bool) {}
    @objc(viewDidDisappear:) open func viewDidDisappear(_ animated: Bool) {}
    @objc open func viewWillLayoutSubviews() {}
    @objc open func viewDidLayoutSubviews() {}
    @objc open func viewDidUnload() {}
    @objc open func didReceiveMemoryWarning() {}
    @objc(viewWillTransitionToSize:withTransitionCoordinator:)
    open func viewWillTransition(to size: CGSize, with coordinator: Any?) {}
    @objc(traitCollectionDidChange:)
    open func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {}

    @objc open var title: String? {
        get { _title }
        set { _title = newValue }
    }

    @objc open var modalPresentationStyle: UIModalPresentationStyle {
        get { _modalStyle }
        set { _modalStyle = newValue }
    }

    @objc open var edgesForExtendedLayout: UIRectEdge {
        get { _edges }
        set { _edges = newValue }
    }

    @objc open var traitCollection: UITraitCollection { UITraitCollection.pear_current() }
    @objc open var parentViewController: UIViewController? { nil }
    @objc open var childViewControllers: [UIViewController] { [] }
    @objc open var navigationController: AnyObject? { nil }
    @objc open var tabBarController: AnyObject? { nil }

    @objc open var prefersStatusBarHidden: Bool { true }
    @objc open var prefersHomeIndicatorAutoHidden: Bool { true }
    @objc open var prefersPointerLocked: Bool { false }
    @objc open var shouldAutorotate: Bool { true }
    @objc open var supportedInterfaceOrientations: UIInterfaceOrientationMask { .landscape }
    @objc open var preferredInterfaceOrientationForPresentation: UIInterfaceOrientation { .landscapeRight }
    @objc open var preferredScreenEdgesDeferringSystemGestures: UIRectEdge { [] }
    @objc open func setNeedsUpdateOfPrefersPointerLocked() {}
    @objc open func setNeedsUpdateOfHomeIndicatorAutoHidden() {}
    @objc open func setNeedsUpdateOfScreenEdgesDeferringSystemGestures() {}
    @objc open func setNeedsStatusBarAppearanceUpdate() {}

    @objc open var presentedViewController: UIViewController? { _presented }
    @objc open var presentingViewController: UIViewController? { _presenting }

    @objc(pear_setPresenting:)
    public func pear_setPresenting(_ vc: UIViewController?) { _presenting = vc }

    @objc(presentViewController:animated:completion:)
    open func present(_ vc: UIViewController, animated: Bool, completion: (() -> Void)?) {
        if let alert = vc as? UIAlertController {
            alert.pear_present(completion: completion)
            return
        }
        // Modal presentation of arbitrary controllers is not hosted yet; the
        // presented controller's view is layered over the presenter's window.
        let caller = Thread.callStackReturnAddresses.dropFirst().first.map { UnsafeRawPointer(bitPattern: $0.uintValue) } ?? nil
        PearMissingReport(PearMissingBehavior, class_getName(type(of: vc)), "presentViewController: overlays the view only", caller)
        _presented = vc
        vc.pear_setPresenting(self)
        if let w = _view?.window {
            let v = vc.view!
            v.frame = w.bounds
            vc.viewWillAppear(animated)
            w.addSubview(v)
            vc.viewDidAppear(animated)
        }
        if let completion {
            DispatchQueue.main.async(execute: completion)
        }
    }

    @objc(dismissViewControllerAnimated:completion:)
    open func dismiss(animated: Bool, completion: (() -> Void)?) {
        let target = _presented ?? self
        let presenter = _presented != nil ? self : _presenting
        target.viewWillDisappear(animated)
        target.viewIfLoaded?.removeFromSuperview()
        target.viewDidDisappear(animated)
        target.pear_setPresenting(nil)
        presenter?._presented = nil
        if let completion {
            DispatchQueue.main.async(execute: completion)
        }
    }
}

// ---- UIWindow ---------------------------------------------------------------------

@objc(UIWindow)
open class UIWindow: UIView {
    private var _native: UnsafeMutableRawPointer?
    private var _root: UIViewController?
    private var _screen: UIScreen = UIScreen.mainScreen()
    private var _level: CGFloat = 0

    @objc(initWithFrame:)
    public override init(frame: CGRect) {
        super.init(frame: frame)
        _screen = UIScreen.mainScreen()
        pear_setWindow(self)
        PearUIKitRegisterWindow(self)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        super.init(coder: coder)
        _screen = UIScreen.mainScreen()
        // Nib windows are designed for a phone-sized canvas; they fill the screen.
        self.frame = UIScreen.mainScreen().bounds
        pear_setWindow(self)
        PearUIKitRegisterWindow(self)
    }

    @objc(initWithWindowScene:)
    public convenience init(windowScene: Any?) {
        self.init(frame: UIScreen.mainScreen().bounds)
    }

    @objc open override var window: UIWindow? { self }
    open override var nextResponder: UIResponder? { UIApplication.sharedApplication() }

    @objc open var screen: UIScreen {
        get { _screen }
        set { _screen = newValue }
    }

    @objc open var windowLevel: CGFloat {
        get { _level }
        set { _level = newValue }
    }

    @objc open var windowScene: AnyObject? { get { nil } set {} }
    @objc(isKeyWindow) open func isKeyWindow() -> Bool { PearUIKitKeyWindow() === self }

    @objc open var rootViewController: UIViewController? {
        get { _root }
        set {
            if _root === newValue { return }
            _root?.viewIfLoaded?.removeFromSuperview()
            _root = newValue
            if let vc = newValue {
                let v = vc.view!
                v.frame = bounds
                v.autoresizingMask = [.flexibleWidth, .flexibleHeight]
                vc.viewWillAppear(false)
                addSubview(v)
                vc.viewDidAppear(false)
            }
        }
    }

    private func pearEnsureNative() {
        if _native != nil { return }
        _native = PearBackendCreateWindow(self, bounds)
        // The window's layer is positioned by the backend, not by UIKit frames.
        layer.anchorPoint = .zero
        layer.position = .zero
        if let native = _native {
            PearBackendSetRootLayer(native, layer)
        }
    }

    @objc open func makeKeyWindow() {
        pearEnsureNative()
        PearUIKitSetKeyWindow(self)
    }

    @objc open func makeKeyAndVisible() {
        pearEnsureNative()
        setHidden(false)
        if let native = _native {
            PearBackendShowWindow(native, true)
        }
        PearUIKitSetKeyWindow(self)
        setNeedsLayout()
    }

    open override func setHidden(_ hidden: Bool) {
        super.setHidden(hidden)
        if !hidden, let native = _native {
            PearBackendShowWindow(native, false)
        }
    }

    @objc open func becomeKeyWindow() {}
    @objc open func resignKeyWindow() {}
    @objc(sendEvent:) open func sendEvent(_ event: UIEvent) {}
}

@_cdecl("PearUIKitWindowDidResize")
public func PearUIKitWindowDidResize(_ window: UIWindow, _ pointSize: CGSize, _ scale: CGFloat) {
    PearUIKitScreenDidChange(pointSize, scale)
    window.frame = CGRect(origin: .zero, size: pointSize)
    window.layer.anchorPoint = .zero
    window.layer.position = .zero
    let w = Int(pointSize.width.rounded())
    let h = Int(pointSize.height.rounded())
    let s = String(format: "%.1f", scale)
    PearHostLogMessage("host", "window resized to \(w)x\(h) pt @\(s)x")
}
