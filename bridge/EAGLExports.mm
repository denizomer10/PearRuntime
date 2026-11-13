// Original public EAGL constants/entry points. No system GL implementation.
#import "PearANGLEGameRouting.h"
#import "PearANGLEBackend.h"
extern "C" {
extern NSString *const kEAGLDrawablePropertyColorFormat = @"EAGLDrawablePropertyColorFormat";
extern NSString *const kEAGLDrawablePropertyRetainedBacking = @"EAGLDrawablePropertyRetained";
extern NSString *const kEAGLColorFormatRGBA8 = @"EAGLColorFormat8888";
extern NSString *const kEAGLColorFormatRGB565 = @"EAGLColorFormat565";
extern NSString *const kEAGLColorFormatSRGBA8 = @"EAGLColorFormatS8888";
void EAGLGetVersion(unsigned int *major, unsigned int *minor) {
    if (major) *major = 1;
    if (minor) *minor = 0;
}
void *EAGLGetProcAddress(const char *name) { return PearANGLEGameSymbol(name); }
}
