#import "PearANGLEBackend.h"
#include <EGL/eglext.h>
#include <dlfcn.h>
#include <atomic>
#include <mutex>

static std::mutex gANGLELock;
static std::atomic<bool> gANGLEReady{false};
static void *gANGLELibrary;
static EGLDisplay gANGLEDisplay = EGL_NO_DISPLAY;
static PFNEGLGETPROCADDRESSPROC gANGLEGetProc;
static NSString *gANGLEDirectory;

static BOOL PearANGLEError(NSError *__autoreleasing *error, NSInteger code, NSString *reason) {
    if (error) *error = [NSError errorWithDomain:@"PearRuntime.ANGLE" code:code
        userInfo:@{NSLocalizedDescriptionKey: reason}];
    return NO;
}

BOOL PearANGLEInitialize(NSString *directory, NSError *__autoreleasing *error) {
    std::lock_guard<std::mutex> guard(gANGLELock);
    if (!directory.isAbsolutePath)
        return PearANGLEError(error, 1, @"ANGLE requires an absolute library directory");
    NSString *selected = directory.stringByStandardizingPath;
    if (gANGLEReady.load(std::memory_order_acquire))
        return [selected isEqualToString:gANGLEDirectory] ? YES :
            PearANGLEError(error, 2, @"ANGLE backend cannot change within a running process");
    NSString *path = [selected stringByAppendingPathComponent:@"libEGL.dylib"];
    void *library = dlopen(path.fileSystemRepresentation, RTLD_NOW | RTLD_LOCAL);
    if (!library) {
        const char *reason = dlerror();
        return PearANGLEError(error, 3, [NSString stringWithFormat:@"ANGLE load failed: %s", reason ?: "unknown"]);
    }
    auto proc = (PFNEGLGETPROCADDRESSPROC)dlsym(library, "eglGetProcAddress");
    auto initialize = (PFNEGLINITIALIZEPROC)dlsym(library, "eglInitialize");
    auto platform = proc ? (PFNEGLGETPLATFORMDISPLAYEXTPROC)proc("eglGetPlatformDisplayEXT") : nullptr;
    if (!proc || !initialize || !platform) {
        dlclose(library);
        return PearANGLEError(error, 4, @"ANGLE library lacks the required EGL entry points");
    }
    EGLint attributes[] = {EGL_PLATFORM_ANGLE_TYPE_ANGLE, EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE, EGL_NONE};
    EGLDisplay display = platform(EGL_PLATFORM_ANGLE_ANGLE, nullptr, attributes);
    EGLint major = 0, minor = 0;
    if (display == EGL_NO_DISPLAY || !initialize(display, &major, &minor)) {
        dlclose(library);
        return PearANGLEError(error, 5, @"ANGLE could not initialize its Metal display");
    }
    gANGLELibrary = library;
    gANGLEGetProc = proc;
    gANGLEDisplay = display;
    gANGLEDirectory = [selected copy];
    // Keep the display/library alive for the process. Existing EGL contexts
    // must never be invalidated by a loader reload or a settings change.
    gANGLEReady.store(true, std::memory_order_release);
    return YES;
}

void *PearANGLEGetProcAddress(const char *name) {
    if (!name || !*name || !gANGLEReady.load(std::memory_order_acquire)) return nullptr;
    void *symbol = dlsym(gANGLELibrary, name);
    return symbol ?: reinterpret_cast<void *>(gANGLEGetProc(name));
}

EGLDisplay PearANGLEDisplay(void) {
    return gANGLEReady.load(std::memory_order_acquire) ? gANGLEDisplay : EGL_NO_DISPLAY;
}
