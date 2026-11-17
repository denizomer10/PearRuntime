// macOS backend for PearHost UIKit: native windows, screen metrics, display
// link, run loop and pointer input on AppKit. Written from the public AppKit
// and QuartzCore interfaces.
import AppKit
import Foundation
import IOKit.pwr_mgt
import ObjectiveC
import QuartzCore

private var gNativeWindows: [NSWindow] = [] // keeps NSWindows alive
private var gPrimaryView: PearHostView?
private var gScreen = PearHostScreenInfo(pointSize: .zero, scale: 2.0, maxFPS: 60)
private var gIdleAssertion: IOPMAssertionID = 0
private var gAppDelegate: PearHostAppDelegate?

private func defaultPointSize() -> CGSize {
    if let envPtr = getenv("PEAR_HOST_SCREEN") {
        let parts = String(cString: envPtr).split(separator: "x")
        if parts.count == 2, let w = Int(parts[0]), let h = Int(parts[1]), w > 0, h > 0 {
            return CGSize(width: w, height: h)
        }
    }
    let vis = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1280, height: 800)
    var width = floor(vis.size.width * 0.8)
    var height = floor(width * 9.0 / 16.0)
    if height > vis.size.height * 0.85 {
        height = floor(vis.size.height * 0.85)
        width = floor(height * 16.0 / 9.0)
    }
    return CGSize(width: width, height: height)
}

@_cdecl("PearBackendInitialize")
public func PearBackendInitialize() {
    _ = NSApplication.shared
    NSApp.setActivationPolicy(.regular)
    gNativeWindows = []
    let mainScreen = NSScreen.main
    let scale = mainScreen?.backingScaleFactor ?? 2.0
    let maxFPS = mainScreen?.maximumFramesPerSecond ?? 60
    gScreen = PearHostScreenInfo(
        pointSize: defaultPointSize(),
        scale: scale > 0 ? scale : 2.0,
        maxFPS: maxFPS > 0 ? maxFPS : 60
    )
    PearBackendInstallQuartzCoreCompat()

    let nc = NotificationCenter.default
    nc.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil, queue: nil) { _ in
        PearUIKitApplicationDidBecomeActive()
    }
    nc.addObserver(forName: NSApplication.willResignActiveNotification, object: nil, queue: nil) { _ in
        PearUIKitApplicationWillResignActive()
    }
    nc.addObserver(forName: NSApplication.didHideNotification, object: nil, queue: nil) { _ in
        PearUIKitApplicationDidEnterBackground()
    }
    nc.addObserver(forName: NSApplication.willUnhideNotification, object: nil, queue: nil) { _ in
        PearUIKitApplicationWillEnterForeground()
    }
    nc.addObserver(forName: NSApplication.willTerminateNotification, object: nil, queue: nil) { _ in
        PearUIKitApplicationWillTerminate()
    }

    let w = Int(gScreen.pointSize.width.rounded())
    let h = Int(gScreen.pointSize.height.rounded())
    let s = String(format: "%.1f", gScreen.scale)
    PearHostLogMessage("host", "AppKit backend: screen \(w)x\(h) pt @\(s)x, \(gScreen.maxFPS) Hz")
}

@_cdecl("PearBackendScreenInfo")
public func PearBackendScreenInfo() -> PearHostScreenInfo {
    gScreen
}

@_cdecl("PearBackendCreateWindow")
public func PearBackendCreateWindow(_ window: UIWindow, _ frame: CGRect) -> UnsafeMutableRawPointer {
    let content = NSRect(x: 0, y: 0, width: frame.size.width, height: frame.size.height)
    let w = NSWindow(
        contentRect: content,
        styleMask: [.titled, .closable, .miniaturizable, .resizable],
        backing: .buffered,
        defer: false
    )
    let displayName = Bundle.main.infoDictionary?["CFBundleDisplayName"] as? String
    w.title = displayName ?? ProcessInfo.processInfo.processName
    w.isReleasedWhenClosed = false
    w.acceptsMouseMovedEvents = true
    w.collectionBehavior.insert(.fullScreenPrimary)
    w.contentAspectRatio = content.size

    let v = PearHostView(frame: content)
    v.uiWindow = window
    w.contentView = v
    w.makeFirstResponder(v) // key events must reach the host view

    let d = PearHostWindowDelegate()
    d.uiWindow = window
    w.delegate = d
    objc_setAssociatedObject(w, "pear.delegate", d, .OBJC_ASSOCIATION_RETAIN)

    w.center()
    gNativeWindows.append(w)
    if gPrimaryView == nil { gPrimaryView = v }
    return Unmanaged.passUnretained(w).toOpaque()
}

