// UIView, UIWindow, UIViewController.
// Clean-room: behavior from public UIKit documentation (view/layer
// relationship, responder chain, view controller lifecycle).
#import "PearUIKitPrivate.h"
#import <objc/runtime.h>

// ---- UIView --------------------------------------------------------------------

@implementation UIView {
    CALayer *_layer;
    __weak UIView *_superview;
    NSMutableArray<UIView *> *_subviews;
    __weak UIWindow *_window;
    __weak UIViewController *_viewController;
    UIColor *_backgroundColor;
    NSInteger _tag;
    UIViewAutoresizing _autoresizing;
    BOOL _multipleTouch, _userInteraction, _clearsContext, _needsLayout;
}

+ (Class)layerClass { return [CALayer class]; }

- (void)pear_commonInit:(CGRect)frame {
    _subviews = [NSMutableArray array];
    _userInteraction = YES;
    _clearsContext = YES;
    _layer = [[[self class] layerClass] layer];
    _layer.delegate = (id<CALayerDelegate>)self;
    _layer.anchorPoint = CGPointMake(0.5, 0.5);
    _layer.contentsScale = UIScreen.mainScreen.scale;
    _layer.geometryFlipped = NO;
    self.frame = frame;
}

- (instancetype)init { return [self initWithFrame:CGRectZero]; }
- (instancetype)initWithFrame:(CGRect)frame {
    if ((self = [super init])) [self pear_commonInit:frame];
    return self;
}
- (instancetype)initWithCoder:(NSCoder *)coder {
    if ((self = [super init])) [self pear_commonInit:CGRectZero];
    return self;
}

- (CALayer *)layer { return _layer; }

// CALayerDelegate: suppress implicit animations, as UIKit does for its layers
// outside animation blocks.
- (id<CAAction>)actionForLayer:(CALayer *)layer forKey:(NSString *)event { return (id<CAAction>)[NSNull null]; }

- (CGRect)frame { return _layer.frame; }
- (void)setFrame:(CGRect)frame {
    CGSize old = _layer.bounds.size;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _layer.frame = frame;
    [CATransaction commit];
    if (!CGSizeEqualToSize(old, frame.size)) [self pear_boundsSizeChangedFrom:old];
}
- (CGRect)bounds { return _layer.bounds; }
- (void)setBounds:(CGRect)bounds {
    CGSize old = _layer.bounds.size;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _layer.bounds = bounds;
    [CATransaction commit];
    if (!CGSizeEqualToSize(old, bounds.size)) [self pear_boundsSizeChangedFrom:old];
}
- (CGPoint)center { return _layer.position; }
- (void)setCenter:(CGPoint)c {
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _layer.position = c;
    [CATransaction commit];
}

- (void)pear_boundsSizeChangedFrom:(CGSize)old {
    CGSize now = _layer.bounds.size;
    CGFloat dw = now.width - old.width, dh = now.height - old.height;
    for (UIView *v in _subviews) {
        UIViewAutoresizing m = v->_autoresizing;
        if (m == UIViewAutoresizingNone) continue;
        CGRect f = v.frame;
        if (m & UIViewAutoresizingFlexibleWidth) f.size.width += dw;
        else if (m & UIViewAutoresizingFlexibleLeftMargin) f.origin.x += dw;
        if (m & UIViewAutoresizingFlexibleHeight) f.size.height += dh;
        else if (m & UIViewAutoresizingFlexibleTopMargin) f.origin.y += dh;
        v.frame = f;
    }
    [self setNeedsLayout];
}

- (UIView *)superview { return _superview; }
- (NSArray<UIView *> *)subviews { return [_subviews copy]; }
- (UIWindow *)window { return _window; }

- (UIResponder *)nextResponder {
    if (_viewController) return _viewController;
    return _superview;
}

- (void)pear_setViewController:(UIViewController *)vc { _viewController = vc; }

- (void)pear_setWindow:(UIWindow *)window {
    if (_window == window) return;
    [self willMoveToWindow:window];
    _window = window;
    for (UIView *v in _subviews) [v pear_setWindow:window];
    [self didMoveToWindow];
}

