// GLKitAdapter.mm: ANGLE-backed GLKView / GLKViewController.
//
// GLKit is a system framework: its GLKView creates and binds renderbuffers
// with the *system* OpenGL ES it links. Handed a PearEAGLContext (ANGLE), it
// bound storage in the system GL while asking our context to allocate it, so
// ANGLE never saw a bound renderbuffer and every frame was refused (GTA: LCS
// stayed black). These original classes implement the public GLKView /
// GLKViewController behaviour on ANGLE; import routing binds the app's class
// references (including subclasses' superclass) to them. No GLKit code is
// used or copied.
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/message.h>
#import "PearEAGLContext.h"
#import "PearANGLEGameRouting.h"
#import "PearANGLEBackend.h"
#include <GLES2/gl2.h>
#include <GLES2/gl2ext.h>

namespace {
struct GLKFunctions {
    void (*genFramebuffers)(GLsizei, GLuint *);
    void (*genRenderbuffers)(GLsizei, GLuint *);
    void (*deleteFramebuffers)(GLsizei, const GLuint *);
    void (*deleteRenderbuffers)(GLsizei, const GLuint *);
    void (*bindFramebuffer)(GLenum, GLuint);
    void (*bindRenderbuffer)(GLenum, GLuint);
    void (*renderbufferStorage)(GLenum, GLenum, GLsizei, GLsizei);
    void (*framebufferRenderbuffer)(GLenum, GLenum, GLenum, GLuint);
    void (*getRenderbufferParameteriv)(GLenum, GLenum, GLint *);
    GLenum (*checkFramebufferStatus)(GLenum);
    void (*viewport)(GLint, GLint, GLsizei, GLsizei);
};
const GLKFunctions *GLK() {
    static GLKFunctions functions;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        // Resolved through the routing table: ANGLE only, started on demand.
        functions.genFramebuffers = (decltype(functions.genFramebuffers))PearANGLEGameSymbol("glGenFramebuffers");
        functions.genRenderbuffers = (decltype(functions.genRenderbuffers))PearANGLEGameSymbol("glGenRenderbuffers");
        functions.deleteFramebuffers = (decltype(functions.deleteFramebuffers))PearANGLEGameSymbol("glDeleteFramebuffers");
        functions.deleteRenderbuffers = (decltype(functions.deleteRenderbuffers))PearANGLEGameSymbol("glDeleteRenderbuffers");
        functions.bindFramebuffer = (decltype(functions.bindFramebuffer))PearANGLEGameSymbol("glBindFramebuffer");
        functions.bindRenderbuffer = (decltype(functions.bindRenderbuffer))PearANGLEGameSymbol("glBindRenderbuffer");
        functions.renderbufferStorage = (decltype(functions.renderbufferStorage))PearANGLEGameSymbol("glRenderbufferStorage");
        functions.framebufferRenderbuffer = (decltype(functions.framebufferRenderbuffer))PearANGLEGameSymbol("glFramebufferRenderbuffer");
        functions.getRenderbufferParameteriv = (decltype(functions.getRenderbufferParameteriv))PearANGLEGameSymbol("glGetRenderbufferParameteriv");
        functions.checkFramebufferStatus = (decltype(functions.checkFramebufferStatus))PearANGLEGameSymbol("glCheckFramebufferStatus");
        functions.viewport = (decltype(functions.viewport))PearANGLEGameSymbol("glViewport");
    });
    return functions.genFramebuffers && functions.bindRenderbuffer && functions.renderbufferStorage ? &functions : nullptr;
}
}  // namespace

// Public GLKit enum values (GLKView.h).
enum { PearGLKColorRGBA8888 = 0, PearGLKColorRGB565 = 1, PearGLKColorSRGBA8888 = 2 };
enum { PearGLKDepthNone = 0, PearGLKDepth16 = 1, PearGLKDepth24 = 2 };
enum { PearGLKStencilNone = 0, PearGLKStencil8 = 1 };

@protocol PearGLKViewDelegate <NSObject>
- (void)glkView:(UIView *)view drawInRect:(CGRect)rect;
@end

@protocol PearGLKViewControllerDelegate <NSObject>
- (void)glkViewControllerUpdate:(UIViewController *)controller;
@end

