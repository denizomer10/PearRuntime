// Hardware keyboard and text input: UIKey / UIPress / UIPressesEvent delivery
// to the first responder, UITextView, and the keyboard notifications.
// Clean-room: from the public UIKey/UIPress/UITextView/UIKeyInput documentation;
// key codes are USB HID usages as documented for UIKeyboardHIDUsage.
#import "PearUIKitPrivate.h"
#import <objc/message.h>
#import <objc/runtime.h>

PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardWillShowNotification = @"UIKeyboardWillShowNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardDidShowNotification = @"UIKeyboardDidShowNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardWillHideNotification = @"UIKeyboardWillHideNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardDidHideNotification = @"UIKeyboardDidHideNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardWillChangeFrameNotification = @"UIKeyboardWillChangeFrameNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UIKeyboardDidChangeFrameNotification = @"UIKeyboardDidChangeFrameNotification";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardFrameBeginUserInfoKey = @"UIKeyboardFrameBeginUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardFrameEndUserInfoKey = @"UIKeyboardFrameEndUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardAnimationDurationUserInfoKey = @"UIKeyboardAnimationDurationUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardAnimationCurveUserInfoKey = @"UIKeyboardAnimationCurveUserInfoKey";
PEAR_UIKIT_EXPORT NSString *const UIKeyboardIsLocalUserInfoKey = @"UIKeyboardIsLocalUserInfoKey";
PEAR_UIKIT_EXPORT NSNotificationName const UITextViewTextDidBeginEditingNotification = @"UITextViewTextDidBeginEditingNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UITextViewTextDidChangeNotification = @"UITextViewTextDidChangeNotification";
PEAR_UIKIT_EXPORT NSNotificationName const UITextViewTextDidEndEditingNotification = @"UITextViewTextDidEndEditingNotification";

// ---- UIKey / UIPress ------------------------------------------------------------

@implementation UIKey {
    NSString *_characters, *_charactersIgnoringModifiers;
    NSInteger _modifierFlags, _keyCode;
}
- (instancetype)initWithCharacters:(NSString *)c ignoringModifiers:(NSString *)ci modifiers:(NSInteger)m keyCode:(NSInteger)k {
    if ((self = [super init])) {
        _characters = [c copy] ?: @"";
        _charactersIgnoringModifiers = [ci copy] ?: @"";
        _modifierFlags = m;
        _keyCode = k;
    }
    return self;
}
- (NSString *)characters { return _characters; }
- (NSString *)charactersIgnoringModifiers { return _charactersIgnoringModifiers; }
- (NSInteger)modifierFlags { return _modifierFlags; }
- (NSInteger)keyCode { return _keyCode; }
- (id)copyWithZone:(NSZone *)zone { return self; }
@end

@implementation UIPress {
    UIKey *_key;
    NSInteger _phase, _type;
    NSTimeInterval _timestamp;
}
- (instancetype)initWithKey:(UIKey *)key type:(NSInteger)type phase:(NSInteger)phase timestamp:(NSTimeInterval)t {
    if ((self = [super init])) {
        _key = key;
        _type = type;
        _phase = phase;
        _timestamp = t;
    }
    return self;
}
- (UIKey *)key { return _key; }
- (NSInteger)phase { return _phase; }
- (NSInteger)type { return _type; }
- (NSTimeInterval)timestamp { return _timestamp; }
- (CGFloat)force { return _phase == 3 ? 0 : 1; }  // ended -> 0
- (UIResponder *)responder { return nil; }
- (UIWindow *)window { return PearUIKitKeyWindow(); }
- (NSArray *)gestureRecognizers { return @[]; }
@end

@implementation UIPressesEvent {
    NSSet *_presses;
    NSTimeInterval _ts;
}
- (instancetype)initWithPresses:(NSSet *)presses timestamp:(NSTimeInterval)t {
    if ((self = [super init])) {
        _presses = presses;
        _ts = t;
    }
    return self;
}
- (UIEventType)type { return UIEventTypePresses; }
- (NSTimeInterval)timestamp { return _ts; }
- (NSSet *)allPresses { return _presses; }
- (NSSet *)pressesForGestureRecognizer:(id)g { return [NSSet set]; }
- (NSSet *)allTouches { return nil; }
@end

// ---- keyboard visibility ------------------------------------------------------------

static BOOL gSoftKeyboardShown;

