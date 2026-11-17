// Clean-room: from the public UIStoryboard documentation.
import Foundation

@objc(UIStoryboard)
open class UIStoryboard: NSObject {
    private var _name: String
    private var _bundle: Bundle

    private init(name: String, bundle: Bundle) {
        _name = name
        _bundle = bundle
        super.init()
    }

    @objc(storyboardWithName:bundle:)
    public class func storyboard(name: String?, bundle storyboardBundleOrNil: Bundle?) -> UIStoryboard? {
        guard let name, !name.isEmpty else { return nil }
        let bundle = storyboardBundleOrNil ?? .main
        // LaunchScreen storyboards are optional on host; return nil when no compiled storyboard exists.
        guard bundle.path(forResource: name, ofType: "storyboardc") != nil else {
            return nil
        }
        return UIStoryboard(name: name, bundle: bundle)
    }

    @objc public func instantiateInitialViewController() -> UIViewController? {
        let caller = Thread.callStackReturnAddresses.dropFirst().first.map { UnsafeRawPointer(bitPattern: $0.uintValue) } ?? nil
        PearMissingReport(PearMissingBehavior, "UIStoryboard", "instantiateInitialViewController (\(_name).storyboardc)", caller)
        return nil
    }

    @objc(instantiateViewControllerWithIdentifier:)
    public func instantiateViewController(withIdentifier identifier: String) -> UIViewController {
        let caller = Thread.callStackReturnAddresses.dropFirst().first.map { UnsafeRawPointer(bitPattern: $0.uintValue) } ?? nil
        PearMissingReport(PearMissingBehavior, "UIStoryboard", "instantiateViewControllerWithIdentifier:\(identifier)", caller)
        return UIViewController()
    }
}