@interface PearGLKView : UIView
@property(nonatomic, strong) PearEAGLContext *context;
@property(nonatomic, weak) id delegate;
@property(nonatomic) NSInteger drawableColorFormat;
@property(nonatomic) NSInteger drawableDepthFormat;
@property(nonatomic) NSInteger drawableStencilFormat;
@property(nonatomic) NSInteger drawableMultisample;
@property(nonatomic, readonly) NSInteger drawableWidth;
@property(nonatomic, readonly) NSInteger drawableHeight;
@property(nonatomic) BOOL enableSetNeedsDisplay;
@end

@implementation PearGLKView {
    GLuint _framebuffer, _color, _depth;
    CGSize _allocatedPixels;
    BOOL _displayScheduled;
}

+ (Class)layerClass {
    // The drawable host only has to be a CALayer; CAEAGLLayer carries the
    // drawable-properties API engines may set, when the class still exists.
    return NSClassFromString(@"CAEAGLLayer") ?: CALayer.class;
}

- (void)pearSetup {
    _enableSetNeedsDisplay = YES;
    self.opaque = YES;
    self.contentScaleFactor = UIScreen.mainScreen.scale;
}

- (instancetype)initWithFrame:(CGRect)frame { if ((self = [super initWithFrame:frame])) [self pearSetup]; return self; }
- (instancetype)initWithCoder:(NSCoder *)coder { if ((self = [super initWithCoder:coder])) [self pearSetup]; return self; }
- (instancetype)initWithFrame:(CGRect)frame context:(PearEAGLContext *)context {
    if ((self = [self initWithFrame:frame])) _context = context;
    return self;
}

- (void)setContext:(PearEAGLContext *)context {
    if (context == _context) return;
    [self deleteDrawable];
    _context = context;
}

- (NSInteger)drawableWidth { return (NSInteger)_allocatedPixels.width; }
- (NSInteger)drawableHeight { return (NSInteger)_allocatedPixels.height; }

- (void)layoutSubviews {
    [super layoutSubviews];
    // Reallocate on the next display, in the display's own GL context.
    if (_enableSetNeedsDisplay) [self setNeedsDisplay];
}

- (void)setNeedsDisplay {
    [super setNeedsDisplay];
    if (!_enableSetNeedsDisplay || _displayScheduled) return;
    _displayScheduled = YES;
    dispatch_async(dispatch_get_main_queue(), ^{
        self->_displayScheduled = NO;
        [self display];
    });
}

- (BOOL)pearMakeCurrent {
    if (!_context) return NO;
    if (PearEAGLContext.currentContext != _context && ![PearEAGLContext setCurrentContext:_context]) return NO;
    return GLK() != nullptr;
}

- (CGSize)pearWantedPixels {
    CGFloat scale = self.layer.contentsScale > 0 ? self.layer.contentsScale : self.contentScaleFactor;
    return CGSizeMake(round(self.layer.bounds.size.width * scale), round(self.layer.bounds.size.height * scale));
}

