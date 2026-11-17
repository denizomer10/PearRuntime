// Clean-room: from the public UIFeedbackGenerator documentation.
import Foundation

@objc(UIFeedbackGenerator)
open class UIFeedbackGenerator: NSObject {
    @objc public func prepare() {}
}

@objc(UIImpactFeedbackGenerator)
open class UIImpactFeedbackGenerator: UIFeedbackGenerator {
    private var _style: Int = 0

    @objc(initWithStyle:)
    public init(style: Int) {
        _style = style
        super.init()
    }

    @objc public func impactOccurred() {}

    @objc(impactOccurredWithIntensity:)
    public func impactOccurred(intensity: CGFloat) {}
}

@objc(UINotificationFeedbackGenerator)
open class UINotificationFeedbackGenerator: UIFeedbackGenerator {
    @objc(notificationOccurred:)
    public func notificationOccurred(_ notificationType: Int) {}
}

@objc(UISelectionFeedbackGenerator)
open class UISelectionFeedbackGenerator: UIFeedbackGenerator {
    @objc public func selectionChanged() {}
}
