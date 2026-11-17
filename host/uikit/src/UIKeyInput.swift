// Hardware keyboard and text input: UIKey / UIPress / UIPressesEvent delivery
// to the first responder, UITextView, and the keyboard notifications.
// Clean-room: from the public UIKey/UIPress/UITextView/UIKeyInput documentation;
// key codes are USB HID usages as documented for UIKeyboardHIDUsage.
import Foundation
import ObjectiveC

private func sendTextDelegate(_ target: AnyObject, _ selName: String, _ tv: UITextView) {
    let sel = NSSelectorFromString(selName)
    guard target.responds(to: sel),
          let method = class_getInstanceMethod(object_getClass(target), sel) else { return }
    let fn = unsafeBitCast(method_getImplementation(method), to: (@convention(c) (AnyObject, Selector, UITextView) -> Void).self)
    fn(target, sel, tv)
}

private func sendTextDelegateBool(_ target: AnyObject, _ selName: String, _ tv: UITextView) -> Bool? {
    let sel = NSSelectorFromString(selName)
    guard target.responds(to: sel),
          let method = class_getInstanceMethod(object_getClass(target), sel) else { return nil }
    let fn = unsafeBitCast(method_getImplementation(method), to: (@convention(c) (AnyObject, Selector, UITextView) -> ObjCBool).self)
    return fn(target, sel, tv).boolValue
}

private func sendShouldChangeText(_ target: AnyObject, _ tv: UITextView, _ range: NSRange, _ replacement: String) -> Bool? {
    let sel = NSSelectorFromString("textView:shouldChangeTextInRange:replacementText:")
    guard target.responds(to: sel),
          let method = class_getInstanceMethod(object_getClass(target), sel) else { return nil }
    let fn = unsafeBitCast(
        method_getImplementation(method),
        to: (@convention(c) (AnyObject, Selector, UITextView, NSRange, NSString) -> ObjCBool).self
    )
    return fn(target, sel, tv, range, replacement as NSString).boolValue
}

// ---- UIKey / UIPress ------------------------------------------------------------

@objc(UIKey)
open class UIKey: NSObject, NSCopying {
    private let _characters: String
    private let _charactersIgnoringModifiers: String
    private let _modifierFlags: Int
    private let _keyCode: Int

    @objc(initWithCharacters:ignoringModifiers:modifiers:keyCode:)
    public init(characters: String?, ignoringModifiers: String?, modifiers: Int, keyCode: Int) {
        _characters = characters ?? ""
        _charactersIgnoringModifiers = ignoringModifiers ?? ""
        _modifierFlags = modifiers
        _keyCode = keyCode
        super.init()
    }

    @objc public var characters: String { _characters }
    @objc public var charactersIgnoringModifiers: String { _charactersIgnoringModifiers }
    @objc public var modifierFlags: Int { _modifierFlags }
    @objc public var keyCode: Int { _keyCode }

    @objc(copyWithZone:)
    public func copy(with zone: NSZone? = nil) -> Any { self }
}

@objc(UIPress)
open class UIPress: NSObject {
    private let _key: UIKey?
    private let _phase: Int
    private let _type: Int
    private let _timestamp: TimeInterval

    @objc(initWithKey:type:phase:timestamp:)
    public init(key: UIKey?, type: Int, phase: Int, timestamp: TimeInterval) {
        _key = key
        _type = type
        _phase = phase
        _timestamp = timestamp
        super.init()
    }

    @objc public var key: UIKey? { _key }
    @objc public var phase: Int { _phase }
    @objc public var type: Int { _type }
    @objc public var timestamp: TimeInterval { _timestamp }
    @objc public var force: CGFloat { _phase == 3 ? 0 : 1 } // ended -> 0
    @objc public var responder: UIResponder? { nil }
    @objc public var window: UIWindow? { PearUIKitKeyWindow() }
    @objc public var gestureRecognizers: [Any] { [] }
}

@objc(UIPressesEvent)
open class UIPressesEvent: UIEvent {
    private let _presses: Set<UIPress>
    private let _ts: TimeInterval

    @objc(initWithPresses:timestamp:)
    public init(presses: Set<UIPress>, timestamp: TimeInterval) {
        _presses = presses
        _ts = timestamp
        super.init(touches: [], timestamp: timestamp)
    }