- (void)insertSubview:(UIView *)view atIndex:(NSInteger)index {
    if (!view || view == self) return;
    if (view->_superview) [view removeFromSuperview];
    [view willMoveToSuperview:self];
    index = MAX(0, MIN(index, (NSInteger)_subviews.count));
    [_subviews insertObject:view atIndex:(NSUInteger)index];
    [_layer insertSublayer:view.layer atIndex:(unsigned)index];
    view->_superview = self;
    [view pear_setWindow:_window];
    [view didMoveToSuperview];
    [self didAddSubview:view];
    [self setNeedsLayout];
}
- (void)addSubview:(UIView *)view { [self insertSubview:view atIndex:(NSInteger)_subviews.count]; }
- (void)bringSubviewToFront:(UIView *)view {
    if (view->_superview != self) return;
    [_subviews removeObject:view];
    [_subviews addObject:view];
    [view.layer removeFromSuperlayer];
    [_layer addSublayer:view.layer];
}
- (void)sendSubviewToBack:(UIView *)view {
    if (view->_superview != self) return;
    [_subviews removeObject:view];
    [_subviews insertObject:view atIndex:0];
    [view.layer removeFromSuperlayer];
    [_layer insertSublayer:view.layer atIndex:0];
}
- (void)removeFromSuperview {
    UIView *parent = _superview;
    if (!parent) return;
    [parent willRemoveSubview:self];
    [self willMoveToSuperview:nil];
    [parent->_subviews removeObject:self];
    [_layer removeFromSuperlayer];
    _superview = nil;
    [self pear_setWindow:nil];
    [self didMoveToSuperview];
}
- (void)didAddSubview:(UIView *)v {}
- (void)willRemoveSubview:(UIView *)v {}
- (void)willMoveToSuperview:(UIView *)s {}
- (void)didMoveToSuperview {}
- (void)willMoveToWindow:(UIWindow *)w {}
- (void)didMoveToWindow {}
- (BOOL)isDescendantOfView:(UIView *)view {
    for (UIView *v = self; v; v = v->_superview)
        if (v == view) return YES;
    return NO;
}

- (UIView *)viewWithTag:(NSInteger)tag {
    if (_tag == tag) return self;
    for (UIView *v in _subviews) {
        UIView *hit = [v viewWithTag:tag];
        if (hit) return hit;
    }
    return nil;
}
- (NSInteger)tag { return _tag; }
- (void)setTag:(NSInteger)tag { _tag = tag; }

- (void)setNeedsLayout {
    _needsLayout = YES;
    __weak UIView *weakSelf = self;
    dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf layoutIfNeeded]; });
}
- (void)layoutIfNeeded {
    if (!_needsLayout) return;
    _needsLayout = NO;
    UIViewController *vc = _viewController;
    [vc viewWillLayoutSubviews];
    [self layoutSubviews];
    [vc viewDidLayoutSubviews];
    for (UIView *v in _subviews) [v layoutIfNeeded];
}
- (void)layoutSubviews {}
- (void)setNeedsDisplay { [_layer setNeedsDisplay]; }
- (void)setNeedsDisplayInRect:(CGRect)r { [_layer setNeedsDisplayInRect:r]; }
- (void)drawRect:(CGRect)rect {}
- (CGSize)sizeThatFits:(CGSize)size { return self.bounds.size; }
- (void)sizeToFit {
    CGRect f = self.frame;
    f.size = [self sizeThatFits:f.size];
    self.frame = f;
}

