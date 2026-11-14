// PearHost MetalKit.framework: MTKView as the UIView subclass iOS apps expect
// (macOS MetalKit's MTKView is an NSView). Clean-room: behavior from the public
// MTKView.h interface and Apple's MetalKit documentation: a CAMetalLayer-backed
// view that sizes its drawable from its bounds, owns the depth/stencil and
// multisample textures, builds a render pass descriptor per frame and drives
// drawing from a display link (or on demand).
#import <Metal/Metal.h>
#import "PearUIKit.h"

#define PEAR_MTK_EXPORT __attribute__((visibility("default")))

@class MTKView;
@protocol MTKViewDelegate <NSObject>
- (void)mtkView:(MTKView *)view drawableSizeWillChange:(CGSize)size;
- (void)drawInMTKView:(MTKView *)view;
@end

PEAR_MTK_EXPORT
@interface MTKView : UIView
@end

@implementation MTKView {
    id<MTLDevice> _device;
    __weak id<MTKViewDelegate> _delegate;
    id<CAMetalDrawable> _currentDrawable;
    MTLRenderPassDescriptor *_passDescriptor;
    id<MTLTexture> _depthTexture, _msaaTexture;
    MTLPixelFormat _depthFormat;
    NSUInteger _sampleCount;
    MTLClearColor _clearColor;
    double _clearDepth;
    uint32_t _clearStencil;
    NSInteger _fps;
    BOOL _paused, _enableSetNeedsDisplay, _autoResize, _needsDisplay;
    CADisplayLink *_link;
    CGSize _lastDrawableSize;
}

+ (Class)layerClass { return [CAMetalLayer class]; }

- (CAMetalLayer *)pear_metalLayer { return (CAMetalLayer *)self.layer; }

- (void)pear_setup:(id<MTLDevice>)device {
    _sampleCount = 1;
    _clearColor = MTLClearColorMake(0, 0, 0, 1);
    _clearDepth = 1.0;
    _fps = 60;
    _autoResize = YES;
    CAMetalLayer *l = [self pear_metalLayer];
    l.pixelFormat = MTLPixelFormatBGRA8Unorm;
    l.framebufferOnly = YES;
    self.device = device;
    self.contentScaleFactor = UIScreen.mainScreen.scale;
}

- (instancetype)initWithFrame:(CGRect)frame { return [self initWithFrame:frame device:nil]; }
- (instancetype)initWithFrame:(CGRect)frame device:(id<MTLDevice>)device {
    if ((self = [super initWithFrame:frame])) [self pear_setup:device];
    return self;
}
- (instancetype)initWithCoder:(NSCoder *)coder {
    if ((self = [super initWithCoder:coder])) [self pear_setup:nil];
    return self;
}
- (void)dealloc { [_link invalidate]; }

- (id<MTLDevice>)device { return _device; }
- (void)setDevice:(id<MTLDevice>)device {
    _device = device;
    [self pear_metalLayer].device = device;
    _depthTexture = _msaaTexture = nil;
}
- (id<MTLDevice>)preferredDevice { return MTLCreateSystemDefaultDevice(); }
- (id<MTKViewDelegate>)delegate { return _delegate; }
- (void)setDelegate:(id<MTKViewDelegate>)d {
    _delegate = d;
    [self pear_updateLoop];
}

