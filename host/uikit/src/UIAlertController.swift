// UIAlertController / UIAlertAction, presented as a native alert by the backend.
// Clean-room: from the public UIAlertController documentation.
import Foundation

@objc(UIAlertAction)
open class UIAlertAction: NSObject, NSCopying {
    private var _title: String?
    private var _style: Int = 0
    private var _handler: ((UIAlertAction) -> Void)?
    private var _enabled: Bool = true

    public required override init() {
        super.init()
    }

    @objc(actionWithTitle:style:handler:)
    public class func action(title: String?, style: Int, handler: ((UIAlertAction) -> Void)?) -> Self {
        let action = self.init()
        action._title = title
        action._style = style
        action._handler = handler
        action._enabled = true
        return action
    }

    @objc public var title: String? { _title }
    @objc public var style: Int { _style }

    @objc(isEnabled) public func isEnabled() -> Bool { _enabled }
    @objc(setEnabled:) public func setEnabled(_ enabled: Bool) { _enabled = enabled }

    @objc(pear_fire)
    public func pear_fire() {
        _handler?(self)
    }

    @objc(copyWithZone:)
    public func copy(with zone: NSZone? = nil) -> Any {
        self
    }
}

@objc(UIAlertController)
open class UIAlertController: UIViewController {
    private var _message: String?
    private var _preferredStyle: Int = 0
    private var _actions: [UIAlertAction] = []
    private var _preferredAction: UIAlertAction?

    @objc(alertControllerWithTitle:message:preferredStyle:)
    public class func alertController(title: String?, message: String?, preferredStyle: Int) -> UIAlertController {
        let controller = UIAlertController()
        controller.title = title
        controller._message = message
        controller._preferredStyle = preferredStyle
        return controller
    }

    @objc public var message: String? {
        get { _message }
        set { _message = newValue }
    }

    @objc public var preferredStyle: Int { _preferredStyle }
    @objc public var actions: [UIAlertAction] { _actions }

    @objc public var preferredAction: UIAlertAction? {
        get { _preferredAction }
        set { _preferredAction = newValue }
    }

    @objc(addAction:)
    public func addAction(_ action: UIAlertAction?) {
        if let action {
            _actions.append(action)
        }
    }

    @objc public var textFields: [Any] { [] }

    @objc(addTextFieldWithConfigurationHandler:)
    public func addTextField(configurationHandler: ((Any?) -> Void)?) {
        let caller = Thread.callStackReturnAddresses.dropFirst().first.map { UnsafeRawPointer(bitPattern: $0.uintValue) } ?? nil
        PearMissingReport(PearMissingBehavior, "UIAlertController", "text fields in alerts", caller)
    }

    @objc public var popoverPresentationController: AnyObject? { nil }

    @objc(pear_presentWithCompletion:)
    public func pear_present(completion: (() -> Void)?) {
        let titles = _actions.map { $0.title ?? "OK" }
        let titleStr = self.title ?? ""
        let messageStr = _message ?? ""
        let actionsSummary = titles.joined(separator: ", ")
        PearHostLogMessage("alert", "title=\"\(titleStr)\" message=\"\(messageStr)\" actions=[\(actionsSummary)]")
        let snapshot = _actions
        PearBackendPresentAlert(titleStr, messageStr, titles) { index in
            if index >= 0 && index < snapshot.count {
                snapshot[index].pear_fire()
            }
        }
        if let completion {
            DispatchQueue.main.async(execute: completion)
        }
    }
}