- (CGFloat)contentScaleFactor { return _layer.contentsScale; }
- (void)setContentScaleFactor:(CGFloat)s { _layer.contentsScale = s; }
- (BOOL)isMultipleTouchEnabled { return _multipleTouch; }
- (void)setMultipleTouchEnabled:(BOOL)e { _multipleTouch = e; }
- (BOOL)isExclusiveTouch { return NO; }
- (void)setExclusiveTouch:(BOOL)e {}
- (BOOL)isUserInteractionEnabled { return _userInteraction; }
- (void)setUserInteractionEnabled:(BOOL)e { _userInteraction = e; }
- (BOOL)isHidden { return _layer.hidden; }
- (void)setHidden:(BOOL)h { _layer.hidden = h; }
- (BOOL)isOpaque { return _layer.opaque; }
- (void)setOpaque:(BOOL)o { _layer.opaque = o; }
- (CGFloat)alpha { return _layer.opacity; }
- (void)setAlpha:(CGFloat)a { _layer.opacity = (float)a; }
- (UIViewAutoresizing)autoresizingMask { return _autoresizing; }
- (void)setAutoresizingMask:(UIViewAutoresizing)m { _autoresizing = m; }
- (BOOL)autoresizesSubviews { return YES; }
- (void)setAutoresizesSubviews:(BOOL)a {}
- (BOOL)clearsContextBeforeDrawing { return _clearsContext; }
- (void)setClearsContextBeforeDrawing:(BOOL)c { _clearsContext = c; }
- (BOOL)clipsToBounds { return _layer.masksToBounds; }
- (void)setClipsToBounds:(BOOL)c { _layer.masksToBounds = c; }
- (UIColor *)backgroundColor { return _backgroundColor; }
- (void)setBackgroundColor:(UIColor *)c {
    _backgroundColor = c;
    _layer.backgroundColor = c.CGColor;
}
- (NSInteger)contentMode { return 0; }
- (void)setContentMode:(NSInteger)m {}
- (UIEdgeInsets)safeAreaInsets { return (UIEdgeInsets){0, 0, 0, 0}; }
- (UIEdgeInsets)layoutMargins { return (UIEdgeInsets){8, 8, 8, 8}; }
- (UITraitCollection *)traitCollection { return [UITraitCollection pear_current]; }
- (NSArray *)gestureRecognizers { return @[]; }
- (void)addGestureRecognizer:(id)g {
    PearMissingReport(PearMissingBehavior, "UIView", "gesture recognizers are ignored", __builtin_return_address(0));
}
- (void)removeGestureRecognizer:(id)g {}
- (void)setTranslatesAutoresizingMaskIntoConstraints:(BOOL)t {}
- (BOOL)translatesAutoresizingMaskIntoConstraints { return YES; }

// Hit testing and coordinate conversion (frames are layer frames; no transforms
// beyond what CALayer applies).
- (BOOL)pointInside:(CGPoint)p withEvent:(UIEvent *)e { return CGRectContainsPoint(self.bounds, p); }
- (UIView *)hitTest:(CGPoint)p withEvent:(UIEvent *)e {
    if (self.hidden || !_userInteraction || self.alpha < 0.01) return nil;
    if (![self pointInside:p withEvent:e]) return nil;
    for (UIView *v in [_subviews reverseObjectEnumerator]) {
        UIView *hit = [v hitTest:[self convertPoint:p toView:v] withEvent:e];
        if (hit) return hit;
    }
    return self;
}
- (CGPoint)convertPoint:(CGPoint)p toView:(UIView *)view {
    CALayer *target = view ? view.layer : (_window ? _window.layer : nil);
    return target ? [_layer convertPoint:p toLayer:target] : p;
}
- (CGPoint)convertPoint:(CGPoint)p fromView:(UIView *)view {
    CALayer *source = view ? view.layer : (_window ? _window.layer : nil);
    return source ? [_layer convertPoint:p fromLayer:source] : p;
}
- (CGRect)convertRect:(CGRect)r toView:(UIView *)view {
    CALayer *target = view ? view.layer : (_window ? _window.layer : nil);
    return target ? [_layer convertRect:r toLayer:target] : r;
}
- (CGRect)convertRect:(CGRect)r fromView:(UIView *)view {
    CALayer *source = view ? view.layer : (_window ? _window.layer : nil);
    return source ? [_layer convertRect:r fromLayer:source] : r;
}

// Accessibility: no assistive technology is attached on this host.
- (BOOL)isAccessibilityElement { return NO; }
- (void)setIsAccessibilityElement:(BOOL)a {}
- (void)setAccessibilityLabel:(NSString *)l {}
- (void)setAccessibilityTraits:(uint64_t)t {}
- (void)setAccessibilityElements:(NSArray *)e {}
- (BOOL)endEditing:(BOOL)force { return YES; }
@end

// ---- UIViewController -----------------------------------------------------------

