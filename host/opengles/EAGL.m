// PearHost OpenGLES.framework: GL entry points come from ANGLE (re-exported),
// the EAGL layer is ours. Clean-room: from the public EAGL.h / EAGLDrawable.h
// interface. EAGLContext is a placeholder until the EAGL-on-EGL implementation
// in modules/graphics-gl is moved to the host (see docs/HANDOFF.md); every
// message it receives is reported by the missing-call reporter.
#import <Foundation/Foundation.h>
#include "../core/PearHost.h"

#define PEAR_GLES_EXPORT __attribute__((visibility("default")))

PEAR_GLES_EXPORT NSString *const kEAGLDrawablePropertyRetainedBacking = @"EAGLDrawablePropertyRetainedBacking";
PEAR_GLES_EXPORT NSString *const kEAGLDrawablePropertyColorFormat = @"EAGLDrawablePropertyColorFormat";
PEAR_GLES_EXPORT NSString *const kEAGLColorFormatRGBA8 = @"EAGLColorFormatRGBA8";
PEAR_GLES_EXPORT NSString *const kEAGLColorFormatRGB565 = @"EAGLColorFormatRGB565";
PEAR_GLES_EXPORT NSString *const kEAGLColorFormatSRGBA8 = @"EAGLColorFormatSRGBA8";

PEAR_GLES_EXPORT
@interface EAGLSharegroup : NSObject
@end
@implementation EAGLSharegroup
@end

PEAR_GLES_EXPORT
@interface EAGLContext : NSObject
@end
@implementation EAGLContext
// Documented to return nil when the requested API is not available; apps then
// fall back to another renderer (Metal for most current titles).
- (instancetype)initWithAPI:(NSUInteger)api {
    PearMissingReport(PearMissingBehavior, "EAGLContext", "initWithAPI: (EAGL on host not implemented yet; returned nil)",
                      __builtin_return_address(0));
    return nil;
}
- (instancetype)initWithAPI:(NSUInteger)api sharegroup:(EAGLSharegroup *)sharegroup { return [self initWithAPI:api]; }
+ (EAGLContext *)currentContext { return nil; }
+ (BOOL)setCurrentContext:(EAGLContext *)context { return context == nil; }
@end

__attribute__((constructor)) static void PearOpenGLESLoad(void) {
    PearMissingInstallForImage((const void *)&PearOpenGLESLoad);
}
