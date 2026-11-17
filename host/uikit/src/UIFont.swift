// Clean-room: from the public UIFont / CoreText CTFont documentation.
import AppKit
import CoreText
import Foundation

@objc(UIFont)
open class UIFont: NSObject, NSCopying {
    private let _ctFont: CTFont

    public init(ctFont: CTFont) {
        _ctFont = ctFont
        super.init()
    }

    public var ctFont: CTFont { _ctFont }

    @objc(systemFontOfSize:)
    public class func systemFont(ofSize size: CGFloat) -> UIFont {
        UIFont(ctFont: NSFont.systemFont(ofSize: size))
    }

    @objc(systemFontOfSize:weight:)
    public class func systemFont(ofSize size: CGFloat, weight: CGFloat) -> UIFont {
        UIFont(ctFont: NSFont.systemFont(ofSize: size, weight: NSFont.Weight(rawValue: weight)))
    }

    @objc(boldSystemFontOfSize:)
    public class func boldSystemFont(ofSize size: CGFloat) -> UIFont {
        UIFont(ctFont: NSFont.boldSystemFont(ofSize: size))
    }

    @objc(italicSystemFontOfSize:)
    public class func italicSystemFont(ofSize size: CGFloat) -> UIFont {
        let base = NSFont.systemFont(ofSize: size)
        let desc = base.fontDescriptor.withSymbolicTraits(.italic)
        let italic = NSFont(descriptor: desc, size: size) ?? base
        return UIFont(ctFont: italic)
    }

    @objc(fontWithName:size:)
    public class func font(withName fontName: String, size fontSize: CGFloat) -> UIFont? {
        guard let nsFont = NSFont(name: fontName, size: fontSize) else { return nil }
        return UIFont(ctFont: nsFont)
    }

    @objc(fontWithDescriptor:size:)
    public class func font(with descriptor: NSFontDescriptor, size fontSize: CGFloat) -> UIFont {
        let font = NSFont(descriptor: descriptor, size: fontSize) ?? NSFont.systemFont(ofSize: fontSize)
        return UIFont(ctFont: font)
    }

    @objc public class var familyNames: [String] {
        NSFontManager.shared.availableFontFamilies
    }

    @objc(fontNamesForFamilyName:)
    public class func fontNames(forFamilyName familyName: String) -> [String] {
        (NSFontManager.shared.availableMembers(ofFontFamily: familyName)?.compactMap { $0.first as? String }) ?? []
    }

    @objc open var familyName: String { CTFontCopyFamilyName(_ctFont) as String }
    @objc open var fontName: String { CTFontCopyPostScriptName(_ctFont) as String }
    @objc open var pointSize: CGFloat { CTFontGetSize(_ctFont) }
    @objc open var ascender: CGFloat { CTFontGetAscent(_ctFont) }
    @objc open var descender: CGFloat { -CTFontGetDescent(_ctFont) }
    @objc open var capHeight: CGFloat { CTFontGetCapHeight(_ctFont) }
    @objc open var xHeight: CGFloat { CTFontGetXHeight(_ctFont) }
    @objc open var leading: CGFloat { CTFontGetLeading(_ctFont) }
    @objc open var lineHeight: CGFloat { ascender - descender + leading }

    @objc(fontWithSize:)
    open func withSize(_ fontSize: CGFloat) -> UIFont {
        let copy = CTFontCreateCopyWithAttributes(_ctFont, fontSize, nil, nil)
        return UIFont(ctFont: copy)
    }

    @objc(copyWithZone:)
    public func copy(with zone: NSZone? = nil) -> Any { self }
}