- (MTLPixelFormat)colorPixelFormat { return [self pear_metalLayer].pixelFormat; }
- (void)setColorPixelFormat:(MTLPixelFormat)f {
    [self pear_metalLayer].pixelFormat = f;
    _msaaTexture = nil;
}
- (MTLPixelFormat)depthStencilPixelFormat { return _depthFormat; }
- (void)setDepthStencilPixelFormat:(MTLPixelFormat)f {
    _depthFormat = f;
    _depthTexture = nil;
}
- (NSUInteger)sampleCount { return _sampleCount; }
- (void)setSampleCount:(NSUInteger)n {
    _sampleCount = MAX(1u, n);
    _depthTexture = _msaaTexture = nil;
}
- (MTLClearColor)clearColor { return _clearColor; }
- (void)setClearColor:(MTLClearColor)c { _clearColor = c; }
- (double)clearDepth { return _clearDepth; }
- (void)setClearDepth:(double)d { _clearDepth = d; }
- (uint32_t)clearStencil { return _clearStencil; }
- (void)setClearStencil:(uint32_t)s { _clearStencil = s; }
- (BOOL)framebufferOnly { return [self pear_metalLayer].framebufferOnly; }
- (void)setFramebufferOnly:(BOOL)f { [self pear_metalLayer].framebufferOnly = f; }
- (BOOL)presentsWithTransaction { return [self pear_metalLayer].presentsWithTransaction; }
- (void)setPresentsWithTransaction:(BOOL)p { [self pear_metalLayer].presentsWithTransaction = p; }
- (CGColorSpaceRef)colorspace { return [self pear_metalLayer].colorspace; }
- (void)setColorspace:(CGColorSpaceRef)c { [self pear_metalLayer].colorspace = c; }
- (MTLTextureUsage)depthStencilAttachmentTextureUsage { return MTLTextureUsageRenderTarget; }
- (NSInteger)preferredFramesPerSecond { return _fps; }
- (void)setPreferredFramesPerSecond:(NSInteger)fps {
    _fps = fps;
    [self pear_updateLoop];
}
- (BOOL)isPaused { return _paused; }
- (void)setPaused:(BOOL)p {
    _paused = p;
    [self pear_updateLoop];
}
- (BOOL)enableSetNeedsDisplay { return _enableSetNeedsDisplay; }
- (void)setEnableSetNeedsDisplay:(BOOL)e {
    _enableSetNeedsDisplay = e;
    [self pear_updateLoop];
}
- (BOOL)autoResizeDrawable { return _autoResize; }
- (void)setAutoResizeDrawable:(BOOL)a { _autoResize = a; }
- (CGSize)drawableSize { return [self pear_metalLayer].drawableSize; }
- (void)setDrawableSize:(CGSize)size {
    if (CGSizeEqualToSize(size, [self pear_metalLayer].drawableSize)) return;
    [self pear_metalLayer].drawableSize = size;
    _depthTexture = _msaaTexture = nil;
    [_delegate mtkView:self drawableSizeWillChange:size];
}
- (CGSize)preferredDrawableSize {
    CGFloat s = self.contentScaleFactor;
    return CGSizeMake(self.bounds.size.width * s, self.bounds.size.height * s);
}

// Drawable follows the view's size, as documented for autoResizeDrawable.
- (void)layoutSubviews {
    [super layoutSubviews];
    [self pear_resizeDrawable];
}
- (void)setFrame:(CGRect)frame {
    [super setFrame:frame];
    [self pear_resizeDrawable];
}
- (void)setBounds:(CGRect)bounds {
    [super setBounds:bounds];
    [self pear_resizeDrawable];
}
- (void)setContentScaleFactor:(CGFloat)s {
    [super setContentScaleFactor:s];
    [self pear_resizeDrawable];
}
- (void)pear_resizeDrawable {
    if (!_autoResize) return;
    CGSize want = [self preferredDrawableSize];
    if (want.width < 1 || want.height < 1) return;
    self.drawableSize = want;
}

- (void)didMoveToWindow {
    [super didMoveToWindow];
    [self pear_resizeDrawable];
    [self pear_updateLoop];
}

// ---- frame loop -----------------------------------------------------------------

