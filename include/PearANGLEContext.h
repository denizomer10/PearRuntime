#pragma once
#import <Foundation/Foundation.h>
#import "PearANGLEBackend.h"
@class CALayer;

@interface PearANGLESharegroup : NSObject
@end

// Context lifecycle for the EAGL adapter. This does not replace EAGLContext
// in a game yet. Surface dimensions are pixels, never UIKit points.
@interface PearANGLEContext : NSObject
- (instancetype)init NS_UNAVAILABLE;
+ (instancetype)new NS_UNAVAILABLE;
- (instancetype)initWithClientVersion:(NSInteger)version
                           sharegroup:(PearANGLESharegroup *)group
                                error:(NSError *__autoreleasing *)error;
@property(nonatomic, strong, readonly) PearANGLESharegroup *sharegroup;
@property(nonatomic, readonly) NSInteger clientVersion;
@property(nonatomic, readonly) NSUInteger pixelWidth;
@property(nonatomic, readonly) NSUInteger pixelHeight;
+ (PearANGLEContext *)currentContext;
+ (BOOL)setCurrentContext:(PearANGLEContext *)context error:(NSError *__autoreleasing *)error;
- (BOOL)resizePixelWidth:(NSUInteger)width height:(NSUInteger)height error:(NSError *__autoreleasing *)error;
// Native surface changes occur on the context's render thread. A layer
// geometry change requires detach/rebind before the next frame is presented.
- (BOOL)bindDrawableLayer:(CALayer *)layer error:(NSError *__autoreleasing *)error;
- (BOOL)detachDrawableLayerWithError:(NSError *__autoreleasing *)error;
@property(nonatomic, readonly) BOOL hasWindowSurface;
// Matching RGBA8 source color attachment 0, in this context's object space.
// The allocation adapter must retain/validate attachment format metadata.
// EAGL renderbuffer/import routing is a separate adapter.
- (BOOL)presentFramebuffer:(unsigned int)framebuffer width:(NSUInteger)width height:(NSUInteger)height
                     error:(NSError *__autoreleasing *)error;
@end
