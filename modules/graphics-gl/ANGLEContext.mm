// Original EGL ownership adapter. No system EAGL objects enter ANGLE.
#import "PearANGLEContext.h"
#include <EGL/eglext.h>
#include <GLES3/gl3.h>
#include <cstring>
#include <pthread.h>
#include <mutex>

static std::recursive_mutex gContextOperations;
static pthread_key_t gCurrentContextKey;
static BOOL gContextTLSReady;

struct PearContextAPI {
    PFNEGLCHOOSECONFIGPROC choose;
    PFNEGLCREATECONTEXTPROC create;
    PFNEGLDESTROYCONTEXTPROC destroy;
    PFNEGLCREATEPBUFFERSURFACEPROC surface;
    PFNEGLDESTROYSURFACEPROC destroySurface;
    PFNEGLMAKECURRENTPROC current;
    PFNEGLGETERRORPROC error;
    PFNEGLSWAPBUFFERSPROC swap;
    PFNEGLRELEASETHREADPROC releaseThread;
};
static PearContextAPI gEGL;

static BOOL PearContextError(NSError *__autoreleasing *error, NSInteger code, NSString *reason) {
    if (error) *error = [NSError errorWithDomain:@"PearRuntime.ANGLE.Context" code:code
        userInfo:@{NSLocalizedDescriptionKey: reason}];
    return NO;
}

static BOOL PearLoadContextAPI(NSError *__autoreleasing *error) {
    if (PearANGLEDisplay() == EGL_NO_DISPLAY)
        return PearContextError(error, 1, @"ANGLE backend must be initialized first");
#define LOAD(field, name, type) gEGL.field = (type)PearANGLEGetProcAddress(name); if (!gEGL.field) return PearContextError(error, 2, @"Required EGL context API is unavailable")
    LOAD(choose, "eglChooseConfig", PFNEGLCHOOSECONFIGPROC);
    LOAD(create, "eglCreateContext", PFNEGLCREATECONTEXTPROC);
    LOAD(destroy, "eglDestroyContext", PFNEGLDESTROYCONTEXTPROC);
    LOAD(surface, "eglCreatePbufferSurface", PFNEGLCREATEPBUFFERSURFACEPROC);
    LOAD(destroySurface, "eglDestroySurface", PFNEGLDESTROYSURFACEPROC);
    LOAD(current, "eglMakeCurrent", PFNEGLMAKECURRENTPROC);
    LOAD(error, "eglGetError", PFNEGLGETERRORPROC);
    LOAD(swap, "eglSwapBuffers", PFNEGLSWAPBUFFERSPROC);
    LOAD(releaseThread, "eglReleaseThread", PFNEGLRELEASETHREADPROC);
#undef LOAD
    return YES;
}

@interface PearANGLESharegroup ()
@property(nonatomic) NSInteger version;
@property(nonatomic, strong) NSMutableArray<NSValue *> *members;
@end
@implementation PearANGLESharegroup
- (instancetype)init {
    if ((self = [super init])) _members = [NSMutableArray array];
    return self;
}
@end

@interface PearANGLEContext () {
    EGLContext _context;
    EGLSurface _surface;
    EGLConfig _config;
    pthread_t _ownerThread;
    BOOL _hasOwner;
    NSUInteger _width, _height;
}
+ (void)threadExitedWithRetainedContext:(void *)value;
@end

static void PearContextThreadExited(void *value) {
    @autoreleasepool { [PearANGLEContext threadExitedWithRetainedContext:value]; }
}

@implementation PearANGLEContext
+ (void)initialize {
    if (self != PearANGLEContext.class) return;
    gContextTLSReady = pthread_key_create(&gCurrentContextKey, PearContextThreadExited) == 0;
}

- (instancetype)initWithClientVersion:(NSInteger)version sharegroup:(PearANGLESharegroup *)group
                                error:(NSError *__autoreleasing *)error {
    if (!(self = [super init])) return nil;
    std::lock_guard<std::recursive_mutex> guard(gContextOperations);
    if (!gContextTLSReady) { PearContextError(error, 20, @"Context thread-local storage is unavailable"); return nil; }
    if (!PearLoadContextAPI(error)) return nil;
    if (version < 1 || version > 3) {
        PearContextError(error, 3, @"Supported GLES client versions are 1, 2 and 3"); return nil;
    }
    _sharegroup = group ?: [PearANGLESharegroup new];
    if (_sharegroup.version && _sharegroup.version != version) {
        PearContextError(error, 4, @"A sharegroup cannot mix GLES client versions"); return nil;
    }
    EGLint renderable = version == 1 ? EGL_OPENGL_ES_BIT : version == 2 ? EGL_OPENGL_ES2_BIT : EGL_OPENGL_ES3_BIT_KHR;
    EGLint config[] = {EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, renderable,
        EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8, EGL_NONE};
    EGLint count = 0;
    if (!gEGL.choose(PearANGLEDisplay(), config, &_config, 1, &count) || count != 1) {
        PearContextError(error, 5, @"ANGLE has no compatible GLES configuration"); return nil;
    }
    EGLContext shared = _sharegroup.members.count ? (EGLContext)_sharegroup.members.firstObject.pointerValue : EGL_NO_CONTEXT;
    EGLint attributes[] = {EGL_CONTEXT_CLIENT_VERSION, (EGLint)version, EGL_NONE};
    _context = gEGL.create(PearANGLEDisplay(), _config, shared, attributes);
    if (_context == EGL_NO_CONTEXT) {
        PearContextError(error, 6, [NSString stringWithFormat:@"EGL context creation failed (0x%x)", gEGL.error()]); return nil;
    }
    _clientVersion = version;
    if (![self resizePixelWidth:1 height:1 error:error]) return nil;
    _sharegroup.version = version;
    [_sharegroup.members addObject:[NSValue valueWithPointer:_context]];
    return self;
}

