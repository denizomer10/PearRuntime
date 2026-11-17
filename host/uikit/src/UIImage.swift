// Clean-room: from the public UIImage documentation and CoreGraphics/ImageIO.
import AppKit
import CoreGraphics
import Foundation
import ImageIO

@objc(UIImage)
open class UIImage: NSObject {
    private let _cgImage: CGImage?
    private let _scale: CGFloat
    private let _orientation: Int
    private let _renderingMode: Int

    public init(cgImage: CGImage?, scale: CGFloat = 1.0, orientation: Int = 0, renderingMode: Int = 0) {
        _cgImage = cgImage
        _scale = scale > 0 ? scale : 1.0
        _orientation = orientation
        _renderingMode = renderingMode
        super.init()
    }

    @objc(imageWithCGImage:)
    public class func image(withCGImage cgImage: CGImage) -> UIImage {
        UIImage(cgImage: cgImage, scale: 1.0, orientation: 0)
    }

    @objc(imageWithCGImage:scale:orientation:)
    public class func image(withCGImage cgImage: CGImage, scale: CGFloat, orientation: Int) -> UIImage {
        UIImage(cgImage: cgImage, scale: scale, orientation: orientation)
    }

    @objc(imageWithData:)
    public class func image(withData data: Data) -> UIImage? {
        UIImage(data: data, scale: 1.0)
    }

    @objc(imageWithData:scale:)
    public class func image(withData data: Data, scale: CGFloat) -> UIImage? {
        UIImage(data: data, scale: scale)
    }

    @objc(imageWithContentsOfFile:)
    public class func image(withContentsOfFile path: String) -> UIImage? {
        UIImage(contentsOfFile: path)
    }

    @objc(imageNamed:)
    public class func imageNamed(_ name: String) -> UIImage? {
        imageNamed(name, in: nil, compatibleWith: nil)
    }

    @objc(imageNamed:inBundle:compatibleWithTraitCollection:)
    public class func imageNamed(_ name: String, in bundle: Bundle?, compatibleWith traitCollection: UITraitCollection?) -> UIImage? {
        let b = bundle ?? .main
        let ext = (name as NSString).pathExtension
        let base = ext.isEmpty ? name : (name as NSString).deletingPathExtension
        let candidates = ext.isEmpty ? ["png", "jpg", "jpeg", "webp"] : [ext]
        for suffix in ["@3x", "@2x", ""] {
            let scale: CGFloat = suffix == "@3x" ? 3.0 : (suffix == "@2x" ? 2.0 : 1.0)
            for candidateExt in candidates {
                if let path = b.path(forResource: base + suffix, ofType: candidateExt),
                   let data = try? Data(contentsOf: URL(fileURLWithPath: path)),
                   let img = UIImage(data: data, scale: scale) {
                    return img
                }
            }
        }
        return nil
    }

    @objc(initWithCGImage:)
    public convenience init(cgImage: CGImage) {
        self.init(cgImage: cgImage, scale: 1.0, orientation: 0)
    }

    @objc(initWithCGImage:scale:orientation:)
    public convenience init(cgImage: CGImage, scale: CGFloat, orientation: Int) {
        self.init(cgImage: cgImage, scale: scale, orientation: orientation, renderingMode: 0)
    }

    @objc(initWithData:)
    public convenience init?(data: Data) {
        self.init(data: data, scale: 1.0)
    }

    @objc(initWithData:scale:)
    public convenience init?(data: Data, scale: CGFloat) {
        guard let src = CGImageSourceCreateWithData(data as CFData, nil),
              let cg = CGImageSourceCreateImageAtIndex(src, 0, nil) else {
            return nil
        }
        self.init(cgImage: cg, scale: scale, orientation: 0)
    }

    @objc(initWithContentsOfFile:)
    public convenience init?(contentsOfFile path: String) {
        guard let data = try? Data(contentsOf: URL(fileURLWithPath: path)) else { return nil }
        let scale: CGFloat = path.contains("@3x") ? 3.0 : (path.contains("@2x") ? 2.0 : 1.0)
        self.init(data: data, scale: scale)
    }

    @objc(CGImage)
    open var cgImage: CGImage? { _cgImage }

    @objc open var scale: CGFloat { _scale }
    @objc open var imageOrientation: Int { _orientation }
    @objc open var renderingMode: Int { _renderingMode }

    @objc open var size: CGSize {
        guard let cg = _cgImage else { return .zero }
        return CGSize(width: CGFloat(cg.width) / _scale, height: CGFloat(cg.height) / _scale)
    }

    @objc(imageWithRenderingMode:)
    open func withRenderingMode(_ renderingMode: Int) -> UIImage {
        UIImage(cgImage: _cgImage, scale: _scale, orientation: _orientation, renderingMode: renderingMode)
    }

    @objc(resizableImageWithCapInsets:)
    open func resizableImage(withCapInsets capInsets: UIEdgeInsets) -> UIImage {
        self
    }

    @objc(drawInRect:)
    open func draw(in rect: CGRect) {
        guard let cg = _cgImage, let ctx = UIGraphicsGetCurrentContext() else { return }
        ctx.saveGState()
        ctx.translateBy(x: rect.origin.x, y: rect.origin.y + rect.size.height)
        ctx.scaleBy(x: 1, y: -1)
        ctx.draw(cg, in: CGRect(origin: .zero, size: rect.size))
        ctx.restoreGState()
    }
}

private func encodeImage(_ image: UIImage?, uti: CFString, properties: CFDictionary?) -> Data? {
    guard let cg = image?.cgImage else { return nil }
    let data = NSMutableData()
    guard let dest = CGImageDestinationCreateWithData(data, uti, 1, nil) else { return nil }
    CGImageDestinationAddImage(dest, cg, properties)
    guard CGImageDestinationFinalize(dest) else { return nil }
    return data as Data
}

@_cdecl("UIImagePNGRepresentation")
public func UIImagePNGRepresentation(_ image: UIImage?) -> Data? {
    encodeImage(image, uti: "public.png" as CFString, properties: nil)
}

@_cdecl("UIImageJPEGRepresentation")
public func UIImageJPEGRepresentation(_ image: UIImage?, _ compressionQuality: CGFloat) -> Data? {
    let props = [kCGImageDestinationLossyCompressionQuality: compressionQuality] as CFDictionary
    return encodeImage(image, uti: "public.jpeg" as CFString, properties: props)
}