// There is no on-screen keyboard on a Mac: report a zero-height keyboard at the
// bottom edge, so layout code that subtracts its height leaves the UI in place.
static void postKeyboard(BOOL show) {
    if (gSoftKeyboardShown == show) return;
    gSoftKeyboardShown = show;
    CGRect screen = UIScreen.mainScreen.bounds;
    CGRect frame = CGRectMake(0, screen.size.height, screen.size.width, 0);
    NSDictionary *info = @{
        UIKeyboardFrameBeginUserInfoKey : [NSValue valueWithBytes:&frame objCType:@encode(CGRect)],
        UIKeyboardFrameEndUserInfoKey : [NSValue valueWithBytes:&frame objCType:@encode(CGRect)],
        UIKeyboardAnimationDurationUserInfoKey : @0.0,
        UIKeyboardAnimationCurveUserInfoKey : @7,
        UIKeyboardIsLocalUserInfoKey : @YES,
    };
    NSNotificationCenter *nc = NSNotificationCenter.defaultCenter;
    [nc postNotificationName:show ? UIKeyboardWillShowNotification : UIKeyboardWillHideNotification object:nil userInfo:info];
    [nc postNotificationName:UIKeyboardWillChangeFrameNotification object:nil userInfo:info];
    [nc postNotificationName:show ? UIKeyboardDidShowNotification : UIKeyboardDidHideNotification object:nil userInfo:info];
    [nc postNotificationName:UIKeyboardDidChangeFrameNotification object:nil userInfo:info];
    PearHostLog("keyboard", "text input %s", show ? "started" : "ended");
}

// ---- UITextView ---------------------------------------------------------------------

@implementation UITextView {
    NSMutableString *_text;
    NSRange _selected;
    __weak id _textDelegate;
    BOOL _editable, _editing;
    NSInteger _keyboardType, _returnKeyType, _autocap, _autocorrect, _spell, _smartQuotes, _smartDashes;
    BOOL _secure;
    UIColor *_textColor;
    id _font;
}
- (instancetype)initWithFrame:(CGRect)frame {
    if ((self = [super initWithFrame:frame])) {
        _text = [NSMutableString string];
        _editable = YES;
    }
    return self;
}
- (instancetype)initWithFrame:(CGRect)frame textContainer:(id)container { return [self initWithFrame:frame]; }
- (instancetype)initWithCoder:(NSCoder *)coder {
    if ((self = [super initWithCoder:coder])) {
        _text = [NSMutableString stringWithString:[coder decodeObjectForKey:@"UIText"] ?: @""];
        _editable = YES;
    }
    return self;
}

- (id)delegate { return _textDelegate; }
- (void)setDelegate:(id)d { _textDelegate = d; }
- (NSString *)text { return [_text copy]; }
- (void)setText:(NSString *)text {
    [_text setString:text ?: @""];
    _selected = NSMakeRange(_text.length, 0);
}
- (NSAttributedString *)attributedText { return [[NSAttributedString alloc] initWithString:_text]; }
- (void)setAttributedText:(NSAttributedString *)t { self.text = t.string; }
- (NSRange)selectedRange { return _selected; }
- (void)setSelectedRange:(NSRange)r {
    NSUInteger len = _text.length;
    if (r.location > len) r.location = len;
    if (NSMaxRange(r) > len) r.length = len - r.location;
    _selected = r;
}
- (BOOL)isEditable { return _editable; }
- (void)setEditable:(BOOL)e { _editable = e; }
- (BOOL)isSelectable { return YES; }
- (void)setSelectable:(BOOL)s {}
- (BOOL)hasText { return _text.length > 0; }
- (id)font { return _font; }
- (void)setFont:(id)f { _font = f; }
- (UIColor *)textColor { return _textColor; }
- (void)setTextColor:(UIColor *)c { _textColor = c; }
- (NSInteger)textAlignment { return 0; }
- (void)setTextAlignment:(NSInteger)a {}
- (BOOL)isScrollEnabled { return NO; }
- (void)setScrollEnabled:(BOOL)s {}
- (id)textContainer { return nil; }
- (UIEdgeInsets)textContainerInset { return (UIEdgeInsets){0, 0, 0, 0}; }
- (void)setTextContainerInset:(UIEdgeInsets)i {}
- (void)scrollRangeToVisible:(NSRange)r {}
- (id)inputDelegate { return nil; }
- (void)setInputDelegate:(id)d {}

