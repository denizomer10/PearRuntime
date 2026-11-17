// Clean-room: from the public UIGraphics.h / CoreGraphics PDF & bitmap context documentation.
import CoreGraphics
import Foundation

private enum PearGraphicsEntry {
    case image(context: CGContext, scale: CGFloat)
    case pdf(context: CGContext, inPage: Bool)
    case external(context: CGContext)

    var context: CGContext {
        switch self {
        case .image(let ctx, _), .pdf(let ctx, _), .external(let ctx):
            return ctx
        }
    }
}

private var gContextStack: [PearGraphicsEntry] = []

@_cdecl("UIGraphicsGetCurrentContext")
public func UIGraphicsGetCurrentContext() -> CGContext? {
    gContextStack.last?.context
}

@_cdecl("UIGraphicsPushContext")
public func UIGraphicsPushContext(_ context: CGContext) {
    gContextStack.append(.external(context: context))
}

@_cdecl("UIGraphicsPopContext")
public func UIGraphicsPopContext() {
    if !gContextStack.isEmpty {
        gContextStack.removeLast()
    }
}

@_cdecl("UIGraphicsBeginImageContextWithOptions")
public func UIGraphicsBeginImageContextWithOptions(_ size: CGSize, _ opaque: Bool, _ scale: CGFloat) {
    let resolvedScale = scale > 0 ? scale : UIScreen.mainScreen().scale
    let width = max(1, Int((size.width * resolvedScale).rounded(.up)))
    let height = max(1, Int((size.height * resolvedScale).rounded(.up)))
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    let alpha: CGImageAlphaInfo = opaque ? .noneSkipLast : .premultipliedLast
    let bitmapInfo = CGBitmapInfo.byteOrder32Big.rawValue | alpha.rawValue
    guard let ctx = CGContext(
        data: nil,
        width: width,
        height: height,
        bitsPerComponent: 8,
        bytesPerRow: width * 4,
        space: colorSpace,
        bitmapInfo: bitmapInfo
    ) else {
        return
    }
    ctx.translateBy(x: 0, y: CGFloat(height))
    ctx.scaleBy(x: resolvedScale, y: -resolvedScale)
    gContextStack.append(.image(context: ctx, scale: resolvedScale))
}

@_cdecl("UIGraphicsBeginImageContext")
public func UIGraphicsBeginImageContext(_ size: CGSize) {
    UIGraphicsBeginImageContextWithOptions(size, false, 1.0)
}

@_cdecl("UIGraphicsGetImageFromCurrentImageContext")
public func UIGraphicsGetImageFromCurrentImageContext() -> UIImage? {
    guard case .image(let ctx, let scale) = gContextStack.last,
          let cg = ctx.makeImage() else {
        return nil
    }
    return UIImage(cgImage: cg, scale: scale, orientation: 0)
}

@_cdecl("UIGraphicsEndImageContext")
public func UIGraphicsEndImageContext() {
    if case .image = gContextStack.last {
        gContextStack.removeLast()
    }
}

@_cdecl("UIGraphicsBeginPDFContextToFile")
public func UIGraphicsBeginPDFContextToFile(_ path: NSString, _ bounds: CGRect, _ documentInfo: NSDictionary?) -> Bool {
    let url = URL(fileURLWithPath: path as String) as CFURL
    var mediaBox = bounds.isEmpty ? CGRect(x: 0, y: 0, width: 612, height: 792) : bounds
    guard let ctx = CGContext(url, mediaBox: &mediaBox, documentInfo) else {
        return false
    }
    gContextStack.append(.pdf(context: ctx, inPage: false))
    return true
}

@_cdecl("UIGraphicsBeginPDFContextToData")
public func UIGraphicsBeginPDFContextToData(_ data: NSMutableData, _ bounds: CGRect, _ documentInfo: NSDictionary?) {
    guard let consumer = CGDataConsumer(data: data as CFMutableData) else { return }
    var mediaBox = bounds.isEmpty ? CGRect(x: 0, y: 0, width: 612, height: 792) : bounds
    guard let ctx = CGContext(consumer: consumer, mediaBox: &mediaBox, documentInfo) else { return }
    gContextStack.append(.pdf(context: ctx, inPage: false))
}

@_cdecl("UIGraphicsBeginPDFPage")
public func UIGraphicsBeginPDFPage() {
    UIGraphicsBeginPDFPageWithInfo(.zero, nil)
}

@_cdecl("UIGraphicsBeginPDFPageWithInfo")
public func UIGraphicsBeginPDFPageWithInfo(_ bounds: CGRect, _ pageInfo: NSDictionary?) {
    guard case .pdf(let ctx, let inPage) = gContextStack.last else { return }
    if inPage {
        ctx.endPDFPage()
    }
    var info = (pageInfo as? [CFString: Any]) ?? [:]
    if !bounds.isEmpty {
        var box = bounds
        info[kCGPDFContextMediaBox] = NSData(bytes: &box, length: MemoryLayout<CGRect>.size)
    }
    ctx.beginPDFPage(info as CFDictionary)
    gContextStack[gContextStack.count - 1] = .pdf(context: ctx, inPage: true)
}

@_cdecl("UIGraphicsEndPDFContext")
public func UIGraphicsEndPDFContext() {
    guard case .pdf(let ctx, let inPage) = gContextStack.last else { return }
    if inPage {
        ctx.endPDFPage()
    }
    ctx.closePDF()
    gContextStack.removeLast()
}
