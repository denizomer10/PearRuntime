// Original implementation of public EAGL selectors over ANGLE. No Apple
// implementation is included. This class is not registered as EAGLContext.
#import "PearEAGLContext.h"
#import <QuartzCore/QuartzCore.h>
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
    _angleContext = [[PearANGLEContext alloc] initWithClientVersion:(NSInteger)api sharegroup:_sharegroup.angleGroup error:&error];
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
    if (![PearANGLEContext setCurrentContext:context.angleContext error:&error]) {
        context.lastError = error;
        return NO;
    }
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
    if (format && ![format isEqual:@"EAGLColorFormat8888"])
        return [self adapterError:3 message:@"Only EAGL RGBA8 drawables are currently implemented"];
    id retained = properties[@"EAGLDrawablePropertyRetained"];
    if (retained && ![retained isKindOfClass:NSNumber.class])
        return [self adapterError:4 message:@"Retained backing must be a boolean"];
    NSError *error = nil;
    BOOL result = [_angleContext allocateBoundRenderbuffer:target fromLayer:drawable colorFormat:PearANGLEColorRGBA8
                                         retainedBacking:[retained boolValue] error:&error];
    self.lastError = error;
    if (getenv("PEAR_GRAPHICS_BACKEND")) PearANGLETrace( "[PearRuntime][angle] storage %lux%lu result=%d error=%s\n",
        (unsigned long)_angleContext.pixelWidth, (unsigned long)_angleContext.pixelHeight, result,
        error.localizedDescription.UTF8String ?: "none");
    return result;
}
- (BOOL)presentRenderbuffer:(NSUInteger)target {
    NSError *error = nil;
    BOOL result = [_angleContext presentBoundRenderbuffer:target error:&error];
    self.lastError = error;
    if (getenv("PEAR_GRAPHICS_BACKEND")) {
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
