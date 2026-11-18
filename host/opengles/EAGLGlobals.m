// OpenGLES.framework ABI shim: exported EAGL class symbols and string
// constants iOS binaries bind to. Every method, and the image initializer
// that registers them, is Rust (rust/crates/pear-gles).
#import <Foundation/Foundation.h>

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
@end