// UITextInputTraits: stored, no effect on a hardware keyboard.
- (NSInteger)keyboardType { return _keyboardType; }
- (void)setKeyboardType:(NSInteger)v { _keyboardType = v; }
- (NSInteger)returnKeyType { return _returnKeyType; }
- (void)setReturnKeyType:(NSInteger)v { _returnKeyType = v; }
- (NSInteger)autocapitalizationType { return _autocap; }
- (void)setAutocapitalizationType:(NSInteger)v { _autocap = v; }
- (NSInteger)autocorrectionType { return _autocorrect; }
- (void)setAutocorrectionType:(NSInteger)v { _autocorrect = v; }
- (NSInteger)spellCheckingType { return _spell; }
- (void)setSpellCheckingType:(NSInteger)v { _spell = v; }
- (NSInteger)smartQuotesType { return _smartQuotes; }
- (void)setSmartQuotesType:(NSInteger)v { _smartQuotes = v; }
- (NSInteger)smartDashesType { return _smartDashes; }
- (void)setSmartDashesType:(NSInteger)v { _smartDashes = v; }
- (NSInteger)smartInsertDeleteType { return 0; }
- (void)setSmartInsertDeleteType:(NSInteger)v {}
- (NSInteger)keyboardAppearance { return 0; }
- (void)setKeyboardAppearance:(NSInteger)v {}
- (BOOL)enablesReturnKeyAutomatically { return NO; }
- (void)setEnablesReturnKeyAutomatically:(BOOL)v {}
- (BOOL)isSecureTextEntry { return _secure; }
- (void)setSecureTextEntry:(BOOL)v { _secure = v; }
- (id)textContentType { return nil; }
- (void)setTextContentType:(id)t {}

- (BOOL)canBecomeFirstResponder { return _editable; }
- (BOOL)becomeFirstResponder {
    if (_editing) return YES;
    id d = _textDelegate;
    if ([d respondsToSelector:@selector(textViewShouldBeginEditing:)] &&
        !((BOOL(*)(id, SEL, id))objc_msgSend)(d, @selector(textViewShouldBeginEditing:), self))
        return NO;
    if (![super becomeFirstResponder]) return NO;
    _editing = YES;
    postKeyboard(YES);
    if ([d respondsToSelector:@selector(textViewDidBeginEditing:)])
        ((void (*)(id, SEL, id))objc_msgSend)(d, @selector(textViewDidBeginEditing:), self);
    [NSNotificationCenter.defaultCenter postNotificationName:UITextViewTextDidBeginEditingNotification object:self];
    return YES;
}
- (BOOL)resignFirstResponder {
    if (!_editing) return [super resignFirstResponder];
    id d = _textDelegate;
    if ([d respondsToSelector:@selector(textViewShouldEndEditing:)] &&
        !((BOOL(*)(id, SEL, id))objc_msgSend)(d, @selector(textViewShouldEndEditing:), self))
        return NO;
    [super resignFirstResponder];
    _editing = NO;
    if ([d respondsToSelector:@selector(textViewDidEndEditing:)])
        ((void (*)(id, SEL, id))objc_msgSend)(d, @selector(textViewDidEndEditing:), self);
    [NSNotificationCenter.defaultCenter postNotificationName:UITextViewTextDidEndEditingNotification object:self];
    postKeyboard(NO);
    return YES;
}
- (void)removeFromSuperview {
    if (_editing) [self resignFirstResponder];
    [super removeFromSuperview];
}

// The documented editing contract: ask the delegate, apply, then notify.
- (void)pear_replaceRange:(NSRange)range with:(NSString *)replacement {
    if (!_editable) return;
    [self setSelectedRange:_selected];
    if (NSMaxRange(range) > _text.length) return;
    id d = _textDelegate;
    if ([d respondsToSelector:@selector(textView:shouldChangeTextInRange:replacementText:)] &&
        !((BOOL(*)(id, SEL, id, NSRange, id))objc_msgSend)(d, @selector(textView:shouldChangeTextInRange:replacementText:), self,
                                                          range, replacement)) {
        if (getenv("PEAR_HOST_TRACE_INPUT") && *getenv("PEAR_HOST_TRACE_INPUT") == '1')
            PearHostLog("input", "delegate declined replacing %lu chars with \"%s\"", (unsigned long)range.length,
                        [replacement stringByReplacingOccurrencesOfString:@"\n" withString:@"\\n"].UTF8String);
        return;
    }
    [_text replaceCharactersInRange:range withString:replacement];
    if (getenv("PEAR_HOST_TRACE_INPUT") && *getenv("PEAR_HOST_TRACE_INPUT") == '1')
        PearHostLog("input", "text view now \"%s\"", _text.UTF8String);
    _selected = NSMakeRange(range.location + replacement.length, 0);
    if ([d respondsToSelector:@selector(textViewDidChange:)])
        ((void (*)(id, SEL, id))objc_msgSend)(d, @selector(textViewDidChange:), self);
    if ([d respondsToSelector:@selector(textViewDidChangeSelection:)])
        ((void (*)(id, SEL, id))objc_msgSend)(d, @selector(textViewDidChangeSelection:), self);
    [NSNotificationCenter.defaultCenter postNotificationName:UITextViewTextDidChangeNotification object:self];
}
// UIKeyInput
- (void)insertText:(NSString *)text { [self pear_replaceRange:_selected with:text]; }
- (void)deleteBackward {
    NSRange r = _selected;
    if (r.length == 0) {
        if (r.location == 0) return;
        r = [_text rangeOfComposedCharacterSequenceAtIndex:r.location - 1];
    }
    [self pear_replaceRange:r with:@""];
}
- (void)pear_moveCursor:(NSInteger)delta {
    NSInteger loc = (NSInteger)_selected.location + delta;
    [self setSelectedRange:NSMakeRange((NSUInteger)MAX(0, loc), 0)];
    id d = _textDelegate;
    if ([d respondsToSelector:@selector(textViewDidChangeSelection:)])
        ((void (*)(id, SEL, id))objc_msgSend)(d, @selector(textViewDidChangeSelection:), self);
}
@end

