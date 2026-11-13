// Original implementation of public EAGL selectors over ANGLE. No Apple
// implementation is included. This class is not registered as EAGLContext.
#import "PearEAGLContext.h"
#import <QuartzCore/QuartzCore.h>
#include <unistd.h>
#include <pthread.h>

@interface PearEAGLSharegroup ()
@property(nonatomic, strong) PearANGLESharegroup *angleGroup;
@end
@implementation PearEAGLSharegroup
- (instancetype)init {
    if ((self = [super init])) _angleGroup = [PearANGLESharegroup new];
    return self;
}
@end

@protocol PearDrawableProperties <NSObject>
- (NSDictionary *)drawableProperties;
@end

static pthread_key_t gEAGLAdapterKey;
static BOOL gEAGLAdapterKeyReady;
static void PearReleaseEAGLAdapter(void *value) {
    @autoreleasepool {
        // The core's independent TLS destructor handles native EGL unbinding.
        id released = (__bridge_transfer id)value;
        (void)released;
    }
}

@interface PearEAGLContext ()
@property(atomic, strong, readwrite) NSError *lastError;
@property(nonatomic) BOOL reportedPresentation;
/// Made current on a second thread while still current on its render thread
/// (allowed by EAGL). Presentation then goes through a presenter context.
@property(atomic) BOOL sharedAcrossThreads;
@end