    @objc open override var type: UIEventType { .presses }
    @objc open override var timestamp: TimeInterval { _ts }
    @objc public var allPresses: Set<UIPress> { _presses }
    @objc(pressesForGestureRecognizer:) public func presses(for gestureRecognizer: Any?) -> Set<UIPress> { [] }
    @objc open override func allTouches() -> Set<UITouch>? { nil }
}

// ---- keyboard visibility ------------------------------------------------------------

private var gSoftKeyboardShown = false

// There is no on-screen keyboard on a Mac: report a zero-height keyboard at the
// bottom edge, so layout code that subtracts its height leaves the UI in place.
private func postKeyboard(show: Bool) {
    if gSoftKeyboardShown == show { return }
    gSoftKeyboardShown = show
    let screen = UIScreen.mainScreen().bounds
    let frame = CGRect(x: 0, y: screen.size.height, width: screen.size.width, height: 0)
    let info: [AnyHashable: Any] = [
        "UIKeyboardFrameBeginUserInfoKey": NSValue(rect: frame),
        "UIKeyboardFrameEndUserInfoKey": NSValue(rect: frame),
        "UIKeyboardAnimationDurationUserInfoKey": 0.0,
        "UIKeyboardAnimationCurveUserInfoKey": 7,
        "UIKeyboardIsLocalUserInfoKey": true,
    ]
    let nc = NotificationCenter.default
    nc.post(name: NSNotification.Name(show ? "UIKeyboardWillShowNotification" : "UIKeyboardWillHideNotification"), object: nil, userInfo: info)
    nc.post(name: NSNotification.Name("UIKeyboardWillChangeFrameNotification"), object: nil, userInfo: info)
    nc.post(name: NSNotification.Name(show ? "UIKeyboardDidShowNotification" : "UIKeyboardDidHideNotification"), object: nil, userInfo: info)
    nc.post(name: NSNotification.Name("UIKeyboardDidChangeFrameNotification"), object: nil, userInfo: info)
    PearHostLogMessage("keyboard", "text input \(show ? "started" : "ended")")
}

// ---- UITextView ---------------------------------------------------------------------

@objc(UITextView)
open class UITextView: UIScrollView {
    private let _text = NSMutableString()
    private var _selected = NSRange(location: 0, length: 0)
    private weak var _textDelegate: AnyObject?
    private var _editable = true
    private var _editing = false
    private var _keyboardType = 0
    private var _returnKeyType = 0
    private var _autocap = 0
    private var _autocorrect = 0
    private var _spell = 0
    private var _smartQuotes = 0
    private var _smartDashes = 0
    private var _secure = false
    private var _textColor: UIColor?
    private var _font: AnyObject?

    @objc(initWithFrame:)
    public override init(frame: CGRect) {
        super.init(frame: frame)
    }

    @objc(initWithFrame:textContainer:)
    public convenience init(frame: CGRect, textContainer: Any?) {
        self.init(frame: frame)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        super.init(coder: coder)
        if let decoded = coder.decodeObject(forKey: "UIText") as? String {
            _text.setString(decoded)
        }
    }

    @objc open override var delegate: AnyObject? {
        get { _textDelegate }
        set { _textDelegate = newValue }
    }

    @objc open var text: String {
        get { _text.copy() as! String }
        set {
            _text.setString(newValue)
            _selected = NSRange(location: _text.length, length: 0)
        }
    }

    @objc open var attributedText: NSAttributedString {
        get { NSAttributedString(string: _text as String) }
        set { text = newValue.string }
    }

    @objc open var selectedRange: NSRange {
        get { _selected }
        set {
            let len = _text.length
            var r = newValue
            if r.location > len { r.location = len }
            if NSMaxRange(r) > len { r.length = len - r.location }
            _selected = r
        }
    }

    @objc(isEditable) open func isEditable() -> Bool { _editable }
    @objc(setEditable:) open func setEditable(_ editable: Bool) { _editable = editable }
    @objc(isSelectable) open func isSelectable() -> Bool { true }
    @objc(setSelectable:) open func setSelectable(_ selectable: Bool) {}
    @objc open var hasText: Bool { _text.length > 0 }

    @objc open var font: AnyObject? {
        get { _font }
        set { _font = newValue }
    }

    @objc open var textColor: UIColor? {
        get { _textColor }
        set { _textColor = newValue }
    }

    @objc open var textAlignment: Int {
        get { 0 }
        set {}
    }

    @objc(isScrollEnabled) open override func isScrollEnabled() -> Bool { false }
    @objc(setScrollEnabled:) open override func setScrollEnabled(_ enabled: Bool) {}
    @objc open var textContainer: AnyObject? { nil }