+ (PearANGLEContext *)currentContext {
    return gContextTLSReady ? (__bridge PearANGLEContext *)pthread_getspecific(gCurrentContextKey) : nil;
}

+ (void)threadExitedWithRetainedContext:(void *)value {
    PearANGLEContext *context = (__bridge_transfer PearANGLEContext *)value;
    std::lock_guard<std::recursive_mutex> guard(gContextOperations);
    gEGL.current(PearANGLEDisplay(), EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    gEGL.releaseThread();
    context->_hasOwner = NO;
}

+ (BOOL)setCurrentContext:(PearANGLEContext *)context error:(NSError *__autoreleasing *)error {
    std::lock_guard<std::recursive_mutex> guard(gContextOperations);
    PearANGLEContext *previous = [self currentContext];
    if (context == previous) return YES;
    if (!PearLoadContextAPI(error)) return NO;
    pthread_t thread = pthread_self();
    if (context && context->_hasOwner && !pthread_equal(context->_ownerThread, thread))
        return PearContextError(error, 7, @"Context is current on another thread");
    EGLSurface surface = context ? context->_surface : EGL_NO_SURFACE;
    EGLContext handle = context ? context->_context : EGL_NO_CONTEXT;
    if (!gEGL.current(PearANGLEDisplay(), surface, surface, handle))
        return PearContextError(error, 8, [NSString stringWithFormat:@"EGL make-current failed (0x%x)", gEGL.error()]);
    void *replacement = context ? (__bridge_retained void *)context : nullptr;
    void *old = pthread_getspecific(gCurrentContextKey);
    if (pthread_setspecific(gCurrentContextKey, replacement) != 0) {
        id discarded = (__bridge_transfer id)replacement;
        (void)discarded;
        gEGL.current(PearANGLEDisplay(), previous ? previous->_surface : EGL_NO_SURFACE,
                     previous ? previous->_surface : EGL_NO_SURFACE,
                     previous ? previous->_context : EGL_NO_CONTEXT);
        return PearContextError(error, 21, @"Context thread-local binding failed");
    }
    if (previous) previous->_hasOwner = NO;
    if (context) { context->_ownerThread = thread; context->_hasOwner = YES; }
    id discarded = (__bridge_transfer id)old;
    (void)discarded;
    return YES;
}

- (BOOL)resizePixelWidth:(NSUInteger)width height:(NSUInteger)height error:(NSError *__autoreleasing *)error {
    std::lock_guard<std::recursive_mutex> guard(gContextOperations);
    if (!width || !height || width > 8192 || height > 8192)
        return PearContextError(error, 9, @"Drawable dimensions must be 1…8192 pixels");
    if (_hasOwner && !pthread_equal(_ownerThread, pthread_self()))
        return PearContextError(error, 7, @"Cannot resize a context owned by another thread");
    if (width == _width && height == _height) return YES;
    EGLint attributes[] = {EGL_WIDTH, (EGLint)width, EGL_HEIGHT, (EGLint)height, EGL_NONE};
    EGLSurface replacement = gEGL.surface(PearANGLEDisplay(), _config, attributes);
    if (replacement == EGL_NO_SURFACE)
        return PearContextError(error, 10, @"EGL drawable allocation failed");
    if (_hasOwner && !gEGL.current(PearANGLEDisplay(), replacement, replacement, _context)) {
        gEGL.destroySurface(PearANGLEDisplay(), replacement);
        return PearContextError(error, 11, @"EGL drawable switch failed; previous drawable retained");
    }
    EGLSurface previous = _surface;
    _surface = replacement;
    _width = width; _height = height;
    if (previous != EGL_NO_SURFACE) gEGL.destroySurface(PearANGLEDisplay(), previous);
    return YES;
}

- (NSUInteger)pixelWidth { std::lock_guard<std::recursive_mutex> guard(gContextOperations); return _width; }
- (NSUInteger)pixelHeight { std::lock_guard<std::recursive_mutex> guard(gContextOperations); return _height; }

- (BOOL)presentFramebuffer:(unsigned int)framebuffer width:(NSUInteger)width height:(NSUInteger)height
                     error:(NSError *__autoreleasing *)error {
    std::lock_guard<std::recursive_mutex> guard(gContextOperations);
    if ([PearANGLEContext currentContext] != self)
        return PearContextError(error, 12, @"Presentation requires this context on the calling thread");
    if (_clientVersion < 2 || !width || !height || width > 8192 || height > 8192)
        return PearContextError(error, 13, @"Framebuffer presentation requires GLES 2/3 and valid pixel dimensions");
    if (_clientVersion == 2 && (width != _width || height != _height))
        return PearContextError(error, 19, @"GLES 2 framebuffer blit requires identical source/drawable pixel dimensions");
    // These entry points are resolved only after the immutable backend has
    // initialized. No per-frame symbol lookup and no system-GL fallback.
    static auto get = (void (*)(GLenum, GLint *))PearANGLEGetProcAddress("glGetIntegerv");
    static auto bind = (void (*)(GLenum, GLuint))PearANGLEGetProcAddress("glBindFramebuffer");
    static auto status = (GLenum (*)(GLenum))PearANGLEGetProcAddress("glCheckFramebufferStatus");
    using Blit = void (*)(GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLbitfield, GLenum);
    static auto blitANGLE = (Blit)PearANGLEGetProcAddress("glBlitFramebufferANGLE");
    static auto blitCore = (Blit)PearANGLEGetProcAddress("glBlitFramebuffer");
    Blit blit = _clientVersion >= 3 ? blitCore : blitANGLE;
    static auto enabled = (GLboolean (*)(GLenum))PearANGLEGetProcAddress("glIsEnabled");
    static auto enable = (void (*)(GLenum))PearANGLEGetProcAddress("glEnable");
    static auto disable = (void (*)(GLenum))PearANGLEGetProcAddress("glDisable");
    static auto string = (const GLubyte *(*)(GLenum))PearANGLEGetProcAddress("glGetString");
    static auto readBuffer = (void (*)(GLenum))PearANGLEGetProcAddress("glReadBuffer");
    if (!get || !bind || !status || !blit || !enabled || !enable || !disable || !string || (_clientVersion >= 3 && !readBuffer))
        return PearContextError(error, 14, @"Required ANGLE framebuffer operations are unavailable");
    if (_clientVersion == 2) {
        const char *extensions = (const char *)string(GL_EXTENSIONS);
        if (!extensions || !strstr(extensions, "GL_ANGLE_framebuffer_blit"))
            return PearContextError(error, 15, @"GLES 2 presentation requires ANGLE_framebuffer_blit");
    }
    if (framebuffer) {
        GLint oldRead = 0, oldDraw = 0, sourceReadBuffer = GL_COLOR_ATTACHMENT0;
        get(GL_READ_FRAMEBUFFER_BINDING, &oldRead);
        get(GL_DRAW_FRAMEBUFFER_BINDING, &oldDraw);
        BOOL scissor = enabled(GL_SCISSOR_TEST);
        BOOL changedReadBuffer = NO;
        @try {
            bind(GL_READ_FRAMEBUFFER, framebuffer);
            if (status(GL_READ_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE)
                return PearContextError(error, 16, @"Source framebuffer is incomplete");
            if (_clientVersion >= 3) {
                get(GL_READ_BUFFER, &sourceReadBuffer);
                readBuffer(GL_COLOR_ATTACHMENT0);
                changedReadBuffer = YES;
            }
            bind(GL_DRAW_FRAMEBUFFER, 0);
            if (status(GL_DRAW_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE)
                return PearContextError(error, 17, @"ANGLE drawable framebuffer is incomplete");
            if (scissor) disable(GL_SCISSOR_TEST);
            blit(0, 0, (GLint)width, (GLint)height, 0, 0, (GLint)_width, (GLint)_height, GL_COLOR_BUFFER_BIT, GL_NEAREST);
        } @finally {
            if (changedReadBuffer) readBuffer((GLenum)sourceReadBuffer);
            bind(GL_READ_FRAMEBUFFER, (GLuint)oldRead);
            bind(GL_DRAW_FRAMEBUFFER, (GLuint)oldDraw);
            if (scissor) enable(GL_SCISSOR_TEST);
        }
    }
    if (!gEGL.swap(PearANGLEDisplay(), _surface))
        return PearContextError(error, 18, @"EGL drawable presentation failed");
    return YES;
}

- (void)dealloc {
    std::lock_guard<std::recursive_mutex> guard(gContextOperations);
    if (_context != EGL_NO_CONTEXT) {
        [_sharegroup.members removeObject:[NSValue valueWithPointer:_context]];
        gEGL.destroy(PearANGLEDisplay(), _context);
    }
    if (_surface != EGL_NO_SURFACE) gEGL.destroySurface(PearANGLEDisplay(), _surface);
}
@end
