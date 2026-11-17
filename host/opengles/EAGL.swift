// PearHost OpenGLES.framework: GL entry points come from PearANGLE (Metal-only),
// and the EAGL / CAEAGLLayer layer runs in Swift on top of EGL.
// Clean-room: from the public EAGL.h / EAGLDrawable.h interfaces and EGL 1.5
// Metal-ANGLE extension specs.
import Darwin
import Foundation
import QuartzCore

private enum EGL {
    static let falseVal: Int32 = 0
    static let trueVal: Int32 = 1
    static let none: Int32 = 0x3038
    static let noContext: UnsafeMutableRawPointer? = nil
    static let noSurface: UnsafeMutableRawPointer? = nil
    static let noDisplay: UnsafeMutableRawPointer? = nil

    static let redSize: Int32 = 0x3024
    static let greenSize: Int32 = 0x3023
    static let blueSize: Int32 = 0x3022
    static let alphaSize: Int32 = 0x3021
    static let surfaceType: Int32 = 0x3033
    static let windowBit: Int32 = 0x0004
    static let pbufferBit: Int32 = 0x0001
    static let renderableType: Int32 = 0x3040
    static let openglES2Bit: Int32 = 0x0004
    static let openglES3Bit: Int32 = 0x0040
    static let width: Int32 = 0x3057
    static let height: Int32 = 0x3056
    static let contextClientVersion: Int32 = 0x3098
    static let openglESAPI: UInt32 = 0x30A0

    static let platformAngleANGLE: Int32 = 0x3202
    static let platformAngleTypeANGLE: Int32 = 0x3203
    static let platformAngleTypeMetalANGLE: Int32 = 0x3489
}

private final class PearEGLBridge {
    static let shared = PearEGLBridge()

    let display: UnsafeMutableRawPointer?
    let config: UnsafeMutableRawPointer?