- (void)pear_updateLoop {
    BOOL want = self.window && !_paused && !_enableSetNeedsDisplay;
    if (want && !_link) {
        _link = [UIScreen.mainScreen displayLinkWithTarget:self selector:@selector(pear_tick:)];
        [_link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    } else if (!want && _link) {
        [_link invalidate];
        _link = nil;
    }
    if (_link && _fps > 0) _link.preferredFrameRateRange = CAFrameRateRangeMake(_fps, _fps, _fps);
}
- (void)pear_tick:(CADisplayLink *)link { [self draw]; }

- (void)setNeedsDisplay {
    [super setNeedsDisplay];
    if (_enableSetNeedsDisplay && !_needsDisplay) {
        _needsDisplay = YES;
        dispatch_async(dispatch_get_main_queue(), ^{
            self->_needsDisplay = NO;
            [self draw];
        });
    }
}

- (void)draw {
    @autoreleasepool {
        _currentDrawable = nil;
        _passDescriptor = nil;
        id<MTKViewDelegate> d = _delegate;
        if (d) [d drawInMTKView:self];
        else [self drawRect:self.bounds];
        _currentDrawable = nil;
        _passDescriptor = nil;
    }
}

- (void)releaseDrawables {
    _depthTexture = _msaaTexture = nil;
    _currentDrawable = nil;
}

- (id<CAMetalDrawable>)currentDrawable {
    if (!_currentDrawable) _currentDrawable = [[self pear_metalLayer] nextDrawable];
    return _currentDrawable;
}

- (id<MTLTexture>)pear_texture:(MTLPixelFormat)format samples:(NSUInteger)samples usage:(MTLTextureUsage)usage {
    CGSize size = self.drawableSize;
    if (!_device || size.width < 1 || size.height < 1) return nil;
    MTLTextureDescriptor *t = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:format
                                                                                 width:(NSUInteger)size.width
                                                                                height:(NSUInteger)size.height
                                                                             mipmapped:NO];
    t.textureType = samples > 1 ? MTLTextureType2DMultisample : MTLTextureType2D;
    t.sampleCount = samples;
    t.usage = usage;
    t.storageMode = MTLStorageModePrivate;
    return [_device newTextureWithDescriptor:t];
}

- (id<MTLTexture>)depthStencilTexture {
    if (_depthFormat == MTLPixelFormatInvalid) return nil;
    if (!_depthTexture) _depthTexture = [self pear_texture:_depthFormat samples:_sampleCount usage:MTLTextureUsageRenderTarget];
    return _depthTexture;
}
- (id<MTLTexture>)multisampleColorTexture {
    if (_sampleCount <= 1) return nil;
    if (!_msaaTexture) _msaaTexture = [self pear_texture:self.colorPixelFormat samples:_sampleCount usage:MTLTextureUsageRenderTarget];
    return _msaaTexture;
}

- (MTLRenderPassDescriptor *)currentRenderPassDescriptor {
    if (_passDescriptor) return _passDescriptor;
    id<CAMetalDrawable> drawable = self.currentDrawable;
    if (!drawable) return nil;
    MTLRenderPassDescriptor *p = [MTLRenderPassDescriptor renderPassDescriptor];
    MTLRenderPassColorAttachmentDescriptor *c = p.colorAttachments[0];
    if (_sampleCount > 1) {
        c.texture = self.multisampleColorTexture;
        c.resolveTexture = drawable.texture;
        c.storeAction = MTLStoreActionMultisampleResolve;
    } else {
        c.texture = drawable.texture;
        c.storeAction = MTLStoreActionStore;
    }
    c.loadAction = MTLLoadActionClear;
    c.clearColor = _clearColor;
    id<MTLTexture> depth = self.depthStencilTexture;
    if (depth) {
        p.depthAttachment.texture = depth;
        p.depthAttachment.loadAction = MTLLoadActionClear;
        p.depthAttachment.storeAction = MTLStoreActionDontCare;
        p.depthAttachment.clearDepth = _clearDepth;
        MTLPixelFormat f = _depthFormat;
        if (f == MTLPixelFormatDepth32Float_Stencil8 || f == MTLPixelFormatStencil8 ||
            f == MTLPixelFormatX32_Stencil8) {
            p.stencilAttachment.texture = depth;
            p.stencilAttachment.loadAction = MTLLoadActionClear;
            p.stencilAttachment.storeAction = MTLStoreActionDontCare;
            p.stencilAttachment.clearStencil = _clearStencil;
        }
    }
    _passDescriptor = p;
    return p;
}
@end

__attribute__((constructor)) static void PearMetalKitLoad(void) {
    PearMissingInstallForImage((const void *)&PearMetalKitLoad);
}