@implementation UIViewController {
    UIView *_view;
    NSString *_title;
    UIModalPresentationStyle _modalStyle;
    UIRectEdge _edges;
    UIViewController *_presented;
    __weak UIViewController *_presenting;
}
- (instancetype)init { return [self initWithNibName:nil bundle:nil]; }
- (instancetype)initWithNibName:(NSString *)nibName bundle:(NSBundle *)bundle {
    if ((self = [super init])) {
        _edges = UIRectEdgeAll;
        if (nibName)
            PearMissingReport(PearMissingBehavior, "UIViewController", "nib loading", __builtin_return_address(0));
    }
    return self;
}
- (instancetype)initWithCoder:(NSCoder *)coder { return [self initWithNibName:nil bundle:nil]; }

- (void)loadView { self.view = [[UIView alloc] initWithFrame:UIScreen.mainScreen.bounds]; }
- (UIView *)view {
    if (!_view) {
        [self loadView];
        if (!_view) self.view = [[UIView alloc] initWithFrame:UIScreen.mainScreen.bounds];
        [self viewDidLoad];
    }
    return _view;
}
- (void)setView:(UIView *)view {
    [_view pear_setViewController:nil];
    _view = view;
    [view pear_setViewController:self];
}
- (UIView *)viewIfLoaded { return _view; }
- (BOOL)isViewLoaded { return _view != nil; }
- (void)loadViewIfNeeded { (void)self.view; }
- (UIResponder *)nextResponder { return _view.superview ?: (UIResponder *)_view.window; }

- (void)viewDidLoad {}
- (void)viewWillAppear:(BOOL)a {}
- (void)viewDidAppear:(BOOL)a {}
- (void)viewWillDisappear:(BOOL)a {}
- (void)viewDidDisappear:(BOOL)a {}
- (void)viewWillLayoutSubviews {}
- (void)viewDidLayoutSubviews {}
- (void)viewDidUnload {}
- (void)didReceiveMemoryWarning {}
- (void)viewWillTransitionToSize:(CGSize)size withTransitionCoordinator:(id)c {}
- (void)traitCollectionDidChange:(UITraitCollection *)previous {}

- (NSString *)title { return _title; }
- (void)setTitle:(NSString *)t { _title = [t copy]; }
- (UIModalPresentationStyle)modalPresentationStyle { return _modalStyle; }
- (void)setModalPresentationStyle:(UIModalPresentationStyle)s { _modalStyle = s; }
- (UIRectEdge)edgesForExtendedLayout { return _edges; }
- (void)setEdgesForExtendedLayout:(UIRectEdge)e { _edges = e; }
- (UITraitCollection *)traitCollection { return [UITraitCollection pear_current]; }
- (UIViewController *)parentViewController { return nil; }
- (NSArray *)childViewControllers { return @[]; }
- (id)navigationController { return nil; }
- (id)tabBarController { return nil; }

- (BOOL)prefersStatusBarHidden { return YES; }
- (BOOL)prefersHomeIndicatorAutoHidden { return YES; }
- (BOOL)prefersPointerLocked { return NO; }
- (BOOL)shouldAutorotate { return YES; }
- (UIInterfaceOrientationMask)supportedInterfaceOrientations { return UIInterfaceOrientationMaskLandscape; }
- (UIInterfaceOrientation)preferredInterfaceOrientationForPresentation { return UIInterfaceOrientationLandscapeRight; }
- (UIRectEdge)preferredScreenEdgesDeferringSystemGestures { return UIRectEdgeNone; }
- (void)setNeedsUpdateOfPrefersPointerLocked {}
- (void)setNeedsUpdateOfHomeIndicatorAutoHidden {}
- (void)setNeedsUpdateOfScreenEdgesDeferringSystemGestures {}
- (void)setNeedsStatusBarAppearanceUpdate {}

- (UIViewController *)presentedViewController { return _presented; }
- (UIViewController *)presentingViewController { return _presenting; }
- (void)pear_setPresenting:(UIViewController *)vc { _presenting = vc; }