    private let fnGetPlatformDisplay: (@convention(c) (Int32, UnsafeMutableRawPointer?, UnsafePointer<Int>?) -> UnsafeMutableRawPointer?)?
    private let fnInitialize: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<Int32>?, UnsafeMutablePointer<Int32>?) -> Int32)?
    private let fnBindAPI: (@convention(c) (UInt32) -> Int32)?
    private let fnChooseConfig: (@convention(c) (UnsafeMutableRawPointer?, UnsafePointer<Int32>?, UnsafeMutablePointer<UnsafeMutableRawPointer?>?, Int32, UnsafeMutablePointer<Int32>?) -> Int32)?
    private let fnCreateContext: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<Int32>?) -> UnsafeMutableRawPointer?)?
    private let fnDestroyContext: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Int32)?
    private let fnCreatePbufferSurface: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<Int32>?) -> UnsafeMutableRawPointer?)?
    private let fnCreateWindowSurface: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<Int32>?) -> UnsafeMutableRawPointer?)?
    private let fnDestroySurface: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Int32)?
    private let fnMakeCurrent: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Int32)?
    private let fnSwapBuffers: (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Int32)?

    private init() {
        let handle = dlopen("@rpath/libEGL.dylib", RTLD_LAZY | RTLD_LOCAL)
            ?? dlopen("libEGL.dylib", RTLD_LAZY | RTLD_LOCAL)

        func sym<T>(_ name: String) -> T? {
            guard let handle, let p = dlsym(handle, name) else { return nil }
            return unsafeBitCast(p, to: T.self)
        }

        fnGetPlatformDisplay = sym("eglGetPlatformDisplay")
        fnInitialize = sym("eglInitialize")
        fnBindAPI = sym("eglBindAPI")
        fnChooseConfig = sym("eglChooseConfig")
        fnCreateContext = sym("eglCreateContext")
        fnDestroyContext = sym("eglDestroyContext")
        fnCreatePbufferSurface = sym("eglCreatePbufferSurface")
        fnCreateWindowSurface = sym("eglCreateWindowSurface")
        fnDestroySurface = sym("eglDestroySurface")
        fnMakeCurrent = sym("eglMakeCurrent")
        fnSwapBuffers = sym("eglSwapBuffers")

        var resolvedDisplay: UnsafeMutableRawPointer? = nil
        var resolvedConfig: UnsafeMutableRawPointer? = nil

        if let getDisp = fnGetPlatformDisplay, let initFn = fnInitialize, let bindFn = fnBindAPI, let chooseFn = fnChooseConfig {
            let attribs: [Int] = [Int(EGL.platformAngleTypeANGLE), Int(EGL.platformAngleTypeMetalANGLE), Int(EGL.none)]
            let d = attribs.withUnsafeBufferPointer { buf in
                getDisp(EGL.platformAngleANGLE, nil, buf.baseAddress)
            }
            if d != nil && initFn(d, nil, nil) == EGL.trueVal {
                _ = bindFn(EGL.openglESAPI)
                let cfgAttribs: [Int32] = [
                    EGL.surfaceType, EGL.windowBit | EGL.pbufferBit,
                    EGL.renderableType, EGL.openglES2Bit | EGL.openglES3Bit,
                    EGL.redSize, 8,
                    EGL.greenSize, 8,
                    EGL.blueSize, 8,
                    EGL.alphaSize, 8,
                    EGL.none,
                ]
                var cfg: UnsafeMutableRawPointer? = nil
                var count: Int32 = 0
                if cfgAttribs.withUnsafeBufferPointer({ buf in
                    chooseFn(d, buf.baseAddress, &cfg, 1, &count)
                }) == EGL.trueVal && count > 0 {
                    resolvedDisplay = d
                    resolvedConfig = cfg
                }
            }
        }
        display = resolvedDisplay
        config = resolvedConfig
    }

    var isAvailable: Bool { display != nil && config != nil }

    func createContext(version: Int32, share: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
        guard let display, let config, let fnCreateContext else { return nil }
        let attribs: [Int32] = [EGL.contextClientVersion, version, EGL.none]
        return attribs.withUnsafeBufferPointer { buf in
            fnCreateContext(display, config, share, buf.baseAddress)
        }
    }

    func destroyContext(_ ctx: UnsafeMutableRawPointer?) {
        guard let display, let ctx, let fnDestroyContext else { return }
        _ = fnDestroyContext(display, ctx)
    }

    func createPbuffer(width: Int32, height: Int32) -> UnsafeMutableRawPointer? {
        guard let display, let config, let fnCreatePbufferSurface else { return nil }
        let attribs: [Int32] = [EGL.width, max(1, width), EGL.height, max(1, height), EGL.none]
        return attribs.withUnsafeBufferPointer { buf in
            fnCreatePbufferSurface(display, config, buf.baseAddress)
        }
    }

    func createWindowSurface(layer: CALayer) -> UnsafeMutableRawPointer? {
        guard let display, let config, let fnCreateWindowSurface else { return nil }
        let rawLayer = Unmanaged.passUnretained(layer).toOpaque()
        return fnCreateWindowSurface(display, config, rawLayer, nil)
    }

    func destroySurface(_ surface: UnsafeMutableRawPointer?) {
        guard let display, let surface, let fnDestroySurface else { return }
        _ = fnDestroySurface(display, surface)
    }

    func makeCurrent(surface: UnsafeMutableRawPointer?, context: UnsafeMutableRawPointer?) -> Bool {
        guard let display, let fnMakeCurrent else { return false }
        let s = surface ?? EGL.noSurface
        let c = context ?? EGL.noContext
        return fnMakeCurrent(display, s, s, c) == EGL.trueVal
    }

    func swapBuffers(_ surface: UnsafeMutableRawPointer?) -> Bool {
        guard let display, let surface, let fnSwapBuffers else { return false }
        return fnSwapBuffers(display, surface) == EGL.trueVal
    }
}

private let kCurrentContextKey = "PearHost.EAGLCurrentContext"

@objc(EAGLSharegroup)
open class EAGLSharegroup: NSObject {
    fileprivate var rawEGLContext: UnsafeMutableRawPointer?
}

