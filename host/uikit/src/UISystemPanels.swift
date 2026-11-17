// Clean-room: from the public UIActivityViewController, UIDocumentPickerViewController,
// UIImagePickerController, UIDocument and UIPopoverController documentation.
import AppKit
import Foundation
import ObjectiveC

@objc(UIActivityViewController)
open class UIActivityViewController: UIViewController {
    private let _activityItems: [Any]
    @objc open var excludedActivityTypes: [String]?
    @objc open var completionWithItemsHandler: ((String?, Bool, [Any]?, Error?) -> Void)?

    @objc(initWithActivityItems:applicationActivities:)
    public init(activityItems: [Any], applicationActivities: [Any]?) {
        _activityItems = activityItems
        super.init(nibName: nil, bundle: nil)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        _activityItems = []
        super.init(coder: coder)
    }

    open override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        let picker = NSSharingServicePicker(items: _activityItems)
        if let contentView = NSApp.keyWindow?.contentView {
            let rect = NSRect(x: contentView.bounds.midX, y: contentView.bounds.midY, width: 1, height: 1)
            picker.show(relativeTo: rect, of: contentView, preferredEdge: .minY)
        } else {
            completionWithItemsHandler?(nil, false, nil, nil)
        }
    }
}

@objc(UIDocumentPickerViewController)
open class UIDocumentPickerViewController: UIViewController {
    @objc open weak var delegate: AnyObject?
    @objc open var allowsMultipleSelection = false
    @objc open var shouldShowFileExtensions = false
    @objc open var directoryURL: URL?
    private let _urls: [URL]
    private let _isSave: Bool

    @objc(initForOpeningContentTypes:asCopy:)
    public init(forOpeningContentTypes contentTypes: [Any], asCopy: Bool) {
        _urls = []
        _isSave = false
        super.init(nibName: nil, bundle: nil)
    }

    @objc(initWithDocumentTypes:inMode:)
    public init(documentTypes allowedUTIs: [String], in mode: Int) {
        _urls = []
        _isSave = (mode == 2 || mode == 3)
        super.init(nibName: nil, bundle: nil)
    }

    @objc(initForExportingURLs:asCopy:)
    public init(forExporting urls: [URL], asCopy: Bool) {
        _urls = urls
        _isSave = true
        super.init(nibName: nil, bundle: nil)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        _urls = []
        _isSave = false
        super.init(coder: coder)
    }

    open override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            if self._isSave {
                let panel = NSSavePanel()
                if let dir = self.directoryURL { panel.directoryURL = dir }
                panel.nameFieldStringValue = self._urls.first?.lastPathComponent ?? "Document"
                let resp = panel.runModal()
                if resp == .OK, let dest = panel.url {
                    if let src = self._urls.first, src != dest {
                        try? FileManager.default.copyItem(at: src, to: dest)
                    }
                    self.notifyPicked([dest])
                } else {
                    self.notifyCancelled()
                }
            } else {
                let panel = NSOpenPanel()
                panel.allowsMultipleSelection = self.allowsMultipleSelection
                if let dir = self.directoryURL { panel.directoryURL = dir }
                let resp = panel.runModal()
                if resp == .OK {
                    self.notifyPicked(panel.urls)
                } else {
                    self.notifyCancelled()
                }
            }
        }
    }

    private func notifyPicked(_ urls: [URL]) {
        guard let del = delegate else { return }
        let sel = NSSelectorFromString("documentPicker:didPickDocumentsAtURLs:")
        if del.responds(to: sel), let method = class_getInstanceMethod(object_getClass(del), sel) {
            let fn = unsafeBitCast(method_getImplementation(method), to: (@convention(c) (AnyObject, Selector, AnyObject, NSArray) -> Void).self)
            fn(del, sel, self, urls as NSArray)
        }
    }

    private func notifyCancelled() {
        guard let del = delegate else { return }
        let sel = NSSelectorFromString("documentPickerWasCancelled:")
        if del.responds(to: sel), let method = class_getInstanceMethod(object_getClass(del), sel) {
            let fn = unsafeBitCast(method_getImplementation(method), to: (@convention(c) (AnyObject, Selector, AnyObject) -> Void).self)
            fn(del, sel, self)
        }
    }
}

@objc(UIImagePickerController)
open class UIImagePickerController: UINavigationController {
    @objc open var sourceType: Int = 0
    @objc open var mediaTypes: [String] = ["public.image"]
    @objc open var allowsEditing = false
    @objc open var showsCameraControls = true

    @objc(isSourceTypeAvailable:)
    public class func isSourceTypeAvailable(_ sourceType: Int) -> Bool {
        sourceType == 0 || sourceType == 2 // photoLibrary or savedPhotosAlbum
    }

    @objc(availableMediaTypesForSourceType:)
    public class func availableMediaTypes(for sourceType: Int) -> [String]? {
        isSourceTypeAvailable(sourceType) ? ["public.image"] : nil
    }
}

@objc(UIDocument)
open class UIDocument: NSObject {
    @objc open var fileURL: URL
    @objc open var localizedName: String { fileURL.lastPathComponent }
    @objc open var documentState: UInt { 0 }

    @objc(initWithFileURL:)
    public init(fileURL url: URL) {
        self.fileURL = url
        super.init()
    }

    @objc(contentsForType:error:)
    open func contents(forType typeName: String) throws -> Any {
        Data()
    }

    @objc(loadFromContents:ofType:error:)
    open func load(fromContents contents: Any, ofType typeName: String?) throws {}

    @objc(openWithCompletionHandler:)
    open func open(completionHandler: ((Bool) -> Void)? = nil) {
        let ok: Bool
        if let data = try? Data(contentsOf: fileURL) {
            ok = (try? load(fromContents: data, ofType: fileURL.pathExtension)) != nil
        } else {
            ok = false
        }
        DispatchQueue.main.async { completionHandler?(ok) }
    }

    @objc(closeWithCompletionHandler:)
    open func close(completionHandler: ((Bool) -> Void)? = nil) {
        DispatchQueue.main.async { completionHandler?(true) }
    }

    @objc(saveToURL:forSaveOperation:completionHandler:)
    open func save(to url: URL, for saveOperation: Int, completionHandler: ((Bool) -> Void)? = nil) {
        var ok = false
        if let data = (try? contents(forType: url.pathExtension)) as? Data {
            ok = (try? data.write(to: url, options: .atomic)) != nil
        }
        DispatchQueue.main.async { completionHandler?(ok) }
    }
}

@objc(UIPopoverController)
open class UIPopoverController: NSObject {
    @objc open var contentViewController: UIViewController
    @objc open weak var delegate: AnyObject?
    @objc open var popoverContentSize: CGSize = CGSize(width: 320, height: 480)
    private var _visible = false

    @objc(initWithContentViewController:)
    public init(contentViewController viewController: UIViewController) {
        self.contentViewController = viewController
        super.init()
    }

    @objc(isPopoverVisible) open func isPopoverVisible() -> Bool { _visible }

    @objc(presentPopoverFromRect:inView:permittedArrowDirections:animated:)
    open func present(from rect: CGRect, in view: UIView, permittedArrowDirections arrowDirections: UInt, animated: Bool) {
        _visible = true
        if let parentVC = view.window?.rootViewController {
            parentVC.present(contentViewController, animated: animated, completion: nil)
        }
    }

    @objc(dismissPopoverAnimated:)
    open func dismiss(animated: Bool) {
        guard _visible else { return }
        _visible = false
        contentViewController.dismiss(animated: animated, completion: nil)
    }
}
