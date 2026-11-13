// Original renderbuffer/native-surface adapter. No system GL entry points.
#import "PearANGLEDrawable.h"
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <GLES3/gl3.h>
#include <cmath>

@interface PearANGLEStorage : NSObject
@property(nonatomic, strong) CALayer *layer;
@property(nonatomic) PearANGLEDrawableGeometry geometry;
@property(nonatomic) BOOL retained;
@end
@implementation PearANGLEStorage
@end

@interface PearANGLEDrawableState : NSObject
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, PearANGLEStorage *> *storage;
@property(nonatomic) GLuint framebuffer;
@property(nonatomic) uint64_t generation;
@end
@implementation PearANGLEDrawableState
- (instancetype)init {
    if ((self = [super init])) _storage = [NSMutableDictionary dictionary];
    return self;
}
@end

static char gDrawableStateKey;
// The current-context ownership rule serializes access, including migration.
// The private FBO lives with the context and is freed by eglDestroyContext.
static PearANGLEDrawableState *PearDrawableState(PearANGLEContext *context) {
    PearANGLEDrawableState *state = objc_getAssociatedObject(context, &gDrawableStateKey);
    if (!state) {
        state = [PearANGLEDrawableState new];
        objc_setAssociatedObject(context, &gDrawableStateKey, state, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    }
    return state;
}

static PearANGLEStorage *PearStoredDrawable(PearANGLEDrawableState *state, GLuint name) {
    if (!state) return nil;
    @synchronized(state) { return state.storage[@(name)]; }
}

static void PearForgetDrawable(PearANGLEDrawableState *state, GLuint name) {
    @synchronized(state) { [state.storage removeObjectForKey:@(name)]; }
}

struct PearDrawableGL {
    PFNGLGETINTEGERVPROC get;
    PFNGLGETRENDERBUFFERPARAMETERIVPROC parameter;
    PFNGLRENDERBUFFERSTORAGEPROC storage;
    PFNGLISRENDERBUFFERPROC isRenderbuffer;
    PFNGLGENFRAMEBUFFERSPROC genFramebuffer;
    PFNGLBINDFRAMEBUFFERPROC bindFramebuffer;
    PFNGLFRAMEBUFFERRENDERBUFFERPROC attach;
};

static const PearDrawableGL &PearDrawableAPI() {
    // Used only after initialization and current-context validation.
    static const PearDrawableGL api = {
        (PFNGLGETINTEGERVPROC)PearANGLEGetProcAddress("glGetIntegerv"),
        (PFNGLGETRENDERBUFFERPARAMETERIVPROC)PearANGLEGetProcAddress("glGetRenderbufferParameteriv"),
        (PFNGLRENDERBUFFERSTORAGEPROC)PearANGLEGetProcAddress("glRenderbufferStorage"),
        (PFNGLISRENDERBUFFERPROC)PearANGLEGetProcAddress("glIsRenderbuffer"),
        (PFNGLGENFRAMEBUFFERSPROC)PearANGLEGetProcAddress("glGenFramebuffers"),
        (PFNGLBINDFRAMEBUFFERPROC)PearANGLEGetProcAddress("glBindFramebuffer"),
        (PFNGLFRAMEBUFFERRENDERBUFFERPROC)PearANGLEGetProcAddress("glFramebufferRenderbuffer"),
    };
    return api;
}

static BOOL PearDrawableError(NSError *__autoreleasing *error, NSInteger code, NSString *message) {
    if (error) *error = [NSError errorWithDomain:@"PearRuntime.ANGLE.Drawable" code:code
        userInfo:@{NSLocalizedDescriptionKey: message}];
    return NO;
}

static BOOL PearDrawableReady(PearANGLEContext *context, NSUInteger target, NSError *__autoreleasing *error) {
    if (PearANGLEContext.currentContext != context)
        return PearDrawableError(error, 1, @"Drawable operations require this context current on the calling thread");
    if (context.clientVersion < 2 || target != GL_RENDERBUFFER)
        return PearDrawableError(error, 2, @"Drawable operations require GLES2/3 and GL_RENDERBUFFER");
    const auto &gl = PearDrawableAPI();
    if (!gl.get || !gl.parameter || !gl.storage || !gl.isRenderbuffer ||
        !gl.genFramebuffer || !gl.bindFramebuffer || !gl.attach)
        return PearDrawableError(error, 3, @"Required ANGLE renderbuffer entry points are missing");
    return YES;
}

static BOOL PearGeometryMatches(PearANGLEStorage *storage) {
    return storage && CGRectEqualToRect(storage.layer.bounds, storage.geometry.logicalBounds) &&
        storage.layer.contentsScale == storage.geometry.backingScale;
}

static BOOL PearStorageMatches(GLuint name, PearANGLEStorage *storage) {
    const auto &gl = PearDrawableAPI();
    if (!storage || !gl.isRenderbuffer(name)) return NO;
    // Caller has already bound this renderbuffer. Do not drain glGetError:
    // engine errors belong to the engine.
    GLint width = 0, height = 0, format = 0, samples = 0;
    gl.parameter(GL_RENDERBUFFER, GL_RENDERBUFFER_WIDTH, &width);
    gl.parameter(GL_RENDERBUFFER, GL_RENDERBUFFER_HEIGHT, &height);
    gl.parameter(GL_RENDERBUFFER, GL_RENDERBUFFER_INTERNAL_FORMAT, &format);
    gl.parameter(GL_RENDERBUFFER, GL_RENDERBUFFER_SAMPLES, &samples);
    return width == storage.geometry.pixelSize.width && height == storage.geometry.pixelSize.height &&
        format == GL_RGBA8 && samples == 0;
}

@implementation PearANGLEContext (Drawable)
- (BOOL)allocateBoundRenderbuffer:(NSUInteger)target fromLayer:(CALayer *)layer
                     colorFormat:(PearANGLEColorFormat)format retainedBacking:(BOOL)retained
                           error:(NSError *__autoreleasing *)error {
    if (!PearDrawableReady(self, target, error)) return NO;
    if (format != PearANGLEColorRGBA8)
        return PearDrawableError(error, 4, @"Only RGBA8 drawable storage is currently implemented");
    const auto &gl = PearDrawableAPI();
    GLint name = 0;
    gl.get(GL_RENDERBUFFER_BINDING, &name);
    if (!name || !gl.isRenderbuffer((GLuint)name))
        return PearDrawableError(error, 5, @"Bind an ANGLE renderbuffer before allocating drawable storage");
    PearANGLEDrawableState *state = PearDrawableState(self);
    if (!layer) {
        PearForgetDrawable(state, (GLuint)name);
        gl.storage(GL_RENDERBUFFER, GL_RGBA8, 0, 0);
        return YES;
    }
    if (![layer isKindOfClass:CALayer.class])
        return PearDrawableError(error, 6, @"Drawable storage requires a CALayer host");
    CGRect bounds = layer.bounds;
    CGFloat scale = layer.contentsScale;
    double width = bounds.size.width * scale, height = bounds.size.height * scale;
    if (!std::isfinite(scale) || scale <= 0 || bounds.size.width <= 0 || bounds.size.height <= 0 ||
        !std::isfinite(width) || !std::isfinite(height) || width < 1 || height < 1 || width > 8192 || height > 8192)
        return PearDrawableError(error, 7, @"Invalid drawable geometry");
    NSError *bindingError = nil;
    if (![self bindDrawableLayer:layer error:&bindingError]) {
        if (![bindingError.domain isEqualToString:@"PearRuntime.ANGLE.Context"] || bindingError.code != 24) {
            if (error) *error = bindingError;
            return NO;
        }
        // The engine has explicitly asked to reallocate: rebuild the native
        // surface before publishing the new renderbuffer/input generation.
        if (![self detachDrawableLayerWithError:error] || ![self bindDrawableLayer:layer error:error]) return NO;
    }
    PearForgetDrawable(state, (GLuint)name);
    gl.storage(GL_RENDERBUFFER, GL_RGBA8, (GLsizei)self.pixelWidth, (GLsizei)self.pixelHeight);
    PearANGLEStorage *storage = [PearANGLEStorage new];
    storage.layer = layer;
    storage.retained = retained;
    storage.geometry = {bounds, CGSizeMake(self.pixelWidth, self.pixelHeight), scale, ++state.generation};
    if (!PearStorageMatches((GLuint)name, storage))
        return PearDrawableError(error, 8, @"ANGLE renderbuffer allocation did not match the native drawable");
    if (!PearGeometryMatches(storage))
        return PearDrawableError(error, 9, @"Layer geometry changed during allocation; no input generation was published");
    @synchronized(state) { state.storage[@(name)] = storage; }
    return YES;
}

- (BOOL)presentBoundRenderbuffer:(NSUInteger)target error:(NSError *__autoreleasing *)error {
    if (!PearDrawableReady(self, target, error)) return NO;
    const auto &gl = PearDrawableAPI();
    GLint name = 0;
    gl.get(GL_RENDERBUFFER_BINDING, &name);
    PearANGLEDrawableState *state = PearDrawableState(self);
    PearANGLEStorage *storage = PearStoredDrawable(state, (GLuint)name);
    if (!name || !PearGeometryMatches(storage) || !PearStorageMatches((GLuint)name, storage)) {
        PearForgetDrawable(state, (GLuint)name);
        return PearDrawableError(error, 10, @"Drawable storage is missing or stale; reallocate before presentation");
    }
    if (![self bindDrawableLayer:storage.layer error:error]) return NO;
    if (!state.framebuffer) {
        GLuint framebuffer = 0;
        gl.genFramebuffer(1, &framebuffer);
        state.framebuffer = framebuffer;
    }
    if (!state.framebuffer) return PearDrawableError(error, 11, @"ANGLE presentation framebuffer allocation failed");
    GLint read = 0, draw = 0;
    gl.get(GL_READ_FRAMEBUFFER_BINDING, &read);
    gl.get(GL_DRAW_FRAMEBUFFER_BINDING, &draw);
    @try {
        gl.bindFramebuffer(GL_DRAW_FRAMEBUFFER, state.framebuffer);
        gl.attach(GL_DRAW_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_RENDERBUFFER, (GLuint)name);
        return [self presentFramebuffer:state.framebuffer width:(NSUInteger)storage.geometry.pixelSize.width
                                height:(NSUInteger)storage.geometry.pixelSize.height error:error];
    } @finally {
        // Do not keep deleted/reused game renderbuffer names alive in our FBO.
        gl.bindFramebuffer(GL_DRAW_FRAMEBUFFER, state.framebuffer);
        gl.attach(GL_DRAW_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_RENDERBUFFER, 0);
        gl.bindFramebuffer(GL_READ_FRAMEBUFFER, (GLuint)read);
        gl.bindFramebuffer(GL_DRAW_FRAMEBUFFER, (GLuint)draw);
    }
}

- (BOOL)copyDrawableGeometry:(PearANGLEDrawableGeometry *)geometry forRenderbuffer:(unsigned int)renderbuffer {
    if (!geometry) return NO;
    PearANGLEDrawableState *state = objc_getAssociatedObject(self, &gDrawableStateKey);
    if (!state) return NO;
    // This does not bind GL objects or mutate GL state. Storage is revalidated
    // at presentation; allocation is the authority for published geometry.
    @synchronized(state) {
        PearANGLEStorage *storage = state.storage[@(renderbuffer)];
        if (!PearGeometryMatches(storage)) return NO;
        *geometry = storage.geometry;
        return YES;
    }
}

- (BOOL)mapLayerPoint:(CGPoint)point forRenderbuffer:(unsigned int)renderbuffer
          generation:(uint64_t)generation toPixel:(CGPoint *)pixel {
    PearANGLEDrawableGeometry geometry;
    if (!pixel || ![self copyDrawableGeometry:&geometry forRenderbuffer:renderbuffer] ||
        generation != geometry.generation || !std::isfinite(point.x) || !std::isfinite(point.y) ||
        !CGRectContainsPoint(geometry.logicalBounds, point)) return NO;
    *pixel = CGPointMake((point.x - geometry.logicalBounds.origin.x) * geometry.pixelSize.width / geometry.logicalBounds.size.width,
                         (point.y - geometry.logicalBounds.origin.y) * geometry.pixelSize.height / geometry.logicalBounds.size.height);
    return YES;
}
@end