@_cdecl("PearBackendSetRootLayer")
public func PearBackendSetRootLayer(_ handle: UnsafeMutableRawPointer, _ layer: CALayer) {
    let w = Unmanaged<NSWindow>.fromOpaque(handle).takeUnretainedValue()
    guard let v = w.contentView else { return }
    // Layer-hosting view: the UIWindow's layer tree is the view's content.
    v.layer = layer
    v.wantsLayer = true
    layer.contentsScale = w.backingScaleFactor
}

@_cdecl("PearBackendShowWindow")
public func PearBackendShowWindow(_ handle: UnsafeMutableRawPointer, _ key: Bool) {
    let w = Unmanaged<NSWindow>.fromOpaque(handle).takeUnretainedValue()
    if key {
        w.makeKeyAndOrderFront(nil)
    } else {
        w.orderFront(nil)
    }
    NSApp.activate(ignoringOtherApps: true)
}

@_cdecl("PearBackendDisplayLink")
public func PearBackendDisplayLink(_ target: Any, _ selector: Selector) -> CADisplayLink? {
    if let primary = gPrimaryView {
        return primary.displayLink(target: target, selector: selector)
    }
    return NSScreen.main?.displayLink(target: target, selector: selector)
}

// iOS code creates display links with +[CADisplayLink displayLinkWithTarget:selector:],
// which macOS QuartzCore does not offer; macOS hands them out from NSView/NSScreen.
private let pearDisplayLinkClassFactory: @convention(c) (AnyObject, Selector, AnyObject, Selector) -> AnyObject? = { _, _, target, selector in
    PearBackendDisplayLink(target, selector)
}

// iOS CADisplayLink API that macOS's display link lacks. Documented semantics:
// frameInterval N = fire every Nth refresh; preferredFramesPerSecond = target
// rate (0 = native). Both map onto preferredFrameRateRange.
private var kFrameIntervalKey: UInt8 = 0
private var kPreferredFPSKey: UInt8 = 0

private func applyRate(_ link: CADisplayLink, fps rawFPS: Double) {
    let maxFPS = Double(PearBackendScreenInfo().maxFPS)
    let fps = (rawFPS <= 0 || rawFPS > maxFPS) ? maxFPS : rawFPS
    let f = Float(fps)
    link.preferredFrameRateRange = CAFrameRateRange(minimum: f, maximum: f, preferred: f)
}

private let pearFrameInterval: @convention(c) (AnyObject, Selector) -> Int = { obj, _ in
    (objc_getAssociatedObject(obj, &kFrameIntervalKey) as? NSNumber)?.intValue ?? 1
}

private let pearSetFrameInterval: @convention(c) (AnyObject, Selector, Int) -> Void = { obj, _, rawInterval in
    let interval = max(1, rawInterval)
    objc_setAssociatedObject(obj, &kFrameIntervalKey, NSNumber(value: interval), .OBJC_ASSOCIATION_RETAIN)
    if let link = obj as? CADisplayLink {
        applyRate(link, fps: Double(PearBackendScreenInfo().maxFPS) / Double(interval))
    }
}

private let pearPreferredFPS: @convention(c) (AnyObject, Selector) -> Int = { obj, _ in
    (objc_getAssociatedObject(obj, &kPreferredFPSKey) as? NSNumber)?.intValue ?? 0
}

private let pearSetPreferredFPS: @convention(c) (AnyObject, Selector, Int) -> Void = { obj, _, fps in
    objc_setAssociatedObject(obj, &kPreferredFPSKey, NSNumber(value: fps), .OBJC_ASSOCIATION_RETAIN)
    if let link = obj as? CADisplayLink {
        applyRate(link, fps: Double(fps))
    }
}

