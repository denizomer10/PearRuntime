// Opt-in public Metal API diagnostics. No shader or texture is modified.
#import "PearRuntime.h"
#import <Metal/Metal.h>
#import <objc/runtime.h>
#include <atomic>

static void PearMetalFailure(NSString *operation, NSError *error) {
    static std::atomic<unsigned> failures{0};
    if (failures.fetch_add(1) >= 32) return;
    NSString *reason = [error.localizedDescription componentsSeparatedByString:@"\n"].firstObject;
    PearLog(@"[PearRuntime][metal-diagnostic] %@ failed domain=%@ code=%ld reason=%@",
            operation, error.domain ?: @"unavailable", (long)error.code, reason ?: @"unavailable");
}

__attribute__((constructor)) static void PearInstallMetalDiagnostics(void) {
    PEAR_MODULE_GUARD();
    if (!getenv("PEAR_DEBUG_METAL")) return;
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (!device) return;
    Class cls = [device class];
    SEL pipeline = @selector(newRenderPipelineStateWithDescriptor:error:);
    Method method = class_getInstanceMethod(cls, pipeline);
    if (method) {
        IMP original = method_getImplementation(method);
        method_setImplementation(method, imp_implementationWithBlock(^id(id self, id descriptor, NSError *__autoreleasing *error) {
            NSError *__autoreleasing local = nil;
            NSError *__autoreleasing *destination = error ?: &local;
            id result = ((id (*)(id, SEL, id, NSError *__autoreleasing *))original)(self, pipeline, descriptor, destination);
            if (!result) PearMetalFailure(@"render pipeline", *destination);
            return result;
        }));
    }
    SEL data = @selector(newLibraryWithData:error:);
    method = class_getInstanceMethod(cls, data);
    if (method) {
        IMP original = method_getImplementation(method);
        method_setImplementation(method, imp_implementationWithBlock(^id(id self, dispatch_data_t bytes, NSError *__autoreleasing *error) {
            NSError *__autoreleasing local = nil;
            NSError *__autoreleasing *destination = error ?: &local;
            id result = ((id (*)(id, SEL, dispatch_data_t, NSError *__autoreleasing *))original)(self, data, bytes, destination);
            if (!result) PearMetalFailure(@"compiled library", *destination);
            return result;
        }));
    }
    SEL source = @selector(newLibraryWithSource:options:error:);
    method = class_getInstanceMethod(cls, source);
    if (method) {
        IMP original = method_getImplementation(method);
        method_setImplementation(method, imp_implementationWithBlock(^id(id self, NSString *text, id options, NSError *__autoreleasing *error) {
            NSError *__autoreleasing local = nil;
            NSError *__autoreleasing *destination = error ?: &local;
            id result = ((id (*)(id, SEL, id, id, NSError *__autoreleasing *))original)(self, source, text, options, destination);
            if (!result) PearMetalFailure(@"source library", *destination);
            return result;
        }));
    }
    PearLog(@"[PearRuntime][metal-diagnostic] library and pipeline failures enabled (limit=32)");
}