// ---- routing from the backend ------------------------------------------------------------

static UIResponder *pressTarget(void) {
    UIResponder *r = PearUIKitFirstResponder();
    if (r) return r;
    UIWindow *w = PearUIKitKeyWindow();
    UIViewController *vc = w.rootViewController;
    return vc.viewIfLoaded ?: (UIResponder *)vc ?: (UIResponder *)w;
}

static BOOL isTextResponder(UIResponder *r) {
    return [r respondsToSelector:@selector(insertText:)] && [r respondsToSelector:@selector(deleteBackward)] &&
           [r respondsToSelector:@selector(hasText)];
}

BOOL PearUIKitTextInputActive(void) { return isTextResponder(PearUIKitFirstResponder()); }

void PearUIKitDeliverText(NSString *text) {
    UIResponder *r = PearUIKitFirstResponder();
    if (isTextResponder(r)) [(id)r insertText:text];
}

void PearUIKitDeliverEditCommand(PearEditCommand command) {
    UIResponder *r = PearUIKitFirstResponder();
    if (!isTextResponder(r)) return;
    switch (command) {
    case PearEditDeleteBackward: [(id)r deleteBackward]; break;
    case PearEditNewline: [(id)r insertText:@"\n"]; break;
    case PearEditMoveLeft:
        if ([r respondsToSelector:@selector(pear_moveCursor:)]) [(UITextView *)r pear_moveCursor:-1];
        break;
    case PearEditMoveRight:
        if ([r respondsToSelector:@selector(pear_moveCursor:)]) [(UITextView *)r pear_moveCursor:1];
        break;
    case PearEditCancel: [r resignFirstResponder]; break;
    }
}

// Press type for keys that have a documented UIPressType meaning; other keys
// carry no type of their own.
static NSInteger pressTypeForHID(NSInteger usage) {
    switch (usage) {
    case 0x52: return 0;  // up arrow
    case 0x51: return 1;  // down
    case 0x50: return 2;  // left
    case 0x4F: return 3;  // right
    case 0x28: return 4;  // return -> select
    case 0x29: return 5;  // escape -> menu
    default: return 1000;
    }
}

void PearUIKitDeliverKey(NSInteger hidUsage, NSString *chars, NSString *charsNoMods, NSInteger modifiers, BOOL down,
                         NSTimeInterval timestamp) {
    UIKey *key = [[UIKey alloc] initWithCharacters:chars ignoringModifiers:charsNoMods modifiers:modifiers keyCode:hidUsage];
    UIPress *press = [[UIPress alloc] initWithKey:key type:pressTypeForHID(hidUsage) phase:down ? 0 : 3 timestamp:timestamp];
    NSSet *set = [NSSet setWithObject:press];
    UIPressesEvent *event = [[UIPressesEvent alloc] initWithPresses:set timestamp:timestamp];
    UIResponder *target = pressTarget();
    static int trace = -1;
    if (trace < 0) trace = getenv("PEAR_HOST_TRACE_INPUT") && *getenv("PEAR_HOST_TRACE_INPUT") == '1';
    if (trace) {
        NSMutableString *chain = [NSMutableString string];
        for (UIResponder *r = target; r && chain.length < 400; r = r.nextResponder)
            [chain appendFormat:@"%s%@", chain.length ? " > " : "", NSStringFromClass([r class])];
        PearHostLog("input", "key 0x%02lx %s -> %s (first responder: %s)", (long)hidUsage, down ? "down" : "up",
                    chain.UTF8String, PearUIKitFirstResponder() ? class_getName([PearUIKitFirstResponder() class]) : "none");
    }
    if (down) [target pressesBegan:set withEvent:event];
    else [target pressesEnded:set withEvent:event];
}