@_cdecl("PearBackendInstallQuartzCoreCompat")
public func PearBackendInstallQuartzCoreCompat() {
    guard let cls = objc_getClass("CADisplayLink") as? AnyClass else {
        PearMissingReport(PearMissingBehavior, "QuartzCore", "CADisplayLink class absent (macOS < 14)", nil)
        return
    }
    let factorySel = NSSelectorFromString("displayLinkWithTarget:selector:")
    if !cls.responds(to: factorySel), let meta = object_getClass(cls) {
        class_addMethod(meta, factorySel, unsafeBitCast(pearDisplayLinkClassFactory, to: IMP.self), "@@:@:")
        PearHostLogMessage("host", "+[CADisplayLink displayLinkWithTarget:selector:] provided by the host")
    }
    // macOS's concrete display link class inherits these selectors but raises
    // when they are sent (observed: -[_NSDisplayLink setFrameInterval:]).
    // Probe a real instance; replace each selector that raises or is missing.
    guard let probe = NSScreen.main?.displayLink(target: NSNull(), selector: #selector(NSObject.classForCoder)) else {
        return
    }
    let concrete: AnyClass = object_getClass(probe) ?? cls
    let table: [(Selector, IMP, String, Bool)] = [
        (NSSelectorFromString("frameInterval"), unsafeBitCast(pearFrameInterval, to: IMP.self), "q@:", false),
        (NSSelectorFromString("setFrameInterval:"), unsafeBitCast(pearSetFrameInterval, to: IMP.self), "v@:q", true),
        (NSSelectorFromString("preferredFramesPerSecond"), unsafeBitCast(pearPreferredFPS, to: IMP.self), "q@:", false),
        (NSSelectorFromString("setPreferredFramesPerSecond:"), unsafeBitCast(pearSetPreferredFPS, to: IMP.self), "v@:q", true),
    ]
    for (sel, imp, types, isSetter) in table {
        let works = probe.responds(to: sel) && PearUIKitTryProbeDisplayLinkSelector(probe, sel, isSetter)
        if !works {
            types.withCString { cTypes in
                _ = class_replaceMethod(concrete, sel, imp, cTypes)
            }
            let clsName = String(cString: class_getName(concrete))
            let selName = String(cString: sel_getName(sel))
            PearHostLogMessage("host", "-[\(clsName) \(selName)] provided by the host")
        }
    }
    probe.invalidate()
}

@_cdecl("PearBackendOpenURL")
public func PearBackendOpenURL(_ url: URL) -> Bool {
    NSWorkspace.shared.open(url)
}

@_cdecl("PearBackendSetIdleTimerDisabled")
public func PearBackendSetIdleTimerDisabled(_ disabled: Bool) {
    if disabled && gIdleAssertion == 0 {
        IOPMAssertionCreateWithName(
            kIOPMAssertionTypePreventUserIdleDisplaySleep as CFString,
            IOPMAssertionLevel(kIOPMAssertionLevelOn),
            "iOS app disabled the idle timer" as CFString,
            &gIdleAssertion
        )
    } else if !disabled && gIdleAssertion != 0 {
        IOPMAssertionRelease(gIdleAssertion)
        gIdleAssertion = 0
    }
}

private final class PearHostAppDelegate: NSObject, NSApplicationDelegate {
    var didFinish: (() -> Void)?

