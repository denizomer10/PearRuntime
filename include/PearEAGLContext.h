#pragma once
#import "PearANGLEDrawable.h"

// Original public-selector adapter, deliberately under a separate class name
// until linked GL imports and dynamic lookups can be routed together.
@interface PearEAGLSharegroup : NSObject
@property(nonatomic, copy) NSString *debugLabel;
@end

@interface PearEAGLContext : NSObject
- (instancetype)init NS_UNAVAILABLE;
+ (instancetype)new NS_UNAVAILABLE;
- (instancetype)initWithAPI:(NSUInteger)api;
- (instancetype)initWithAPI:(NSUInteger)api sharegroup:(PearEAGLSharegroup *)group;
+ (BOOL)setCurrentContext:(PearEAGLContext *)context;
+ (PearEAGLContext *)currentContext;
@property(nonatomic, readonly) NSUInteger API;
@property(nonatomic, strong, readonly) PearEAGLSharegroup *sharegroup;
@property(nonatomic, copy) NSString *debugLabel;
// This compatibility property does not allow concurrent ownership of a context.
@property(nonatomic, getter=isMultiThreaded) BOOL multiThreaded;
@property(atomic, strong, readonly) NSError *lastError;
@property(nonatomic, strong, readonly) PearANGLEContext *angleContext;
- (BOOL)renderbufferStorage:(NSUInteger)target fromDrawable:(id)drawable;
- (BOOL)presentRenderbuffer:(NSUInteger)target;
// Scheduled presentation is explicitly unsupported in this milestone.
- (BOOL)presentRenderbuffer:(NSUInteger)target atTime:(CFTimeInterval)time;
- (BOOL)presentRenderbuffer:(NSUInteger)target afterMinimumDuration:(CFTimeInterval)duration;
@end
