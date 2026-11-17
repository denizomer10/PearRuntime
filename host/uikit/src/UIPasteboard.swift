// Clean-room: from the public UIPasteboard documentation.
import AppKit
import Foundation

@objc(UIPasteboard)
open class UIPasteboard: NSObject {
    private static let sharedGeneral = UIPasteboard(pasteboard: .general)
    private let _pasteboard: NSPasteboard

    private init(pasteboard: NSPasteboard) {
        _pasteboard = pasteboard
        super.init()
    }

    @objc(generalPasteboard)
    public class func generalPasteboard() -> UIPasteboard { sharedGeneral }

    @objc(pasteboardWithName:create:)
    public class func pasteboard(withName name: String, create: Bool) -> UIPasteboard? {
        UIPasteboard(pasteboard: NSPasteboard(name: NSPasteboard.Name(name)))
    }

    @objc open var name: String { _pasteboard.name.rawValue }
    @objc open var changeCount: Int { _pasteboard.changeCount }

    @objc open var string: String? {
        get { _pasteboard.string(forType: .string) }
        set {
            _pasteboard.clearContents()
            if let newValue {
                _pasteboard.setString(newValue, forType: .string)
            }
        }
    }

    @objc open var strings: [String]? {
        get {
            guard let items = _pasteboard.pasteboardItems else { return nil }
            let result = items.compactMap { $0.string(forType: .string) }
            return result.isEmpty ? nil : result
        }
        set {
            _pasteboard.clearContents()
            if let newValue {
                _pasteboard.writeObjects(newValue as [NSString])
            }
        }
    }

    @objc open var hasStrings: Bool {
        _pasteboard.canReadItem(withDataConformingToTypes: [NSPasteboard.PasteboardType.string.rawValue])
    }

    @objc open var url: URL? {
        get {
            if let str = _pasteboard.string(forType: .URL), let u = URL(string: str) { return u }
            return (_pasteboard.readObjects(forClasses: [NSURL.self], options: nil)?.first as? URL)
        }
        set {
            _pasteboard.clearContents()
            if let newValue {
                _pasteboard.writeObjects([newValue as NSURL])
            }
        }
    }

    @objc open var urls: [URL]? {
        get { _pasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL] }
        set {
            _pasteboard.clearContents()
            if let newValue {
                _pasteboard.writeObjects(newValue as [NSURL])
            }
        }
    }

    @objc open var hasURLs: Bool {
        _pasteboard.canReadObject(forClasses: [NSURL.self], options: nil)
    }

    @objc open var hasImages: Bool {
        _pasteboard.canReadObject(forClasses: [NSImage.self], options: nil)
    }

    @objc(dataForPasteboardType:)
    open func data(forPasteboardType pasteboardType: String) -> Data? {
        _pasteboard.data(forType: NSPasteboard.PasteboardType(pasteboardType))
    }

    @objc(setData:forPasteboardType:)
    open func setData(_ data: Data, forPasteboardType pasteboardType: String) {
        _pasteboard.clearContents()
        _pasteboard.setData(data, forType: NSPasteboard.PasteboardType(pasteboardType))
    }

    @objc(valueForPasteboardType:)
    open func value(forPasteboardType pasteboardType: String) -> Any? {
        if pasteboardType == "public.utf8-plain-text" || pasteboardType == "public.text" {
            return string
        }
        return data(forPasteboardType: pasteboardType)
    }

    @objc(setValue:forPasteboardType:)
    open func setValue(_ value: Any, forPasteboardType pasteboardType: String) {
        if let str = value as? String {
            string = str
        } else if let data = value as? Data {
            setData(data, forPasteboardType: pasteboardType)
        }
    }
}