    func applicationDidFinishLaunching(_ notification: Notification) {
        didFinish?()
        didFinish = nil
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@_cdecl("PearBackendRun")
public func PearBackendRun(_ didFinishLaunching: @escaping () -> Void) {
    let delegate = PearHostAppDelegate()
    delegate.didFinish = didFinishLaunching
    gAppDelegate = delegate
    NSApp.delegate = delegate

    let bar = NSMenu()
    let appItem = NSMenuItem()
    bar.addItem(appItem)
    let appMenu = NSMenu()
    appMenu.addItem(withTitle: "Quit", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
    appItem.submenu = appMenu
    NSApp.mainMenu = bar

    NSApp.run()
}

// ---- input -------------------------------------------------------------------

private final class PearHostView: NSView {
    weak var uiWindow: UIWindow?

    override var isFlipped: Bool { true } // UIKit coordinates: origin top-left
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override var wantsUpdateLayer: Bool { true }

    private func pearPointer(_ e: NSEvent, phase: UITouchPhase) {
        let p = convert(e.locationInWindow, from: nil)
        guard let w = uiWindow else { return }
        // Window content may be scaled relative to the UIWindow's logical bounds.
        let b = w.bounds
        let ui = CGPoint(
            x: p.x * b.size.width / max(bounds.size.width, 1),
            y: p.y * b.size.height / max(bounds.size.height, 1)
        )
        PearUIKitDeliverPointer(w, phase, ui, e.timestamp)
    }

    override func mouseDown(with event: NSEvent) { pearPointer(event, phase: .began) }
    override func mouseDragged(with event: NSEvent) { pearPointer(event, phase: .moved) }
    override func mouseUp(with event: NSEvent) { pearPointer(event, phase: .ended) }

    // ---- keyboard ----
    // Text: while a UIKit text responder is first responder, AppKit's key binding
    // system turns key downs into text and edit commands for it.
    // Keys: otherwise every key down/up becomes a UIPress with its HID usage
    // (games such as Minecraft read keyboards through pressesBegan:withEvent:).
    override func keyDown(with event: NSEvent) {
        if PearUIKitTextInputActive() {
            interpretKeyEvents([event])
            return
        }
        if event.isARepeat { return }
        pearKey(event, down: true)
    }

    override func keyUp(with event: NSEvent) {
        if PearUIKitTextInputActive() { return }
        pearKey(event, down: false)
    }

    override func flagsChanged(with event: NSEvent) {
        let usage = PearHIDUsageForMacKeyCode(event.keyCode)
        let bit: NSEvent.ModifierFlags
        switch usage {
        case 0xE1, 0xE5: bit = .shift
        case 0xE0, 0xE4: bit = .control
        case 0xE2, 0xE6: bit = .option
        case 0xE3, 0xE7: bit = .command
        case 0x39: bit = .capsLock
        default: return
        }
        let down = event.modifierFlags.contains(bit)
        let mods = Int(event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue)
        PearUIKitDeliverKey(usage, "", "", mods, down, event.timestamp)
    }

    private func pearKey(_ event: NSEvent, down: Bool) {
        let usage = PearHIDUsageForMacKeyCode(event.keyCode)
        if usage == 0 { return }
        // UIKeyModifierFlags use the same bit positions as NSEventModifierFlags.
        let mods = Int(event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue)
        PearUIKitDeliverKey(
            usage,
            (event.characters ?? "") as NSString,
            (event.charactersIgnoringModifiers ?? "") as NSString,
            mods,
            down,
            event.timestamp
        )
    }

    // NSResponder text entry points reached through -interpretKeyEvents:.
    @objc override func insertText(_ string: Any) {
        let s: String? = if let attr = string as? NSAttributedString { attr.string } else { string as? String }
        if let s, !s.isEmpty {
            PearUIKitDeliverText(s as NSString)
        }
    }

    override func doCommand(by selector: Selector) {
        if getenv("PEAR_HOST_TRACE_INPUT").map({ String(cString: $0) }) == "1" {
            let name = String(cString: sel_getName(selector))
            PearHostLogMessage("input", "AppKit command \(name)")
        }
        if selector == #selector(NSResponder.deleteBackward(_:)) {
            PearUIKitDeliverEditCommand(.deleteBackward)
        } else if selector == #selector(NSResponder.insertNewline(_:)) || selector == #selector(NSResponder.insertLineBreak(_:)) {
            PearUIKitDeliverEditCommand(.newline)
        } else if selector == #selector(NSResponder.moveLeft(_:)) {
            PearUIKitDeliverEditCommand(.moveLeft)
        } else if selector == #selector(NSResponder.moveRight(_:)) {
            PearUIKitDeliverEditCommand(.moveRight)
        } else if selector == #selector(NSResponder.cancelOperation(_:)) {
            PearUIKitDeliverEditCommand(.cancel)
        } else if selector == #selector(NSResponder.insertTab(_:)) {
            PearUIKitDeliverText("\t")
        }
    }
}

