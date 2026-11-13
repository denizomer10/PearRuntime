#pragma once
#import "PearANGLEContext.h"
#import <CoreGraphics/CoreGraphics.h>
#include <stdint.h>

// Original bridge contract. System EAGL objects/GL names cannot be passed here.
typedef NS_ENUM(NSUInteger, PearANGLEColorFormat) {
    PearANGLEColorRGBA8 = 1,
    PearANGLEColorRGB565 = 2,
    PearANGLEColorSRGBA8 = 3,
};

typedef struct {
    CGRect logicalBounds;
    CGSize pixelSize;
    CGFloat backingScale;
    uint64_t generation;
} PearANGLEDrawableGeometry;

@interface PearANGLEContext (Drawable)
// Allocates the ANGLE renderbuffer currently bound to GL_RENDERBUFFER.
// RGBA8, single-sample GLES1/2/3; other formats fail explicitly.
// A nil layer releases storage. Allocation/presentation require this context current.
- (BOOL)allocateBoundRenderbuffer:(NSUInteger)target fromLayer:(CALayer *)layer
                     colorFormat:(PearANGLEColorFormat)format retainedBacking:(BOOL)retained
                           error:(NSError *__autoreleasing *)error;
- (BOOL)presentBoundRenderbuffer:(NSUInteger)target error:(NSError *__autoreleasing *)error;
/// EAGL lets the app size a drawable from any thread, but an ANGLE context is
/// current on one thread only. For a layer this context already renders to:
/// unchanged geometry is accepted as is; changed geometry is reallocated by the
/// owning thread at its next presentation.
- (BOOL)acceptDrawableLayer:(CALayer *)layer fromOtherThreadWithError:(NSError *__autoreleasing *)error;
/// Engines that keep the context current on a render thread and present from
/// the main thread (EAGL allows a context current on several threads, ANGLE on
/// one): a presenter context in the same sharegroup owns the native surface
/// and blits the shared renderbuffer. The owning thread calls
/// -releaseNativeSurfaceForSharedThreadUse at its make-current points.
- (BOOL)presentDrawableFromOtherThreadWithError:(NSError *__autoreleasing *)error;
- (void)releaseNativeSurfaceForSharedThreadUse;
// Called by routed GL deletion before ANGLE can recycle an object name.
- (void)forgetDrawableRenderbuffers:(const unsigned int *)names count:(int)count;
// A snapshot becomes invalid when the host's bounds/scale or storage changes.
// Snapshot/mapping queries are safe on an input thread without a current GL
// context. They do not query or mutate GL objects.
- (BOOL)copyDrawableGeometry:(PearANGLEDrawableGeometry *)geometry forRenderbuffer:(unsigned int)renderbuffer;
// Top-left host-layer coordinates -> top-left drawable pixels. Outside points
// and stale generations are rejected, never clamped into a game's controls.
- (BOOL)mapLayerPoint:(CGPoint)point forRenderbuffer:(unsigned int)renderbuffer
          generation:(uint64_t)generation toPixel:(CGPoint *)pixel;
@end