@objc(EAGLContext)
open class EAGLContext: NSObject {
    private let _api: UInt
    private let _sharegroup: EAGLSharegroup
    private var _eglContext: UnsafeMutableRawPointer?
    private var _defaultSurface: UnsafeMutableRawPointer?
    private var _boundSurface: UnsafeMutableRawPointer?
    @objc open var isMultiThreaded = false

    @objc(initWithAPI:)
    public convenience init?(api: UInt) {
        self.init(api: api, sharegroup: nil)
    }

    @objc(initWithAPI:sharegroup:)
    public init?(api: UInt, sharegroup: EAGLSharegroup?) {
        let sg = sharegroup ?? EAGLSharegroup()
        _api = api
        _sharegroup = sg
        super.init()

        let bridge = PearEGLBridge.shared
        guard bridge.isAvailable else {
            let caller = Thread.callStackReturnAddresses.dropFirst().first.map { UnsafeRawPointer(bitPattern: $0.uintValue) } ?? nil
            PearMissingReport(PearMissingBehavior, "EAGLContext", "initWithAPI: (PearANGLE libEGL.dylib not loaded; returned nil)", caller)
            return nil
        }

        let clientVersion: Int32 = api >= 3 ? 3 : 2
        guard let ctx = bridge.createContext(version: clientVersion, share: sg.rawEGLContext) else {
            return nil
        }
        _eglContext = ctx
        if sg.rawEGLContext == nil {
            sg.rawEGLContext = ctx
        }
        _defaultSurface = bridge.createPbuffer(width: 16, height: 16)
    }

    deinit {
        let bridge = PearEGLBridge.shared
        if EAGLContext.current() === self {
            _ = EAGLContext.setCurrent(nil)
        }
        bridge.destroySurface(_boundSurface)
        bridge.destroySurface(_defaultSurface)
        bridge.destroyContext(_eglContext)
    }

    @objc open var API: UInt { _api }
    @objc open var sharegroup: EAGLSharegroup { _sharegroup }

    @objc(currentContext)
    public class func current() -> EAGLContext? {
        Thread.current.threadDictionary[kCurrentContextKey] as? EAGLContext
    }

    @objc(setCurrentContext:)
    public class func setCurrent(_ context: EAGLContext?) -> Bool {
        let bridge = PearEGLBridge.shared
        guard bridge.isAvailable else { return context == nil }
        if let context {
            let surf = context._boundSurface ?? context._defaultSurface
            guard bridge.makeCurrent(surface: surf, context: context._eglContext) else {
                return false
            }
            Thread.current.threadDictionary[kCurrentContextKey] = context
            return true
        } else {
            _ = bridge.makeCurrent(surface: nil, context: nil)
            Thread.current.threadDictionary.removeObject(forKey: kCurrentContextKey)
            return true
        }
    }

    @objc(renderbufferStorage:fromDrawable:)
    open func renderbufferStorage(_ target: UInt, from drawable: AnyObject?) -> Bool {
        let bridge = PearEGLBridge.shared
        guard bridge.isAvailable else { return false }
        if let layer = drawable as? CALayer {
            bridge.destroySurface(_boundSurface)
            _boundSurface = bridge.createWindowSurface(layer: layer)
            if EAGLContext.current() === self {
                _ = bridge.makeCurrent(surface: _boundSurface ?? _defaultSurface, context: _eglContext)
            }
            return _boundSurface != nil
        }
        bridge.destroySurface(_boundSurface)
        _boundSurface = nil
        if EAGLContext.current() === self {
            _ = bridge.makeCurrent(surface: _defaultSurface, context: _eglContext)
        }
        return true
    }

    @objc(presentRenderbuffer:)
    open func presentRenderbuffer(_ target: UInt) -> Bool {
        let bridge = PearEGLBridge.shared
        guard bridge.isAvailable else { return false }
        return bridge.swapBuffers(_boundSurface ?? _defaultSurface)
    }
}
