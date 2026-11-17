// Clean-room: from the public UIAccessibilityElement, UIPointerInteraction,
// UITextInput (UITextPosition/UITextRange) and UIUserNotificationSettings documentation.
import CoreGraphics
import Foundation

@objc(UIAccessibilityElement)
open class UIAccessibilityElement: UIResponder {
    @objc open weak var accessibilityContainer: AnyObject?
    @objc open var isAccessibilityElement = true
    @objc open var accessibilityLabel: String?
    @objc open var accessibilityHint: String?
    @objc open var accessibilityValue: String?
    @objc open var accessibilityFrame: CGRect = .zero
    @objc open var accessibilityTraits: UInt64 = 0

    @objc(initWithAccessibilityContainer:)
    public init(accessibilityContainer container: Any) {
        self.accessibilityContainer = container as AnyObject
        super.init()
    }
}

@objc(UIPointerInteraction)
open class UIPointerInteraction: NSObject {
    @objc open weak var delegate: AnyObject?
    @objc open weak var view: UIView?
    private var _enabled = true

    @objc(initWithDelegate:)
    public init(delegate: AnyObject?) {
        self.delegate = delegate
        super.init()
    }

    @objc(isEnabled) open func isEnabled() -> Bool { _enabled }
    @objc(setEnabled:) open func setEnabled(_ enabled: Bool) { _enabled = enabled }
    @objc open func invalidate() {}
}

@objc(UITextPosition)
open class UITextPosition: NSObject {}

@objc(UITextRange)
open class UITextRange: NSObject {
    @objc(isEmpty) open func isEmpty() -> Bool { true }
    @objc open var start: UITextPosition { UITextPosition() }
    @objc open var end: UITextPosition { UITextPosition() }
}

@objc(UIUserNotificationAction)
open class UIUserNotificationAction: NSObject, NSCopying, NSMutableCopying {
    @objc open var identifier: String?
    @objc open var title: String?
    @objc open var activationMode: UInt = 0
    @objc open var isAuthenticationRequired = false
    @objc open var isDestructive = false

    @objc(copyWithZone:) public func copy(with zone: NSZone? = nil) -> Any { self }
    @objc(mutableCopyWithZone:) public func mutableCopy(with zone: NSZone? = nil) -> Any {
        let copy = UIMutableUserNotificationAction()
        copy.identifier = identifier
        copy.title = title
        copy.activationMode = activationMode
        copy.isAuthenticationRequired = isAuthenticationRequired
        copy.isDestructive = isDestructive
        return copy
    }
}

@objc(UIMutableUserNotificationAction)
open class UIMutableUserNotificationAction: UIUserNotificationAction {}

@objc(UIUserNotificationCategory)
open class UIUserNotificationCategory: NSObject, NSCopying, NSMutableCopying {
    @objc open var identifier: String?
    private var _actionsByContext: [UInt: [UIUserNotificationAction]] = [:]

    @objc(actionsForContext:)
    open func actions(for context: UInt) -> [UIUserNotificationAction]? {
        _actionsByContext[context]
    }

    @objc(setActions:forContext:)
    open func setActions(_ actions: [UIUserNotificationAction]?, for context: UInt) {
        _actionsByContext[context] = actions
    }

    @objc(copyWithZone:) public func copy(with zone: NSZone? = nil) -> Any { self }
    @objc(mutableCopyWithZone:) public func mutableCopy(with zone: NSZone? = nil) -> Any {
        let copy = UIMutableUserNotificationCategory()
        copy.identifier = identifier
        copy._actionsByContext = _actionsByContext
        return copy
    }
}

@objc(UIMutableUserNotificationCategory)
open class UIMutableUserNotificationCategory: UIUserNotificationCategory {}

@objc(UIUserNotificationSettings)
open class UIUserNotificationSettings: NSObject {
    @objc open var types: UInt
    @objc open var categories: Set<UIUserNotificationCategory>?

    private init(types: UInt, categories: Set<UIUserNotificationCategory>?) {
        self.types = types
        self.categories = categories
        super.init()
    }

    @objc(settingsForTypes:categories:)
    public class func settings(forTypes types: UInt, categories: Set<UIUserNotificationCategory>?) -> UIUserNotificationSettings {
        UIUserNotificationSettings(types: types, categories: categories)
    }
}
