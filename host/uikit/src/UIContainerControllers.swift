// Clean-room: from the public UIBarItem, UIBarButtonItem, UINavigationController
// and UITabBarController documentation.
import CoreGraphics
import Foundation

@objc(UIBarItem)
open class UIBarItem: NSObject {
    @objc open var title: String?
    @objc open var image: UIImage?
    @objc open var tag: Int = 0
    private var _enabled = true

    @objc(isEnabled) open func isEnabled() -> Bool { _enabled }
    @objc(setEnabled:) open func setEnabled(_ enabled: Bool) { _enabled = enabled }
}

@objc(UIBarButtonItem)
open class UIBarButtonItem: UIBarItem {
    @objc open var style: Int = 0
    @objc open weak var target: AnyObject?
    @objc open var action: Selector?
    @objc open var customView: UIView?
    @objc open var width: CGFloat = 0

    @objc(initWithTitle:style:target:action:)
    public init(title: String?, style: Int, target: AnyObject?, action: Selector?) {
        super.init()
        self.title = title
        self.style = style
        self.target = target
        self.action = action
    }

    @objc(initWithImage:style:target:action:)
    public init(image: UIImage?, style: Int, target: AnyObject?, action: Selector?) {
        super.init()
        self.image = image
        self.style = style
        self.target = target
        self.action = action
    }

    @objc(initWithBarButtonSystemItem:target:action:)
    public init(barButtonSystemItem systemItem: Int, target: AnyObject?, action: Selector?) {
        super.init()
        self.target = target
        self.action = action
    }

    @objc(initWithCustomView:)
    public init(customView: UIView) {
        super.init()
        self.customView = customView
    }
}

@objc(UINavigationController)
open class UINavigationController: UIViewController {
    private var _viewControllers: [UIViewController] = []
    private var _navigationBarHidden = false
    @objc open weak var delegate: AnyObject?

    @objc(initWithRootViewController:)
    public init(rootViewController: UIViewController) {
        super.init(nibName: nil, bundle: nil)
        _viewControllers = [rootViewController]
    }

    @objc(initWithNibName:bundle:)
    public override init(nibName: String?, bundle: Bundle?) {
        super.init(nibName: nibName, bundle: bundle)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        super.init(coder: coder)
    }

    @objc open var topViewController: UIViewController? { _viewControllers.last }
    @objc open var visibleViewController: UIViewController? { presentedViewController ?? topViewController }
    @objc open var navigationBar: UIView? { nil }

    @objc(isNavigationBarHidden) open func isNavigationBarHidden() -> Bool { _navigationBarHidden }
    @objc(setNavigationBarHidden:) open func setNavigationBarHidden(_ hidden: Bool) { _navigationBarHidden = hidden }
    @objc(setNavigationBarHidden:animated:) open func setNavigationBarHidden(_ hidden: Bool, animated: Bool) {
        _navigationBarHidden = hidden
    }

    @objc open var viewControllers: [UIViewController] {
        get { _viewControllers }
        set { setViewControllers(newValue, animated: false) }
    }

    @objc(setViewControllers:animated:)
    open func setViewControllers(_ viewControllers: [UIViewController], animated: Bool) {
        _viewControllers.last?.viewIfLoaded?.removeFromSuperview()
        _viewControllers = viewControllers
        showTopController(animated: animated)
    }

    @objc(pushViewController:animated:)
    open func pushViewController(_ viewController: UIViewController, animated: Bool) {
        _viewControllers.last?.viewIfLoaded?.removeFromSuperview()
        _viewControllers.append(viewController)
        showTopController(animated: animated)
    }

    @objc(popViewControllerAnimated:)
    @discardableResult
    open func popViewController(animated: Bool) -> UIViewController? {
        guard _viewControllers.count > 1 else { return nil }
        let popped = _viewControllers.removeLast()
        popped.viewIfLoaded?.removeFromSuperview()
        showTopController(animated: animated)
        return popped
    }

    @objc(popToRootViewControllerAnimated:)
    @discardableResult
    open func popToRootViewController(animated: Bool) -> [UIViewController]? {
        guard _viewControllers.count > 1 else { return nil }
        let popped = Array(_viewControllers.dropFirst())
        for vc in popped { vc.viewIfLoaded?.removeFromSuperview() }
        _viewControllers = Array(_viewControllers.prefix(1))
        showTopController(animated: animated)
        return popped
    }

    private func showTopController(animated: Bool) {
        guard isViewLoaded(), let top = _viewControllers.last, let topView = top.view else { return }
        topView.frame = view.bounds
        topView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        top.viewWillAppear(animated)
        view.addSubview(topView)
        top.viewDidAppear(animated)
    }

    open override func viewDidLoad() {
        super.viewDidLoad()
        showTopController(animated: false)
    }
}

@objc(UITabBarController)
open class UITabBarController: UIViewController {
    private var _viewControllers: [UIViewController] = []
    private var _selectedIndex: Int = 0
    @objc open weak var delegate: AnyObject?

    @objc open var viewControllers: [UIViewController]? {
        get { _viewControllers }
        set { setViewControllers(newValue, animated: false) }
    }

    @objc(setViewControllers:animated:)
    open func setViewControllers(_ viewControllers: [UIViewController]?, animated: Bool) {
        _viewControllers.forEach { $0.viewIfLoaded?.removeFromSuperview() }
        _viewControllers = viewControllers ?? []
        _selectedIndex = _viewControllers.isEmpty ? NSNotFound : min(_selectedIndex, _viewControllers.count - 1)
        showSelectedController()
    }

    @objc open var selectedIndex: Int {
        get { _selectedIndex }
        set {
            guard newValue >= 0, newValue < _viewControllers.count else { return }
            selectedViewController?.viewIfLoaded?.removeFromSuperview()
            _selectedIndex = newValue
            showSelectedController()
        }
    }

    @objc open var selectedViewController: UIViewController? {
        get {
            guard _selectedIndex >= 0, _selectedIndex < _viewControllers.count else { return nil }
            return _viewControllers[_selectedIndex]
        }
        set {
            guard let newValue, let idx = _viewControllers.firstIndex(where: { $0 === newValue }) else { return }
            selectedIndex = idx
        }
    }

    @objc open var tabBar: UIView? { nil }

    private func showSelectedController() {
        guard isViewLoaded(), let vc = selectedViewController, let childView = vc.view else { return }
        childView.frame = view.bounds
        childView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(childView)
    }

    open override func viewDidLoad() {
        super.viewDidLoad()
        showSelectedController()
    }
}