- (void)presentViewController:(UIViewController *)vc animated:(BOOL)animated completion:(void (^)(void))completion {
    if ([vc isKindOfClass:[UIAlertController class]]) {
        [(UIAlertController *)vc pear_presentWithCompletion:completion];
        return;
    }
    // Modal presentation of arbitrary controllers is not hosted yet; the
    // presented controller's view is layered over the presenter's window.
    PearMissingReport(PearMissingBehavior, class_getName([vc class]), "presentViewController: overlays the view only",
                      __builtin_return_address(0));
    _presented = vc;
    [vc pear_setPresenting:self];
    UIWindow *w = _view.window;
    if (w) {
        UIView *v = vc.view;
        v.frame = w.bounds;
        [vc viewWillAppear:animated];
        [w addSubview:v];
        [vc viewDidAppear:animated];
    }
    if (completion) dispatch_async(dispatch_get_main_queue(), completion);
}
- (void)dismissViewControllerAnimated:(BOOL)animated completion:(void (^)(void))completion {
    UIViewController *target = _presented ?: self;
    UIViewController *presenter = _presented ? self : _presenting;
    [target viewWillDisappear:animated];
    [target.viewIfLoaded removeFromSuperview];
    [target viewDidDisappear:animated];
    [target pear_setPresenting:nil];
    if (presenter) presenter->_presented = nil;
    if (completion) dispatch_async(dispatch_get_main_queue(), completion);
}
@end

// ---- UIWindow ---------------------------------------------------------------------

@implementation UIWindow {
    void *_native;
    UIViewController *_root;
    UIScreen *_screen;
    CGFloat _level;
}
- (instancetype)initWithFrame:(CGRect)frame {
    if ((self = [super initWithFrame:frame])) {
        _screen = UIScreen.mainScreen;
        [self pear_setWindow:self];
        PearUIKitRegisterWindow(self);
    }
    return self;
}
- (instancetype)initWithWindowScene:(id)scene { return [self initWithFrame:UIScreen.mainScreen.bounds]; }
- (UIWindow *)window { return self; }
- (UIResponder *)nextResponder { return [UIApplication sharedApplication]; }
- (UIScreen *)screen { return _screen; }
- (void)setScreen:(UIScreen *)s { _screen = s ?: UIScreen.mainScreen; }
- (CGFloat)windowLevel { return _level; }
- (void)setWindowLevel:(CGFloat)l { _level = l; }
- (id)windowScene { return nil; }
- (void)setWindowScene:(id)s {}
- (BOOL)isKeyWindow { return PearUIKitKeyWindow() == self; }

- (UIViewController *)rootViewController { return _root; }
- (void)setRootViewController:(UIViewController *)vc {
    if (_root == vc) return;
    [_root.viewIfLoaded removeFromSuperview];
    _root = vc;
    if (vc) {
        UIView *v = vc.view;
        v.frame = self.bounds;
        v.autoresizingMask = UIViewAutoresizingFlexibleWidth | UIViewAutoresizingFlexibleHeight;
        [vc viewWillAppear:NO];
        [self addSubview:v];
        [vc viewDidAppear:NO];
    }
}

- (void)pear_ensureNative {
    if (_native) return;
    _native = PearBackendCreateWindow(self, self.bounds);
    // The window's layer is positioned by the backend, not by UIKit frames.
    self.layer.anchorPoint = CGPointZero;
    self.layer.position = CGPointZero;
    PearBackendSetRootLayer(_native, self.layer);
}
- (void)makeKeyWindow {
    [self pear_ensureNative];
    PearUIKitSetKeyWindow(self);
}
- (void)makeKeyAndVisible {
    [self pear_ensureNative];
    self.hidden = NO;
    PearBackendShowWindow(_native, YES);
    PearUIKitSetKeyWindow(self);
    [self setNeedsLayout];
}
- (void)setHidden:(BOOL)hidden {
    [super setHidden:hidden];
    if (!hidden && _native) PearBackendShowWindow(_native, NO);
}
- (void)becomeKeyWindow {}
- (void)resignKeyWindow {}
- (void)sendEvent:(UIEvent *)event {}
@end

void PearUIKitWindowDidResize(UIWindow *window, CGSize pointSize, CGFloat scale) {
    PearUIKitScreenDidChange(pointSize, scale);
    window.frame = CGRectMake(0, 0, pointSize.width, pointSize.height);
    window.layer.anchorPoint = CGPointZero;
    window.layer.position = CGPointZero;
    PearHostLog("host", "window resized to %.0fx%.0f pt @%.1fx", pointSize.width, pointSize.height, scale);
}