@implementation PearEAGLContext
+ (void)initialize {
    if (self == PearEAGLContext.class)
        gEAGLAdapterKeyReady = pthread_key_create(&gEAGLAdapterKey, PearReleaseEAGLAdapter) == 0;
}
- (instancetype)initWithAPI:(NSUInteger)api { return [self initWithAPI:api sharegroup:nil]; }
- (instancetype)initWithAPI:(NSUInteger)api sharegroup:(PearEAGLSharegroup *)group {
    if (!(self = [super init])) return nil;
    if (!gEAGLAdapterKeyReady || (group && ![group isKindOfClass:PearEAGLSharegroup.class])) return nil;
    _sharegroup = group ?: [PearEAGLSharegroup new];
    NSError *error = nil;
    _angleContext = [[PearANGLEContext alloc] initWithClientVersion:(NSInteger)api sharegroup:_sharegroup.angleGroup serializedThreadSharing:YES error:&error];
    if (!_angleContext) return nil;
    _API = api;
    if (getenv("PEAR_GRAPHICS_BACKEND")) PearANGLETrace( "[PearRuntime][angle] context GLES%lu created\n", (unsigned long)api);
    return self;
}
+ (PearEAGLContext *)currentContext {
    return gEAGLAdapterKeyReady ? (__bridge PearEAGLContext *)pthread_getspecific(gEAGLAdapterKey) : nil;
}
+ (BOOL)setCurrentContext:(PearEAGLContext *)context {
    if (!gEAGLAdapterKeyReady || (context && ![context isKindOfClass:PearEAGLContext.class])) return NO;
    PearEAGLContext *previous = self.currentContext;
    NSError *error = nil;
    BOOL current = [PearANGLEContext setCurrentContext:context.angleContext error:&error];
    // Do not claim success with a nil native context: GL commands would be
    // silently discarded. Allocation/presentation have explicit shared-object
    // paths, but an actual make-current must honor ANGLE's thread ownership.
    if (!current) {
        static unsigned failures;
        if (failures++ < 8)
            PearANGLETrace("[PearRuntime][angle] EAGL setCurrentContext failed: %s\n",
                           error.localizedDescription.UTF8String ?: "unknown");
        context.lastError = error;
        return NO;
    }
    if (context.sharedAcrossThreads) [context.angleContext releaseNativeSurfaceForSharedThreadUse];
    void *replacement = context ? (__bridge_retained void *)context : nullptr;
    void *old = pthread_getspecific(gEAGLAdapterKey);
    if (pthread_setspecific(gEAGLAdapterKey, replacement) != 0) {
        id discarded = (__bridge_transfer id)replacement;
        (void)discarded;
        [PearANGLEContext setCurrentContext:previous.angleContext error:nullptr];
        return NO;
    }
    context.lastError = nil;
    id discarded = (__bridge_transfer id)old;
    (void)discarded;
    return YES;
}
- (BOOL)adapterError:(NSInteger)code message:(NSString *)message {
    self.lastError = [NSError errorWithDomain:@"PearRuntime.ANGLE.EAGL" code:code
        userInfo:@{NSLocalizedDescriptionKey: message}];
    return NO;
}
/// EAGL's drawable calls act on the receiver, current or not (GTA III sizes its
/// drawable from layout while no context is current). ANGLE needs the context
/// current, so make it current for the call and restore the thread's previous
/// one. A context current on another thread still fails, as before.
static BOOL PearWithContextCurrent(PearANGLEContext *context, NSError *__autoreleasing *error, BOOL (^body)(void)) {
    PearANGLEContext *previous = PearANGLEContext.currentContext;
    if (previous == context) return body();
    if (![PearANGLEContext setCurrentContext:context error:error]) return NO;
    BOOL result = body();
    [PearANGLEContext setCurrentContext:previous error:nullptr];
    return result;
}
- (BOOL)renderbufferStorage:(NSUInteger)target fromDrawable:(id)drawable {
    self.lastError = nil;
    if (drawable && ![drawable isKindOfClass:CALayer.class])
        return [self adapterError:1 message:@"EAGL drawable must be a CALayer host"];
    NSDictionary *properties = nil;
    if ([drawable respondsToSelector:@selector(drawableProperties)]) {
        properties = [(id<PearDrawableProperties>)drawable drawableProperties];
        if (properties && ![properties isKindOfClass:NSDictionary.class])
            return [self adapterError:2 message:@"Invalid EAGL drawable properties"];
    }
    // Public constant values verified through exported SDK runtime constants;
    // no dependency on the system GL implementation is linked or called.
    id format = properties[@"EAGLDrawablePropertyColorFormat"];
    // RGB565 (GTA III and other early titles) is stored as RGBA8: same colours
    // at higher precision. It has no alpha, so the layer composites opaque.
    BOOL rgb565 = [format isEqual:@"EAGLColorFormat565"];
    if (format && ![format isEqual:@"EAGLColorFormat8888"] && !rgb565) {
        PearANGLETrace("[PearRuntime][angle] unsupported EAGL colour format %s\n",
                       [[format description] UTF8String] ?: "?");
        return [self adapterError:3 message:@"Only EAGL RGBA8 and RGB565 drawables are implemented"];
    }
    id retained = properties[@"EAGLDrawablePropertyRetained"];
    if (retained && ![retained isKindOfClass:NSNumber.class])
        return [self adapterError:4 message:@"Retained backing must be a boolean"];
    if (rgb565 && drawable && ![(CALayer *)drawable isOpaque]) {
        if (NSThread.isMainThread) {
            ((CALayer *)drawable).opaque = YES;
        } else {
            CALayer *layer = drawable;
            dispatch_async(dispatch_get_main_queue(), ^{ layer.opaque = YES; });
        }
    }
    NSError *error = nil;
    PearANGLEContext *angle = _angleContext;
    __block NSError *inner = nil;
    BOOL result = PearWithContextCurrent(angle, &error, ^BOOL {
        NSError *allocationError = nil;
        BOOL allocated = [angle allocateBoundRenderbuffer:target fromLayer:drawable colorFormat:PearANGLEColorRGBA8
                                         retainedBacking:[retained boolValue] error:&allocationError];
        inner = allocationError;
        return allocated;
    });
    if (!error) error = inner;
    // Current on the app's render thread while layout (main) resizes the view.
    if (!result && [error.domain isEqualToString:@"PearRuntime.ANGLE.Context"] && error.code == 7 &&
        [drawable isKindOfClass:CALayer.class]) {
        NSError *acceptError = nil;
        result = [angle acceptDrawableLayer:drawable fromOtherThreadWithError:&acceptError];
        error = result ? nil : acceptError;
    }
    self.lastError = error;
    // Failures are always logged (bounded); every allocation with PEAR_DEBUG_GL.
    static unsigned storageFailures;
    static const bool verbose = getenv("PEAR_DEBUG_GL") || getenv("PEAR_GRAPHICS_BACKEND");
    if (verbose || (!result && storageFailures++ < 8))
        PearANGLETrace( "[PearRuntime][angle] storage %lux%lu result=%d error=%s\n",
            (unsigned long)_angleContext.pixelWidth, (unsigned long)_angleContext.pixelHeight, result,
            error.localizedDescription.UTF8String ?: "none");
    return result;
}
- (BOOL)presentRenderbuffer:(NSUInteger)target {
    NSError *error = nil;
    PearANGLEContext *angle = _angleContext;
    __block NSError *inner = nil;
    BOOL result = PearWithContextCurrent(angle, &error, ^BOOL {
        NSError *presentError = nil;
        BOOL presented = [angle presentBoundRenderbuffer:target error:&presentError];
        inner = presentError;
        return presented;
    });
    if (!error) error = inner;
    if (!result && [error.domain isEqualToString:@"PearRuntime.ANGLE.Context"] && error.code == 7) {
        NSError *sharedError = nil;
        result = [angle presentDrawableFromOtherThreadWithError:&sharedError];
        error = result ? nil : sharedError;
    }
    self.lastError = error;
    {
        static unsigned failures;
        if (!result && failures++ < 8) PearANGLETrace( "[PearRuntime][angle] presentation failed: %s\n", error.localizedDescription.UTF8String);
        if (result && !self.reportedPresentation) {
            self.reportedPresentation = YES;
            PearANGLETrace("[PearRuntime][angle] first presentation succeeded\n");
        }
    }
    return result;
}
- (BOOL)presentRenderbuffer:(NSUInteger)target atTime:(CFTimeInterval)time {
    return [self adapterError:5 message:@"Scheduled presentation is not implemented; use immediate presentation"];
}
- (BOOL)presentRenderbuffer:(NSUInteger)target afterMinimumDuration:(CFTimeInterval)duration {
    return [self adapterError:5 message:@"Scheduled presentation is not implemented; use immediate presentation"];
}
@end
