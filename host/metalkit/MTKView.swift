// PearHost MetalKit.framework: MTKView as the UIView subclass iOS apps expect
// (macOS MetalKit's MTKView is an NSView). Clean-room: behavior from the public
// MTKView.h interface and Apple's MetalKit documentation: a CAMetalLayer-backed
// view that sizes its drawable from its bounds, owns the depth/stencil and
// multisample textures, builds a render pass descriptor per frame and drives
// drawing from a display link (or on demand).
import Foundation
import Metal
import PearUIKitSwift
import QuartzCore

@objc(MTKViewDelegate)
public protocol MTKViewDelegate: NSObjectProtocol {
    @objc(mtkView:drawableSizeWillChange:)
    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize)

    @objc(drawInMTKView:)
    func draw(in view: MTKView)
}

@objc(MTKView)
open class MTKView: UIView {
    private var _device: MTLDevice?
    private weak var _delegate: MTKViewDelegate?
    private var _currentDrawable: CAMetalDrawable?
    private var _passDescriptor: MTLRenderPassDescriptor?
    private var _depthTexture: MTLTexture?
    private var _msaaTexture: MTLTexture?
    private var _depthFormat: MTLPixelFormat = .invalid
    private var _sampleCount: Int = 1
    private var _clearColor: MTLClearColor = MTLClearColorMake(0, 0, 0, 1)
    private var _clearDepth: Double = 1.0
    private var _clearStencil: UInt32 = 0
    private var _fps: Int = 60
    private var _paused: Bool = false
    private var _enableSetNeedsDisplay: Bool = false
    private var _autoResize: Bool = true
    private var _needsDisplay: Bool = false
    private var _link: CADisplayLink?

    open override class func layerClass() -> AnyClass {
        CAMetalLayer.self
    }

    private var metalLayer: CAMetalLayer {
        layer as! CAMetalLayer
    }

    private func pearSetup(device: MTLDevice?) {
        _sampleCount = 1
        _clearColor = MTLClearColorMake(0, 0, 0, 1)
        _clearDepth = 1.0
        _fps = 60
        _autoResize = true
        let l = metalLayer
        l.pixelFormat = .bgra8Unorm
        l.framebufferOnly = true
        self.device = device
        self.contentScaleFactor = UIScreen.mainScreen().scale
    }

    @objc(initWithFrame:)
    public override convenience init(frame: CGRect) {
        self.init(frame: frame, device: nil)
    }

    @objc(initWithFrame:device:)
    public init(frame: CGRect, device: MTLDevice?) {
        super.init(frame: frame)
        pearSetup(device: device)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        super.init(coder: coder)
        pearSetup(device: nil)
    }

    deinit {
        _link?.invalidate()
    }

    @objc public var device: MTLDevice? {
        get { _device }
        set {
            _device = newValue
            metalLayer.device = newValue
            _depthTexture = nil
            _msaaTexture = nil
        }
    }

    @objc public var preferredDevice: MTLDevice? {
        MTLCreateSystemDefaultDevice()
    }

    @objc public weak var delegate: MTKViewDelegate? {
        get { _delegate }
        set {
            _delegate = newValue
            pearUpdateLoop()
        }
    }

    @objc public var colorPixelFormat: MTLPixelFormat {
        get { metalLayer.pixelFormat }
        set {
            metalLayer.pixelFormat = newValue
            _msaaTexture = nil
        }
    }

    @objc public var depthStencilPixelFormat: MTLPixelFormat {
        get { _depthFormat }
        set {
            _depthFormat = newValue
            _depthTexture = nil
        }
    }

    @objc public var sampleCount: Int {
        get { _sampleCount }
        set {
            _sampleCount = max(1, newValue)
            _depthTexture = nil
            _msaaTexture = nil
        }
    }

    @objc public var clearColor: MTLClearColor {
        get { _clearColor }
        set { _clearColor = newValue }
    }

    @objc public var clearDepth: Double {
        get { _clearDepth }
        set { _clearDepth = newValue }
    }

    @objc public var clearStencil: UInt32 {
        get { _clearStencil }
        set { _clearStencil = newValue }
    }

    @objc public var framebufferOnly: Bool {
        get { metalLayer.framebufferOnly }
        set { metalLayer.framebufferOnly = newValue }
    }

    @objc public var presentsWithTransaction: Bool {
        get { metalLayer.presentsWithTransaction }
        set { metalLayer.presentsWithTransaction = newValue }
    }

    @objc public var colorspace: CGColorSpace? {
        get { metalLayer.colorspace }
        set { metalLayer.colorspace = newValue }
    }

    @objc public var depthStencilAttachmentTextureUsage: MTLTextureUsage {
        .renderTarget
    }

    @objc public var preferredFramesPerSecond: Int {
        get { _fps }
        set {
            _fps = newValue
            pearUpdateLoop()
        }
    }

    @objc(isPaused) public func isPaused() -> Bool { _paused }
    @objc(setPaused:) public func setPaused(_ paused: Bool) {
        _paused = paused
        pearUpdateLoop()
    }

    @objc public var enableSetNeedsDisplay: Bool {
        get { _enableSetNeedsDisplay }
        set {
            _enableSetNeedsDisplay = newValue
            pearUpdateLoop()
        }
    }

    @objc public var autoResizeDrawable: Bool {
        get { _autoResize }
        set { _autoResize = newValue }
    }