    @objc open var textContainerInset: UIEdgeInsets {
        get { UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 0) }
        set {}
    }

    @objc(scrollRangeToVisible:) open func scrollRangeToVisible(_ range: NSRange) {}

    @objc open var inputDelegate: AnyObject? {
        get { nil }
        set {}
    }

    // UITextInputTraits: stored, no effect on a hardware keyboard.
    @objc open var keyboardType: Int { get { _keyboardType } set { _keyboardType = newValue } }
    @objc open var returnKeyType: Int { get { _returnKeyType } set { _returnKeyType = newValue } }
    @objc open var autocapitalizationType: Int { get { _autocap } set { _autocap = newValue } }
    @objc open var autocorrectionType: Int { get { _autocorrect } set { _autocorrect = newValue } }
    @objc open var spellCheckingType: Int { get { _spell } set { _spell = newValue } }
    @objc open var smartQuotesType: Int { get { _smartQuotes } set { _smartQuotes = newValue } }
    @objc open var smartDashesType: Int { get { _smartDashes } set { _smartDashes = newValue } }
    @objc open var smartInsertDeleteType: Int { get { 0 } set {} }
    @objc open var keyboardAppearance: Int { get { 0 } set {} }
    @objc open var enablesReturnKeyAutomatically: Bool { get { false } set {} }
    @objc(isSecureTextEntry) open func isSecureTextEntry() -> Bool { _secure }
    @objc(setSecureTextEntry:) open func setSecureTextEntry(_ secure: Bool) { _secure = secure }
    @objc open var textContentType: AnyObject? { get { nil } set {} }

    open override var canBecomeFirstResponder: Bool { _editable }

    open override func becomeFirstResponder() -> Bool {
        if _editing { return true }
        if let d = _textDelegate, sendTextDelegateBool(d, "textViewShouldBeginEditing:", self) == false {
            return false
        }
        if !super.becomeFirstResponder() { return false }
        _editing = true
        postKeyboard(show: true)
        if let d = _textDelegate {
            sendTextDelegate(d, "textViewDidBeginEditing:", self)
        }
        NotificationCenter.default.post(name: NSNotification.Name("UITextViewTextDidBeginEditingNotification"), object: self)
        return true
    }

    open override func resignFirstResponder() -> Bool {
        if !_editing { return super.resignFirstResponder() }
        if let d = _textDelegate, sendTextDelegateBool(d, "textViewShouldEndEditing:", self) == false {
            return false
        }
        _ = super.resignFirstResponder()
        _editing = false
        if let d = _textDelegate {
            sendTextDelegate(d, "textViewDidEndEditing:", self)
        }
        NotificationCenter.default.post(name: NSNotification.Name("UITextViewTextDidEndEditingNotification"), object: self)
        postKeyboard(show: false)
        return true
    }

    open override func removeFromSuperview() {
        if _editing { _ = resignFirstResponder() }
        super.removeFromSuperview()
    }

    // The documented editing contract: ask the delegate, apply, then notify.
    private func pearReplaceRange(_ range: NSRange, with replacement: String) {
        guard _editable else { return }
        selectedRange = _selected
        if NSMaxRange(range) > _text.length { return }
        let traceInput = (getenv("PEAR_HOST_TRACE_INPUT").map { String(cString: $0) } == "1")
        if let d = _textDelegate, sendShouldChangeText(d, self, range, replacement) == false {
            if traceInput {
                let escaped = replacement.replacingOccurrences(of: "\n", with: "\\n")
                PearHostLogMessage("input", "delegate declined replacing \(range.length) chars with \"\(escaped)\"")
            }
            return
        }
        _text.replaceCharacters(in: range, with: replacement)
        if traceInput {
            PearHostLogMessage("input", "text view now \"\(_text)\"")
        }
        _selected = NSRange(location: range.location + (replacement as NSString).length, length: 0)
        if let d = _textDelegate {
            sendTextDelegate(d, "textViewDidChange:", self)
            sendTextDelegate(d, "textViewDidChangeSelection:", self)
        }
        NotificationCenter.default.post(name: NSNotification.Name("UITextViewTextDidChangeNotification"), object: self)
    }

    // UIKeyInput
    @objc(insertText:)
    open func insertText(_ text: String) {
        pearReplaceRange(_selected, with: text)
    }

    @objc open func deleteBackward() {
        var r = _selected
        if r.length == 0 {
            if r.location == 0 { return }
            r = _text.rangeOfComposedCharacterSequence(at: r.location - 1)
        }
        pearReplaceRange(r, with: "")
    }

    @objc(pear_moveCursor:)
    public func pear_moveCursor(_ delta: Int) {
        let loc = max(0, _selected.location + delta)
        selectedRange = NSRange(location: loc, length: 0)
        if let d = _textDelegate {
            sendTextDelegate(d, "textViewDidChangeSelection:", self)
        }
    }
}