- (BOOL)pearEnsureDrawable {
    CGSize wanted = [self pearWantedPixels];
    if (wanted.width < 1 || wanted.height < 1) return NO;
    if (_framebuffer && CGSizeEqualToSize(wanted, _allocatedPixels)) return YES;
    const GLKFunctions *gl = GLK();
    if (!_framebuffer) gl->genFramebuffers(1, &_framebuffer);
    if (!_color) gl->genRenderbuffers(1, &_color);
    gl->bindFramebuffer(GL_FRAMEBUFFER, _framebuffer);
    gl->bindRenderbuffer(GL_RENDERBUFFER, _color);
    if (![_context renderbufferStorage:GL_RENDERBUFFER fromDrawable:self.layer]) {
        _allocatedPixels = CGSizeZero;
        return NO;
    }
    gl->framebufferRenderbuffer(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_RENDERBUFFER, _color);
    GLint width = 0, height = 0;
    gl->getRenderbufferParameteriv(GL_RENDERBUFFER, GL_RENDERBUFFER_WIDTH, &width);
    gl->getRenderbufferParameteriv(GL_RENDERBUFFER, GL_RENDERBUFFER_HEIGHT, &height);
    if (_drawableDepthFormat != PearGLKDepthNone || _drawableStencilFormat != PearGLKStencilNone) {
        if (!_depth) gl->genRenderbuffers(1, &_depth);
        gl->bindRenderbuffer(GL_RENDERBUFFER, _depth);
        BOOL stencil = _drawableStencilFormat != PearGLKStencilNone;
        GLenum format = stencil ? GL_DEPTH24_STENCIL8_OES
            : (_drawableDepthFormat == PearGLKDepth16 ? GL_DEPTH_COMPONENT16 : GL_DEPTH_COMPONENT24_OES);
        gl->renderbufferStorage(GL_RENDERBUFFER, format, width, height);
        gl->framebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, _depth);
        if (stencil) gl->framebufferRenderbuffer(GL_FRAMEBUFFER, GL_STENCIL_ATTACHMENT, GL_RENDERBUFFER, _depth);
        gl->bindRenderbuffer(GL_RENDERBUFFER, _color);
    }
    GLenum status = gl->checkFramebufferStatus(GL_FRAMEBUFFER);
    if (status != GL_FRAMEBUFFER_COMPLETE) {
        PearANGLETrace("[PearRuntime][glkit] framebuffer incomplete 0x%x (%dx%d)\n", status, width, height);
        return NO;
    }
    _allocatedPixels = CGSizeMake(width, height);
    // Allocation follows the layer's bounds; the request is satisfied even
    // when the engine adapts its own pixel size.
    (void)wanted;
    return YES;
}

- (void)bindDrawable {
    if (![self pearMakeCurrent] || ![self pearEnsureDrawable]) return;
    GLK()->bindFramebuffer(GL_FRAMEBUFFER, _framebuffer);
    GLK()->viewport(0, 0, (GLsizei)_allocatedPixels.width, (GLsizei)_allocatedPixels.height);
}

- (void)deleteDrawable {
    if (!_framebuffer && !_color && !_depth) return;
    if ([self pearMakeCurrent]) {
        const GLKFunctions *gl = GLK();
        if (_framebuffer) gl->deleteFramebuffers(1, &_framebuffer);
        GLuint buffers[2] = {_color, _depth};
        gl->deleteRenderbuffers(2, buffers);
    }
    _framebuffer = _color = _depth = 0;
    _allocatedPixels = CGSizeZero;
}

- (void)display {
    if (![self pearMakeCurrent]) return;
    [self bindDrawable];
    if (!_framebuffer || _allocatedPixels.width < 1) return;
    [self drawRect:self.bounds];
    GLK()->bindRenderbuffer(GL_RENDERBUFFER, _color);
    [_context presentRenderbuffer:GL_RENDERBUFFER];
}

- (void)drawRect:(CGRect)rect {
    id delegate = _delegate;
    if ([delegate respondsToSelector:@selector(glkView:drawInRect:)]) [delegate glkView:self drawInRect:rect];
}

- (UIImage *)snapshot { return nil; }

- (void)dealloc { [self deleteDrawable]; }
@end

@interface PearGLKViewController : UIViewController
@property(nonatomic, weak) id delegate;
@property(nonatomic) NSInteger preferredFramesPerSecond;
@property(nonatomic, readonly) NSInteger framesPerSecond;
@property(nonatomic, getter=isPaused) BOOL paused;
@property(nonatomic, readonly) NSInteger framesDisplayed;
@property(nonatomic, readonly) NSTimeInterval timeSinceFirstResume;
@property(nonatomic, readonly) NSTimeInterval timeSinceLastResume;
@property(nonatomic, readonly) NSTimeInterval timeSinceLastUpdate;
@property(nonatomic, readonly) NSTimeInterval timeSinceLastDraw;
@property(nonatomic) BOOL pauseOnWillResignActive;
@property(nonatomic) BOOL resumeOnDidBecomeActive;
@end

@implementation PearGLKViewController {
    CADisplayLink *_link;
    CFTimeInterval _firstResume, _lastResume, _lastUpdate, _lastDraw;
    NSMutableArray *_observers;
}

