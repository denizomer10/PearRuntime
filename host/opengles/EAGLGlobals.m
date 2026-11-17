#import <Foundation/Foundation.h>
#include "../core/PearHost.h"

#define PEAR_GLES_EXPORT __attribute__((visibility("default")))

PEAR_GLES_EXPORT NSString *const kEAGLDrawablePropertyRetainedBacking = @"EAGLDrawablePropertyRetainedBacking";
PEAR_GLES_EXPORT NSString *const kEAGLDrawablePropertyColorFormat = @"EAGLDrawablePropertyColorFormat";
PEAR_GLES_EXPORT NSString *const kEAGLColorFormatRGBA8 = @"EAGLColorFormatRGBA8";
PEAR_GLES_EXPORT NSString *const kEAGLColorFormatRGB565 = @"EAGLColorFormatRGB565";
PEAR_GLES_EXPORT NSString *const kEAGLColorFormatSRGBA8 = @"EAGLColorFormatSRGBA8";

__attribute__((constructor)) static void PearOpenGLESLoad(void) {
    PearMissingInstallForImage((const void *)&PearOpenGLESLoad);
}