// macOS virtual key code (Carbon kVK_*, public HIToolbox/Events.h) -> USB HID
// keyboard usage (public HID Usage Tables, page 0x07), which is what
// UIKey.keyCode reports.
private let macKeyCodeMap: [UInt16: Int] = [
    0x00: 0x04, 0x0B: 0x05, 0x08: 0x06, 0x02: 0x07, 0x0E: 0x08, 0x03: 0x09, 0x05: 0x0A,
    0x04: 0x0B, 0x22: 0x0C, 0x26: 0x0D, 0x28: 0x0E, 0x25: 0x0F, 0x2E: 0x10, 0x2D: 0x11,
    0x1F: 0x12, 0x23: 0x13, 0x0C: 0x14, 0x0F: 0x15, 0x01: 0x16, 0x11: 0x17, 0x20: 0x18,
    0x09: 0x19, 0x0D: 0x1A, 0x07: 0x1B, 0x10: 0x1C, 0x06: 0x1D, // A-Z
    0x12: 0x1E, 0x13: 0x1F, 0x14: 0x20, 0x15: 0x21, 0x17: 0x22, 0x16: 0x23, 0x1A: 0x24,
    0x1C: 0x25, 0x19: 0x26, 0x1D: 0x27, // 1-0
    0x24: 0x28, 0x35: 0x29, 0x33: 0x2A, 0x30: 0x2B, 0x31: 0x2C, 0x1B: 0x2D, 0x18: 0x2E,
    0x21: 0x2F, 0x1E: 0x30, 0x2A: 0x31, 0x29: 0x33, 0x27: 0x34, 0x32: 0x35, 0x2B: 0x36,
    0x2F: 0x37, 0x2C: 0x38, 0x39: 0x39, 0x0A: 0x64,
    0x7A: 0x3A, 0x78: 0x3B, 0x63: 0x3C, 0x76: 0x3D, 0x60: 0x3E, 0x61: 0x3F, 0x62: 0x40,
    0x64: 0x41, 0x65: 0x42, 0x6D: 0x43, 0x67: 0x44, 0x6F: 0x45, // F1-F12
    0x73: 0x4A, 0x74: 0x4B, 0x75: 0x4C, 0x77: 0x4D, 0x79: 0x4E, // home..pgdn
    0x7C: 0x4F, 0x7B: 0x50, 0x7D: 0x51, 0x7E: 0x52, // arrows
    0x47: 0x53, 0x4B: 0x54, 0x43: 0x55, 0x4E: 0x56, 0x45: 0x57, 0x4C: 0x58, 0x53: 0x59,
    0x54: 0x5A, 0x55: 0x5B, 0x56: 0x5C, 0x57: 0x5D, 0x58: 0x5E, 0x59: 0x5F, 0x5B: 0x60,
    0x5C: 0x61, 0x52: 0x62, 0x41: 0x63, 0x51: 0x67, // keypad
    0x3B: 0xE0, 0x38: 0xE1, 0x3A: 0xE2, 0x37: 0xE3, 0x3E: 0xE4, 0x3C: 0xE5, 0x3D: 0xE6,
    0x36: 0xE7, // modifiers
]

public func PearHIDUsageForMacKeyCode(_ code: UInt16) -> Int {
    macKeyCodeMap[code] ?? 0
}

private final class PearHostWindowDelegate: NSObject, NSWindowDelegate {
    weak var uiWindow: UIWindow?

    func windowDidResize(_ notification: Notification) {
        guard let w = notification.object as? NSWindow,
              let ui = uiWindow,
              let contentView = w.contentView else { return }
        PearUIKitWindowDidResize(ui, contentView.bounds.size, w.backingScaleFactor)
    }

    func windowDidChangeBackingProperties(_ notification: Notification) {
        windowDidResize(notification)
    }

    func windowDidBecomeKey(_ notification: Notification) {
        PearUIKitSetKeyWindow(uiWindow)
    }
}

@_cdecl("PearBackendPresentAlert")
public func PearBackendPresentAlert(
    _ title: String,
    _ message: String,
    _ buttons: [String],
    _ reply: @escaping (Int) -> Void
) {
    DispatchQueue.main.async {
        let alert = NSAlert()
        alert.messageText = title.isEmpty ? " " : title
        alert.informativeText = message
        for b in buttons { alert.addButton(withTitle: b) }
        if buttons.isEmpty { alert.addButton(withTitle: "OK") }
        let done: (NSApplication.ModalResponse) -> Void = { r in
            let index = r.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
            reply(buttons.isEmpty ? -1 : index)
        }
        if let host = NSApp.keyWindow ?? gNativeWindows.first {
            alert.beginSheetModal(for: host, completionHandler: done)
        } else {
            done(alert.runModal())
        }
    }
}

// ---- scripted input (tests) ------------------------------------------------------

private func macKeyCodeForName(_ name: String) -> (UInt16, String) {
    let named: [String: (UInt16, String)] = [
        "return": (0x24, "\r"),
        "escape": (0x35, "\u{1B}"),
        "delete": (0x33, "\u{7F}"),
        "tab": (0x30, "\t"),
        "space": (0x31, " "),
        // Arrow keys carry AppKit's function-key characters.
        "up": (0x7E, "\u{F700}"),
        "down": (0x7D, "\u{F701}"),
        "left": (0x7B, "\u{F702}"),
        "right": (0x7C, "\u{F703}"),
    ]
    if let hit = named[name.lowercased()] { return hit }
    let letters = "asdfhgzxcv_bqweryt123465=97-80]ou[ip_lj'k;\\,/nm."
    let lower = name.lowercased()
    if lower.count == 1, let idx = letters.firstIndex(of: Character(lower)) {
        return (UInt16(letters.distance(from: letters.startIndex, to: idx)), name)
    }
    return (0xFFFF, name)
}