- (void)pearSetup {
    _preferredFramesPerSecond = 30;
    _pauseOnWillResignActive = YES;
    _resumeOnDidBecomeActive = YES;
    _observers = [NSMutableArray array];
    __weak PearGLKViewController *weakSelf = self;
    [_observers addObject:[NSNotificationCenter.defaultCenter addObserverForName:UIApplicationWillResignActiveNotification
        object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *) {
            if (weakSelf.pauseOnWillResignActive) weakSelf.paused = YES;
        }]];
    [_observers addObject:[NSNotificationCenter.defaultCenter addObserverForName:UIApplicationDidBecomeActiveNotification
        object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *) {
            if (weakSelf.resumeOnDidBecomeActive) weakSelf.paused = NO;
        }]];
}

- (instancetype)initWithNibName:(NSString *)nib bundle:(NSBundle *)bundle {
    if ((self = [super initWithNibName:nib bundle:bundle])) [self pearSetup];
    return self;
}
- (instancetype)initWithCoder:(NSCoder *)coder { if ((self = [super initWithCoder:coder])) [self pearSetup]; return self; }

- (void)loadView {
    if (self.nibName) { [super loadView]; return; }
    self.view = [[PearGLKView alloc] initWithFrame:UIScreen.mainScreen.bounds];
}

- (void)viewDidLoad {
    [super viewDidLoad];
    PearGLKView *view = (PearGLKView *)self.view;
    if ([view isKindOfClass:PearGLKView.class]) {
        // Display is driven by the controller, not by setNeedsDisplay.
        view.enableSetNeedsDisplay = NO;
        if (!view.delegate && [self respondsToSelector:@selector(glkView:drawInRect:)]) view.delegate = self;
    }
}

- (void)viewDidAppear:(BOOL)animated { [super viewDidAppear:animated]; if (!_paused) [self pearStart]; }
- (void)viewDidDisappear:(BOOL)animated { [super viewDidDisappear:animated]; [self pearStop]; }

- (NSInteger)framesPerSecond {
    NSInteger maximum = UIScreen.mainScreen.maximumFramesPerSecond ?: 60;
    return MAX(1, MIN(_preferredFramesPerSecond > 0 ? _preferredFramesPerSecond : maximum, maximum));
}

- (void)setPreferredFramesPerSecond:(NSInteger)rate {
    _preferredFramesPerSecond = rate;
    _link.preferredFramesPerSecond = self.framesPerSecond;
}

- (void)setPaused:(BOOL)paused {
    if (_paused == paused) return;
    _paused = paused;
    if (paused) { [self pearStop]; return; }
    if (self.isViewLoaded && self.view.window) [self pearStart];
}

- (void)pearStart {
    if (_link) return;
    CFTimeInterval now = CACurrentMediaTime();
    if (_firstResume == 0) _firstResume = now;
    _lastResume = now;
    _link = [CADisplayLink displayLinkWithTarget:self selector:@selector(pearTick:)];
    _link.preferredFramesPerSecond = self.framesPerSecond;
    [_link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
}

- (void)pearStop { [_link invalidate]; _link = nil; }

- (void)pearTick:(CADisplayLink *)link {
    CFTimeInterval now = CACurrentMediaTime();
    _timeSinceLastUpdate = _lastUpdate > 0 ? now - _lastUpdate : 0;
    _lastUpdate = now;
    _timeSinceFirstResume = now - _firstResume;
    _timeSinceLastResume = now - _lastResume;
    id delegate = _delegate;
    if ([delegate respondsToSelector:@selector(glkViewControllerUpdate:)]) {
        [(id<PearGLKViewControllerDelegate>)delegate glkViewControllerUpdate:self];
    } else if ([self respondsToSelector:@selector(update)]) {
        ((void (*)(id, SEL))objc_msgSend)(self, @selector(update));
    }
    PearGLKView *view = (PearGLKView *)self.view;
    if ([view isKindOfClass:PearGLKView.class]) [view display];
    _timeSinceLastDraw = _lastDraw > 0 ? now - _lastDraw : 0;
    _lastDraw = now;
    _framesDisplayed++;
}

- (void)dealloc {
    [self pearStop];
    for (id observer in _observers) [NSNotificationCenter.defaultCenter removeObserver:observer];
}
@end