    @objc public var drawableSize: CGSize {
        get { metalLayer.drawableSize }
        set {
            if newValue == metalLayer.drawableSize { return }
            metalLayer.drawableSize = newValue
            _depthTexture = nil
            _msaaTexture = nil
            _delegate?.mtkView(self, drawableSizeWillChange: newValue)
        }
    }

    @objc public var preferredDrawableSize: CGSize {
        let s = contentScaleFactor
        return CGSize(width: bounds.size.width * s, height: bounds.size.height * s)
    }

    // Drawable follows the view's size, as documented for autoResizeDrawable.
    open override func layoutSubviews() {
        super.layoutSubviews()
        pearResizeDrawable()
    }

    open override var frame: CGRect {
        get { super.frame }
        set {
            super.frame = newValue
            pearResizeDrawable()
        }
    }

    open override var bounds: CGRect {
        get { super.bounds }
        set {
            super.bounds = newValue
            pearResizeDrawable()
        }
    }

    open override var contentScaleFactor: CGFloat {
        get { super.contentScaleFactor }
        set {
            super.contentScaleFactor = newValue
            pearResizeDrawable()
        }
    }

    private func pearResizeDrawable() {
        guard _autoResize else { return }
        let want = preferredDrawableSize
        if want.width < 1 || want.height < 1 { return }
        drawableSize = want
    }

    open override func didMoveToWindow() {
        super.didMoveToWindow()
        pearResizeDrawable()
        pearUpdateLoop()
    }

    // ---- frame loop -----------------------------------------------------------------

    private func pearUpdateLoop() {
        let want = (window != nil) && !_paused && !_enableSetNeedsDisplay
        if want && _link == nil {
            let link = UIScreen.mainScreen().displayLink(withTarget: self, selector: #selector(pearTick(_:)))
            link?.add(to: .main, forMode: .common)
            _link = link
        } else if !want, let link = _link {
            link.invalidate()
            _link = nil
        }
        if let link = _link, _fps > 0 {
            let fps = Float(_fps)
            link.preferredFrameRateRange = CAFrameRateRange(minimum: fps, maximum: fps, preferred: fps)
        }
    }

    @objc(pear_tick:)
    private func pearTick(_ link: CADisplayLink) {
        draw()
    }

    open override func setNeedsDisplay() {
        super.setNeedsDisplay()
        if _enableSetNeedsDisplay && !_needsDisplay {
            _needsDisplay = true
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self._needsDisplay = false
                self.draw()
            }
        }
    }

    @objc public func draw() {
        autoreleasepool {
            _currentDrawable = nil
            _passDescriptor = nil
            if let d = _delegate {
                d.draw(in: self)
            } else {
                draw(bounds)
            }
            _currentDrawable = nil
            _passDescriptor = nil
        }
    }

    @objc public func releaseDrawables() {
        _depthTexture = nil
        _msaaTexture = nil
        _currentDrawable = nil
    }

    @objc public var currentDrawable: CAMetalDrawable? {
        if _currentDrawable == nil {
            _currentDrawable = metalLayer.nextDrawable()
        }
        return _currentDrawable
    }

    private func pearTexture(format: MTLPixelFormat, samples: Int, usage: MTLTextureUsage) -> MTLTexture? {
        let size = drawableSize
        guard let device = _device, size.width >= 1, size.height >= 1 else { return nil }
        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: format,
            width: Int(size.width),
            height: Int(size.height),
            mipmapped: false
        )
        desc.textureType = samples > 1 ? .type2DMultisample : .type2D
        desc.sampleCount = samples
        desc.usage = usage
        desc.storageMode = .private
        return device.makeTexture(descriptor: desc)
    }

    @objc public var depthStencilTexture: MTLTexture? {
        if _depthFormat == .invalid { return nil }
        if _depthTexture == nil {
            _depthTexture = pearTexture(format: _depthFormat, samples: _sampleCount, usage: .renderTarget)
        }
        return _depthTexture
    }

    @objc public var multisampleColorTexture: MTLTexture? {
        if _sampleCount <= 1 { return nil }
        if _msaaTexture == nil {
            _msaaTexture = pearTexture(format: colorPixelFormat, samples: _sampleCount, usage: .renderTarget)
        }
        return _msaaTexture
    }

    @objc public var currentRenderPassDescriptor: MTLRenderPassDescriptor? {
        if let passDescriptor = _passDescriptor { return passDescriptor }
        guard let drawable = currentDrawable else { return nil }
        let p = MTLRenderPassDescriptor()
        if let c = p.colorAttachments[0] {
            if _sampleCount > 1 {
                c.texture = multisampleColorTexture
                c.resolveTexture = drawable.texture
                c.storeAction = .multisampleResolve
            } else {
                c.texture = drawable.texture
                c.storeAction = .store
            }
            c.loadAction = .clear
            c.clearColor = _clearColor
        }
        if let depth = depthStencilTexture {
            p.depthAttachment.texture = depth
            p.depthAttachment.loadAction = .clear
            p.depthAttachment.storeAction = .dontCare
            p.depthAttachment.clearDepth = _clearDepth
            if _depthFormat == .depth32Float_stencil8 || _depthFormat == .stencil8 || _depthFormat == .x32_stencil8 {
                p.stencilAttachment.texture = depth
                p.stencilAttachment.loadAction = .clear
                p.stencilAttachment.storeAction = .dontCare
                p.stencilAttachment.clearStencil = _clearStencil
            }
        }
        _passDescriptor = p
        return p
    }
}
