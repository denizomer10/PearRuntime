// Clean-room: from the public UILabel and UIProgressView documentation.
import CoreGraphics
import CoreText
import Foundation

@objc(UILabel)
open class UILabel: UIView {
    private var _text: String?
    private var _attributedText: NSAttributedString?
    private var _font: UIFont = UIFont.systemFont(ofSize: 17)
    private var _textColor: UIColor = UIColor.whiteColor()
    private var _textAlignment: Int = 0
    private var _numberOfLines: Int = 1
    private var _lineBreakMode: Int = 4 // byTruncatingTail

    @objc(initWithFrame:)
    public override init(frame: CGRect) {
        super.init(frame: frame)
        setOpaque(false)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        super.init(coder: coder)
        if let decoded = coder.decodeObject(forKey: "UIText") as? String {
            _text = decoded
        }
        if coder.containsValue(forKey: "UINumberOfLines") {
            _numberOfLines = coder.decodeInteger(forKey: "UINumberOfLines")
        }
    }

    @objc open var text: String? {
        get { _text }
        set {
            _text = newValue
            _attributedText = newValue.map { NSAttributedString(string: $0) }
            setNeedsDisplay()
        }
    }

    @objc open var attributedText: NSAttributedString? {
        get { _attributedText }
        set {
            _attributedText = newValue
            _text = newValue?.string
            setNeedsDisplay()
        }
    }

    @objc open var font: UIFont {
        get { _font }
        set { _font = newValue; setNeedsDisplay() }
    }

    @objc open var textColor: UIColor {
        get { _textColor }
        set { _textColor = newValue; setNeedsDisplay() }
    }

    @objc open var textAlignment: Int {
        get { _textAlignment }
        set { _textAlignment = newValue; setNeedsDisplay() }
    }

    @objc open var numberOfLines: Int {
        get { _numberOfLines }
        set { _numberOfLines = newValue; setNeedsDisplay() }
    }

    @objc open var lineBreakMode: Int {
        get { _lineBreakMode }
        set { _lineBreakMode = newValue; setNeedsDisplay() }
    }

    private func buildAttributedString() -> NSAttributedString? {
        guard let text = _text, !text.isEmpty else { return nil }
        let attrs: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): _font.ctFont,
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): _textColor.cgColor,
        ]
        return NSAttributedString(string: text, attributes: attrs)
    }

    open override func sizeThatFits(_ size: CGSize) -> CGSize {
        guard let attr = buildAttributedString() else { return .zero }
        let framesetter = CTFramesetterCreateWithAttributedString(attr)
        let constraint = CGSize(width: size.width > 0 ? size.width : .greatestFiniteMagnitude, height: .greatestFiniteMagnitude)
        return CTFramesetterSuggestFrameSizeWithConstraints(framesetter, CFRange(), nil, constraint, nil)
    }

    open override func draw(_ rect: CGRect) {
        guard let attr = buildAttributedString(), let ctx = UIGraphicsGetCurrentContext() else { return }
        ctx.saveGState()
        ctx.textMatrix = .identity
        ctx.translateBy(x: 0, y: bounds.size.height)
        ctx.scaleBy(x: 1, y: -1)
        let path = CGPath(rect: bounds, transform: nil)
        let framesetter = CTFramesetterCreateWithAttributedString(attr)
        let frame = CTFramesetterCreateFrame(framesetter, CFRange(), path, nil)
        CTFrameDraw(frame, ctx)
        ctx.restoreGState()
    }
}

@objc(UIProgressView)
open class UIProgressView: UIView {
    private var _progress: Float = 0
    private var _progressViewStyle: Int = 0
    private var _progressTintColor: UIColor?
    private var _trackTintColor: UIColor?

    @objc(initWithProgressViewStyle:)
    public init(progressViewStyle: Int) {
        _progressViewStyle = progressViewStyle
        super.init(frame: CGRect(x: 0, y: 0, width: 100, height: 4))
    }

    @objc(initWithFrame:)
    public override init(frame: CGRect) {
        super.init(frame: frame)
    }

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        super.init(coder: coder)
        if coder.containsValue(forKey: "UIProgress") {
            _progress = coder.decodeFloat(forKey: "UIProgress")
        }
    }

    @objc open var progress: Float {
        get { _progress }
        set { _progress = min(1.0, max(0.0, newValue)); setNeedsDisplay() }
    }

    @objc(setProgress:animated:)
    open func setProgress(_ progress: Float, animated: Bool) {
        self.progress = progress
    }

    @objc open var progressViewStyle: Int {
        get { _progressViewStyle }
        set { _progressViewStyle = newValue }
    }

    @objc open var progressTintColor: UIColor? {
        get { _progressTintColor }
        set { _progressTintColor = newValue; setNeedsDisplay() }
    }

    @objc open var trackTintColor: UIColor? {
        get { _trackTintColor }
        set { _trackTintColor = newValue; setNeedsDisplay() }
    }
}