// ---- routing from the backend ------------------------------------------------------------

private func pressTarget() -> UIResponder? {
    if let r = PearUIKitFirstResponder() { return r }
    let w = PearUIKitKeyWindow()
    let vc = w?.rootViewController
    return vc?.viewIfLoaded ?? vc ?? w
}

private func isTextResponder(_ responder: UIResponder?) -> Bool {
    guard let responder else { return false }
    return responder.responds(to: NSSelectorFromString("insertText:"))
        && responder.responds(to: NSSelectorFromString("deleteBackward"))
        && responder.responds(to: NSSelectorFromString("hasText"))
}

@_cdecl("PearUIKitTextInputActive")
public func PearUIKitTextInputActive() -> Bool {
    isTextResponder(PearUIKitFirstResponder())
}

@_cdecl("PearUIKitDeliverText")
public func PearUIKitDeliverText(_ text: NSString) {
    guard let r = PearUIKitFirstResponder(), isTextResponder(r) else { return }
    let sel = NSSelectorFromString("insertText:")
    if let method = class_getInstanceMethod(object_getClass(r), sel) {
        let fn = unsafeBitCast(method_getImplementation(method), to: (@convention(c) (AnyObject, Selector, NSString) -> Void).self)
        fn(r, sel, text)
    }
}

@_cdecl("PearUIKitDeliverEditCommand")
public func PearUIKitDeliverEditCommand(_ command: PearEditCommand) {
    guard let r = PearUIKitFirstResponder(), isTextResponder(r) else { return }
    switch command {
    case .deleteBackward:
        r.perform(NSSelectorFromString("deleteBackward"))
    case .newline:
        PearUIKitDeliverText("\n")
    case .moveLeft:
        if let tv = r as? UITextView { tv.pear_moveCursor(-1) }
    case .moveRight:
        if let tv = r as? UITextView { tv.pear_moveCursor(1) }
    case .cancel:
        _ = r.resignFirstResponder()
    @unknown default:
        break
    }
}

// Press type for keys that have a documented UIPressType meaning; other keys
// carry no type of their own.
private func pressTypeForHID(_ usage: Int) -> Int {
    switch usage {
    case 0x52: return 0 // up arrow
    case 0x51: return 1 // down
    case 0x50: return 2 // left
    case 0x4F: return 3 // right
    case 0x28: return 4 // return -> select
    case 0x29: return 5 // escape -> menu
    default: return 1000
    }
}

private var gTraceInput: Bool = {
    getenv("PEAR_HOST_TRACE_INPUT").map { String(cString: $0) } == "1"
}()

@_cdecl("PearUIKitDeliverKey")
public func PearUIKitDeliverKey(
    _ hidUsage: Int,
    _ chars: NSString,
    _ charsNoMods: NSString,
    _ modifiers: Int,
    _ down: Bool,
    _ timestamp: TimeInterval
) {
    let key = UIKey(characters: chars as String, ignoringModifiers: charsNoMods as String, modifiers: modifiers, keyCode: hidUsage)
    let press = UIPress(key: key, type: pressTypeForHID(hidUsage), phase: down ? 0 : 3, timestamp: timestamp)
    let set: Set<UIPress> = [press]
    let event = UIPressesEvent(presses: set, timestamp: timestamp)
    let target = pressTarget()
    if gTraceInput {
        var chain: [String] = []
        var cur = target
        while let r = cur, chain.count < 20 {
            chain.append(NSStringFromClass(type(of: r)))
            cur = r.nextResponder
        }
        let frName = PearUIKitFirstResponder().map { NSStringFromClass(type(of: $0)) } ?? "none"
        let hexUsage = String(format: "0x%02lx", hidUsage)
        PearHostLogMessage("input", "key \(hexUsage) \(down ? "down" : "up") -> \(chain.joined(separator: " > ")) (first responder: \(frName))")
    }
    if down {
        target?.pressesBegan(set, with: event)
    } else {
        target?.pressesEnded(set, with: event)
    }
}