private func scriptKey(_ name: String, down: Bool) {
    let (code, chars) = macKeyCodeForName(name)
    let w = gPrimaryView?.window
    let e = NSEvent.keyEvent(
        with: down ? .keyDown : .keyUp,
        location: .zero,
        modifierFlags: [],
        timestamp: ProcessInfo.processInfo.systemUptime,
        windowNumber: w?.windowNumber ?? 0,
        context: nil,
        characters: chars,
        charactersIgnoringModifiers: chars,
        isARepeat: false,
        keyCode: code == 0xFFFF ? 0 : code
    )
    guard let e else { return }
    if down {
        gPrimaryView?.keyDown(with: e)
    } else {
        gPrimaryView?.keyUp(with: e)
    }
}

private func scriptCall(_ args: [String]) {
    let t = args[0]
    let w = PearUIKitKeyWindow()
    let target: AnyObject? = switch t {
    case "delegate": UIApplication.sharedApplication()?.delegate
    case "rootvc": w?.rootViewController
    case "rootview": w?.rootViewController?.view
    case "responder": PearUIKitFirstResponder()
    default: NSClassFromString(t)
    }
    let sel = NSSelectorFromString(args[1])
    guard let target, target.responds(to: sel) else {
        let clsName = target.map { String(cString: class_getName(object_getClass($0))) } ?? "nil"
        PearHostLogMessage("script", "call: \(t) (\(clsName)) does not respond to \(args[1])")
        return
    }
    PearBackendInvokeScriptCall(target, sel, args)
    let clsName = String(cString: class_getName(object_getClass(target)))
    PearHostLogMessage("script", "call: -[\(clsName) \(args[1])] sent")
}

@_cdecl("PearBackendStartInputScript")
public func PearBackendStartInputScript() {
    guard let pathPtr = getenv("PEAR_HOST_INPUT_SCRIPT"), pathPtr.pointee != 0 else { return }
    let path = String(cString: pathPtr)
    guard let text = try? String(contentsOfFile: path, encoding: .utf8) else {
        PearHostLogMessage("script", "cannot read \(path)")
        return
    }
    let lines = text.components(separatedBy: .newlines)
    Thread.detachNewThread {
        for raw in lines {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            let parts = line.split(separator: " ", maxSplits: 1, omittingEmptySubsequences: false)
            let cmd = String(parts[0])
            let arg = parts.count > 1 ? String(parts[1]) : ""
            let args = arg.components(separatedBy: " ")
            PearHostLogMessage("script", line)
            if cmd == "wait" {
                Thread.sleep(forTimeInterval: Double(arg) ?? 0)
            } else if cmd == "tap", args.count >= 2 {
                let x = Double(args[0]) ?? 0
                let y = Double(args[1]) ?? 0
                DispatchQueue.main.sync {
                    guard let w = gPrimaryView?.uiWindow else { return }
                    let p = CGPoint(x: x * w.bounds.size.width, y: y * w.bounds.size.height)
                    PearUIKitDeliverPointer(w, .began, p, ProcessInfo.processInfo.systemUptime)
                }
                Thread.sleep(forTimeInterval: 0.08)
                DispatchQueue.main.sync {
                    guard let w = gPrimaryView?.uiWindow else { return }
                    let p = CGPoint(x: x * w.bounds.size.width, y: y * w.bounds.size.height)
                    PearUIKitDeliverPointer(w, .ended, p, ProcessInfo.processInfo.systemUptime)
                }
            } else if cmd == "key" || cmd == "hold" {
                let k = args[0]
                let secs = (cmd == "hold" && args.count > 1) ? (Double(args[1]) ?? 0.05) : 0.05
                DispatchQueue.main.sync { scriptKey(k, down: true) }
                Thread.sleep(forTimeInterval: secs)
                DispatchQueue.main.sync { scriptKey(k, down: false) }
            } else if cmd == "type" {
                for ch in arg {
                    let s = ch == " " ? "space" : String(ch)
                    DispatchQueue.main.sync { scriptKey(s, down: true) }
                    DispatchQueue.main.sync { scriptKey(s, down: false) }
                    Thread.sleep(forTimeInterval: 0.03)
                }
            } else if cmd == "call", args.count >= 2 {
                DispatchQueue.main.sync { scriptCall(args) }
            } else if cmd == "log" {
                // already logged
            } else {
                PearHostLogMessage("script", "unknown command: \(cmd)")
            }
        }
        PearHostLogMessage("script", "done")
    }
}
