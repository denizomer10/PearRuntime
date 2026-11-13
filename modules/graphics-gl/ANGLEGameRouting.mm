// Original, opt-in import-slot router using public Mach-O bind opcodes.
// Only the main app and its embedded images are modified, never system images.
#import "PearANGLEGameRouting.h"
#import "PearEAGLContext.h"
#include "PearRuntimeModules.h"
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <mach-o/fixup-chains.h>
#include <mach-o/fat.h>
#include <libkern/OSByteOrder.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <cstddef>
#include <mach/mach_vm.h>
#include <objc/runtime.h>
#include <dlfcn.h>
#include <algorithm>
#include <atomic>
#include <cstring>
#include <string>
#include <vector>
#include <map>
#include <limits.h>
#include <unistd.h>
#include <GLES2/gl2.h>
#include <mutex>
#import "PearANGLEBackend.h"

static std::atomic<bool> gRoutingReady{false};
static std::string &PearAppRoot() { static std::string root; return root; }
static std::string &PearBridgePath() { static std::string path; return path; }
BOOL PearANGLEGameRoutingEnabled(void) { return gRoutingReady.load(std::memory_order_acquire); }
static bool PearIsGLName(const char *name) {
    return name && name[0] == 'g' && name[1] == 'l' && name[2] >= 'A' && name[2] <= 'Z';
}

static void PearRoutedVersion(unsigned int *major, unsigned int *minor) {
    if (major) *major = 1;
    if (minor) *minor = 0;
}
static void *PearRoutedProc(const char *name) { return PearANGLEGameSymbol(name); }
static Class PearRoutedClassName(const char *name) {
    if (!name) return Nil;
    if (!strcmp(name, "EAGLContext")) return PearEAGLContext.class;
    if (!strcmp(name, "EAGLSharegroup")) return PearEAGLSharegroup.class;
    // GLKit's view classes run on the system OpenGL ES; the ANGLE versions
    // live in GLKitAdapter.mm.
    if (!strcmp(name, "GLKView")) return objc_getClass("PearGLKView");
    if (!strcmp(name, "GLKViewController")) return objc_getClass("PearGLKViewController");
    return Nil;
}
static Class PearRoutedNSClass(NSString *name) {
    Class result = PearRoutedClassName(name.UTF8String);
    return result ?: NSClassFromString(name);
}
static Class PearRoutedGetClass(const char *name) { return PearRoutedClassName(name) ?: objc_getClass(name); }
static Class PearRoutedLookupClass(const char *name) { return PearRoutedClassName(name) ?: objc_lookUpClass(name); }
static Class PearRoutedMetaClass(const char *name) {
    Class cls = PearRoutedClassName(name);
    return cls ? object_getClass(cls) : objc_getMetaClass(name);
}
// GL_APPLE_framebuffer_multisample is iOS-only; ANGLE offers the same through
// GL_ANGLE_framebuffer_multisample/blit (ES2) or ES3 core. Original adapters.
static void *PearANGLEFirstProc(const char *preferred, const char *fallback) {
    void *function = PearANGLEGetProcAddress(preferred);
    return function ?: PearANGLEGetProcAddress(fallback);
}
static void GL_APIENTRY PearRenderbufferStorageMultisampleAPPLE(GLenum target, GLsizei samples, GLenum format,
                                                                 GLsizei width, GLsizei height) {
    static auto storage = (void (GL_APIENTRY *)(GLenum, GLsizei, GLenum, GLsizei, GLsizei))
        PearANGLEFirstProc("glRenderbufferStorageMultisampleANGLE", "glRenderbufferStorageMultisample");
    if (storage) storage(target, samples, format, width, height);
}
/// Size of the renderbuffer attached as colour 0 of `framebuffer`, if any.
static bool PearColorRenderbufferSize(GLenum framebuffer, GLint *width, GLint *height) {
    static auto attachment = (void (GL_APIENTRY *)(GLenum, GLenum, GLenum, GLint *))
        PearANGLEGetProcAddress("glGetFramebufferAttachmentParameteriv");
    static auto integer = (void (GL_APIENTRY *)(GLenum, GLint *))PearANGLEGetProcAddress("glGetIntegerv");
    static auto bind = (void (GL_APIENTRY *)(GLenum, GLuint))PearANGLEGetProcAddress("glBindRenderbuffer");
    static auto parameter = (void (GL_APIENTRY *)(GLenum, GLenum, GLint *))
        PearANGLEGetProcAddress("glGetRenderbufferParameteriv");
    if (!attachment || !integer || !bind || !parameter) return false;
    GLint type = GL_NONE, name = 0, previous = 0;
    attachment(framebuffer, GL_COLOR_ATTACHMENT0, GL_FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE, &type);
    if (type != GL_RENDERBUFFER) return false;
    attachment(framebuffer, GL_COLOR_ATTACHMENT0, GL_FRAMEBUFFER_ATTACHMENT_OBJECT_NAME, &name);
    integer(GL_RENDERBUFFER_BINDING, &previous);
    bind(GL_RENDERBUFFER, (GLuint)name);
    parameter(GL_RENDERBUFFER, GL_RENDERBUFFER_WIDTH, width);
    parameter(GL_RENDERBUFFER, GL_RENDERBUFFER_HEIGHT, height);
    bind(GL_RENDERBUFFER, (GLuint)previous);
    return *width > 0 && *height > 0;
}
/// Resolves the read framebuffer (READ_FRAMEBUFFER_APPLE) into the draw one,
/// colour only, over the whole surface; the scissor test applies as on iOS.
static void GL_APIENTRY PearResolveMultisampleFramebufferAPPLE() {
    static auto blit = (void (GL_APIENTRY *)(GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLbitfield, GLenum))
        PearANGLEFirstProc("glBlitFramebufferANGLE", "glBlitFramebuffer");
    constexpr GLenum kRead = 0x8CA8, kDraw = 0x8CA9; // READ/DRAW_FRAMEBUFFER(_APPLE/_ANGLE)
    GLint width = 0, height = 0, drawWidth = 0, drawHeight = 0;
    if (!blit || !PearColorRenderbufferSize(kRead, &width, &height)) return;
    if (PearColorRenderbufferSize(kDraw, &drawWidth, &drawHeight)) {
        width = std::min(width, drawWidth);
        height = std::min(height, drawHeight);
    }
    blit(0, 0, width, height, 0, 0, width, height, GL_COLOR_BUFFER_BIT, GL_NEAREST);
}

// Apple GPUs on iOS never advertise desktop texture compression. Games pick
// their asset pack from the extension list (GTA III: GTA3_DXT.IMG when DXT is
// present), but iOS bundles ship only the PVR/ETC packs. Games see the
// extensions an iPhone would report; ANGLE itself is unchanged.
// PEAR_GL_ALL_EXTENSIONS=1 shows ANGLE's full list.
static bool PearHiddenExtension(const char *name, size_t length) {
    static const char *const hidden[] = {
        "GL_EXT_texture_compression_dxt1", "GL_EXT_texture_compression_s3tc",
        "GL_EXT_texture_compression_s3tc_srgb", "GL_ANGLE_texture_compression_dxt3",
        "GL_ANGLE_texture_compression_dxt5", "GL_EXT_texture_compression_rgtc",
        "GL_EXT_texture_compression_bptc", "GL_AMD_compressed_ATC_texture",
        "GL_NV_texture_compression_s3tc", "GL_S3_s3tc"};
    static const bool everything = getenv("PEAR_GL_ALL_EXTENSIONS") != nullptr;
    if (everything || !name) return false;
    for (const char *entry : hidden)
        if (strlen(entry) == length && !strncmp(entry, name, length)) return true;
    return false;
}
static const GLubyte *GL_APIENTRY PearRoutedGetString(GLenum which) {
    static auto original = (const GLubyte *(GL_APIENTRY *)(GLenum))PearANGLEGetProcAddress("glGetString");
    const GLubyte *value = original ? original(which) : nullptr;
    if (which != GL_EXTENSIONS || !value) return value;
    // ANGLE keeps one extension string per context; filter each once.
    static std::mutex lock;
    static std::map<const GLubyte *, std::string> filtered;
    std::lock_guard<std::mutex> guard(lock);
    auto found = filtered.find(value);
    if (found == filtered.end()) {
        std::string result;
        for (const char *cursor = (const char *)value; *cursor;) {
            while (*cursor == ' ') ++cursor;
            const char *end = cursor;
            while (*end && *end != ' ') ++end;
            if (end > cursor && !PearHiddenExtension(cursor, size_t(end - cursor))) {
                if (!result.empty()) result += ' ';
                result.append(cursor, end);
            }
            cursor = end;
        }
        found = filtered.emplace(value, std::move(result)).first;
    }
    return (const GLubyte *)found->second.c_str();
}
static constexpr GLenum kPearNumExtensions = 0x821D; // GL_NUM_EXTENSIONS (ES3)
/// ES3 indexed extension queries, renumbered without the hidden entries.
static GLint PearVisibleExtension(GLint wanted, const GLubyte **name) {
    static auto integer = (void (GL_APIENTRY *)(GLenum, GLint *))PearANGLEGetProcAddress("glGetIntegerv");
    static auto indexed = (const GLubyte *(GL_APIENTRY *)(GLenum, GLuint))PearANGLEGetProcAddress("glGetStringi");
    GLint total = 0, visible = 0;
    if (!integer || !indexed) return 0;
    integer(kPearNumExtensions, &total);
    for (GLint index = 0; index < total; ++index) {
        const GLubyte *entry = indexed(GL_EXTENSIONS, (GLuint)index);
        if (!entry || PearHiddenExtension((const char *)entry, strlen((const char *)entry))) continue;
        if (visible == wanted && name) { *name = entry; return visible; }
        ++visible;
    }
    return visible;
}
static const GLubyte *GL_APIENTRY PearRoutedGetStringi(GLenum which, GLuint index) {
    static auto original = (const GLubyte *(GL_APIENTRY *)(GLenum, GLuint))PearANGLEGetProcAddress("glGetStringi");
    if (which != GL_EXTENSIONS || !original) return original ? original(which, index) : nullptr;
    const GLubyte *name = nullptr;
    PearVisibleExtension((GLint)index, &name);
    // Out of range: let ANGLE record GL_INVALID_VALUE as usual.
    return name ?: original(which, 0x7fffffff);
}
static void GL_APIENTRY PearRoutedGetIntegerv(GLenum which, GLint *value) {
    static auto original = (void (GL_APIENTRY *)(GLenum, GLint *))PearANGLEGetProcAddress("glGetIntegerv");
    if (!original) return;
    original(which, value);
    if (which == kPearNumExtensions && value) *value = PearVisibleExtension(-1, nullptr);
}

// Shipped iOS clients (including Dr. Driving) import unsuffixed framebuffer
// names in GLES1 contexts. ANGLE validates GLES2 core names strictly; dispatch these
// equivalent operations to OES when the actual context is GLES1. The exported
// function stays stable when a process switches between GLES1 and GLES2/3.
#define PEAR_FRAMEBUFFER_ALIAS(name, result, parameters, arguments) \
static result GL_APIENTRY PearCompat_##name parameters { \
    using Function = result (GL_APIENTRY *) parameters; \
    static Function core = (Function)PearANGLEGetProcAddress(#name); \
    static Function oes = (Function)PearANGLEGetProcAddress(#name "OES"); \
    Function function = PearANGLEContext.currentContext.clientVersion == 1 ? oes : core; \
    return function arguments; \
}
PEAR_FRAMEBUFFER_ALIAS(glGenRenderbuffers, void, (GLsizei count, GLuint *names), (count, names))
PEAR_FRAMEBUFFER_ALIAS(glBindRenderbuffer, void, (GLenum target, GLuint name), (target, name))
PEAR_FRAMEBUFFER_ALIAS(glIsRenderbuffer, GLboolean, (GLuint name), (name))
PEAR_FRAMEBUFFER_ALIAS(glRenderbufferStorage, void, (GLenum target, GLenum format, GLsizei width, GLsizei height), (target, format, width, height))
PEAR_FRAMEBUFFER_ALIAS(glGetRenderbufferParameteriv, void, (GLenum target, GLenum parameter, GLint *value), (target, parameter, value))
PEAR_FRAMEBUFFER_ALIAS(glGenFramebuffers, void, (GLsizei count, GLuint *names), (count, names))
PEAR_FRAMEBUFFER_ALIAS(glBindFramebuffer, void, (GLenum target, GLuint name), (target, name))
PEAR_FRAMEBUFFER_ALIAS(glDeleteFramebuffers, void, (GLsizei count, const GLuint *names), (count, names))
PEAR_FRAMEBUFFER_ALIAS(glIsFramebuffer, GLboolean, (GLuint name), (name))

PEAR_FRAMEBUFFER_ALIAS(glFramebufferRenderbuffer, void, (GLenum target, GLenum attachment, GLenum kind, GLuint name), (target, attachment, kind, name))
PEAR_FRAMEBUFFER_ALIAS(glFramebufferTexture2D, void, (GLenum target, GLenum attachment, GLenum kind, GLuint name, GLint level), (target, attachment, kind, name, level))
PEAR_FRAMEBUFFER_ALIAS(glGetFramebufferAttachmentParameteriv, void, (GLenum target, GLenum attachment, GLenum parameter, GLint *value), (target, attachment, parameter, value))
#undef PEAR_FRAMEBUFFER_ALIAS

static GLenum GL_APIENTRY PearCompat_glCheckFramebufferStatus(GLenum target) {
    using Function = GLenum (GL_APIENTRY *)(GLenum);
    static Function core = (Function)PearANGLEGetProcAddress("glCheckFramebufferStatus");
    static Function oes = (Function)PearANGLEGetProcAddress("glCheckFramebufferStatusOES");
    GLenum result = (PearANGLEContext.currentContext.clientVersion == 1 ? oes : core)(target);
    static std::atomic<unsigned> failures{0};
    if (result != GL_FRAMEBUFFER_COMPLETE && getenv("PEAR_DEBUG_GL") && failures.fetch_add(1) < 32)
        PearANGLETrace("[PearRuntime][angle-validation] framebuffer target=0x%x status=0x%x GLES%ld\n",
            target, result, (long)PearANGLEContext.currentContext.clientVersion);
    return result;
}
static void GL_APIENTRY PearDiagnosticCompileShader(GLuint shader) {
    static auto compile = (void (*)(GLuint))PearANGLEGetProcAddress("glCompileShader");
    static auto query = (void (*)(GLuint, GLenum, GLint *))PearANGLEGetProcAddress("glGetShaderiv");
    static auto info = (void (*)(GLuint, GLsizei, GLsizei *, GLchar *))PearANGLEGetProcAddress("glGetShaderInfoLog");
    compile(shader);
    GLint compiled = 0; query(shader, GL_COMPILE_STATUS, &compiled);
    if (!compiled) {
        char log[2048] = {}; GLsizei length = 0; info(shader, sizeof(log)-1, &length, log);
        PearANGLETrace("[PearRuntime][angle-validation] shader %u compile failed: %.*s\n", shader, (int)length, log);
    }
}
static void GL_APIENTRY PearDiagnosticLinkProgram(GLuint program) {
    static auto link = (void (*)(GLuint))PearANGLEGetProcAddress("glLinkProgram");
    static auto query = (void (*)(GLuint, GLenum, GLint *))PearANGLEGetProcAddress("glGetProgramiv");
    static auto info = (void (*)(GLuint, GLsizei, GLsizei *, GLchar *))PearANGLEGetProcAddress("glGetProgramInfoLog");
    link(program);
    GLint linked = 0; query(program, GL_LINK_STATUS, &linked);
    if (!linked) {
        char log[2048] = {}; GLsizei length = 0; info(program, sizeof(log)-1, &length, log);
        PearANGLETrace("[PearRuntime][angle-validation] program %u link failed: %.*s\n", program, (int)length, log);
    }
}
static GLint GL_APIENTRY PearDiagnosticUniformLocation(GLuint program, const GLchar *name) {
    static auto original = (GLint (*)(GLuint, const GLchar *))PearANGLEGetProcAddress("glGetUniformLocation");
    GLint location = original(program, name);
    static std::atomic<unsigned> count{0};
    if (count.fetch_add(1) < 128)
        PearANGLETrace("[PearRuntime][angle-validation] uniform program=%u name=%s location=%d\n", program, name ?: "(null)", location);
    return location;
}
static void *GL_APIENTRY PearDiagnosticMapBufferOES(GLenum target, GLenum access) {
    static auto original = (void *(*)(GLenum, GLenum))PearANGLEGetProcAddress("glMapBufferOES");
    void *result = original(target, access);
    static std::atomic<unsigned> count{0};
    if (count.fetch_add(1) < 64)
        PearANGLETrace("[PearRuntime][angle-validation] mapBuffer target=0x%x access=0x%x null=%d GLES%ld\n",
            target, access, result == nullptr, (long)PearANGLEContext.currentContext.clientVersion);
    return result;
}

// iOS accepts READ/DRAW_FRAMEBUFFER(_APPLE) in glDiscardFramebufferEXT (games
// discard the multisample buffer after the APPLE resolve); ANGLE's extension
// accepts only GL_FRAMEBUFFER. Those targets go to ES3 glInvalidateFramebuffer,
// with EXT default-framebuffer names mapped to its enums. Discard is a hint, so
// when no equivalent exists the call is dropped instead of raising an error.
static void GL_APIENTRY PearRoutedDiscardFramebufferEXT(GLenum target, GLsizei count, const GLenum *attachments) {
    static auto discard = (void (GL_APIENTRY *)(GLenum, GLsizei, const GLenum *))PearANGLEGetProcAddress("glDiscardFramebufferEXT");
    static auto invalidate = (void (GL_APIENTRY *)(GLenum, GLsizei, const GLenum *))PearANGLEGetProcAddress("glInvalidateFramebuffer");
    static auto integer = (void (GL_APIENTRY *)(GLenum, GLint *))PearANGLEGetProcAddress("glGetIntegerv");
    constexpr GLenum kRead = 0x8CA8, kDraw = 0x8CA9;
    if (target != kRead && target != kDraw) { if (discard) discard(target, count, attachments); return; }
    if (!invalidate || !integer || count <= 0 || count > 16 || !attachments) return;
    GLint bound = 0;
    integer(target == kRead ? 0x8CAA /* READ_FRAMEBUFFER_BINDING */ : 0x8CA6 /* DRAW_FRAMEBUFFER_BINDING */, &bound);
    GLenum mapped[16];
    for (GLsizei index = 0; index < count; ++index) {
        GLenum value = attachments[index];
        if (!bound) value = value == 0x1800 ? 0x1800 /* GL_COLOR */ : value == 0x1801 ? 0x1801 : value == 0x1802 ? 0x1802 : value;
        else value = value == 0x1800 ? GL_COLOR_ATTACHMENT0 : value == 0x1801 ? GL_DEPTH_ATTACHMENT : value == 0x1802 ? GL_STENCIL_ATTACHMENT : value;
        mapped[index] = value;
    }
    GLint major = 0;
    integer(0x821B /* GL_MAJOR_VERSION */, &major);
    if (major >= 3) invalidate(target, count, mapped);
}

static void PearRoutedDeleteRenderbuffers(GLsizei count, const GLuint *names) {
    [PearANGLEContext.currentContext forgetDrawableRenderbuffers:names count:count];
    static auto core = (void (*)(GLsizei, const GLuint *))PearANGLEGetProcAddress("glDeleteRenderbuffers");
    static auto oes = (void (*)(GLsizei, const GLuint *))PearANGLEGetProcAddress("glDeleteRenderbuffersOES");
    auto original = PearANGLEContext.currentContext.clientVersion == 1 ? oes : core;
    if (original) original(count, names);
}
static void *PearRoutedDlsym(void *handle, const char *name) {
    void *replacement = PearANGLEGameSymbol(name);
    // An unavailable GL function must never silently resolve into system GL.
    if (replacement || PearIsGLName(name)) return replacement;
    // Preserve RTLD_NEXT's caller-image semantics on unrelated lookups.
    [[clang::musttail]] return dlsym(handle, name);
}
static void *PearRoutedDlopen(const char *path, int mode) {
    if (path && (strstr(path, "OpenGLES.framework/") || !strcmp(path, "OpenGLES"))) {
        if (PearBridgePath().empty()) {
            PearANGLETrace("[PearRuntime][angle-route] missing ANGLE ABI bridge; system OpenGLES load refused\n");
            return nullptr;
        }
        return dlopen(PearBridgePath().c_str(), mode);
    }
    [[clang::musttail]] return dlopen(path, mode);
}
static void *PearRoutedBundleProc(CFBundleRef bundle, CFStringRef name) {
    char symbol[256];
    if (name && CFStringGetCString(name, symbol, sizeof(symbol), kCFStringEncodingUTF8)) {
        void *replacement = PearANGLEGameSymbol(symbol);
        if (replacement || PearIsGLName(symbol)) return replacement;
    }
    return CFBundleGetFunctionPointerForName(bundle, name);
}

/// Library directory for lazy initialization. Set when the app does not link
/// OpenGL ES itself (the bridge was found next to the runtime instead of being
/// loaded): ANGLE then starts on the app's first GL use, so a Metal-only game
/// pays nothing for it. Linked apps initialize eagerly in the constructor.
static std::string &PearLazyANGLEDirectory() { static std::string path; return path; }

static void PearEnsureANGLE() {
    if (PearANGLEDisplay() != EGL_NO_DISPLAY || PearLazyANGLEDirectory().empty()) return;
    static std::once_flag once;
    std::call_once(once, [] {
        @autoreleasepool {
            NSError *error = nil;
            NSString *libraries = [NSString stringWithUTF8String:PearLazyANGLEDirectory().c_str()];
            if (!PearANGLEInitialize(libraries, &error)) {
                // No system OpenGL ES fallback: refuse instead of mixing backends.
                PearANGLETrace("[PearRuntime][angle-route] lazy ANGLE initialization failed: %s\n",
                               error.localizedDescription.UTF8String ?: "unknown");
                _exit(78);
            }
            PearANGLETrace("[PearRuntime][angle-route] ANGLE initialized on first GL use\n");
        }
    });
}

/// Routed C/ObjC lookup functions themselves need no GL; only what they
/// resolve to does (those calls come back through PearANGLEGameSymbol).
static bool PearIsLookupWrapper(const char *name) {
    static const char *const wrappers[] = {"dlsym", "dlopen", "CFBundleGetFunctionPointerForName",
        "NSClassFromString", "objc_getClass", "objc_lookUpClass", "objc_getMetaClass"};
    for (const char *wrapper : wrappers) if (!strcmp(name, wrapper)) return true;
    return false;
}

void *PearANGLEGameSymbol(const char *name) {
    if (!name || !PearANGLEGameRoutingEnabled()) return nullptr;
    if (!PearIsLookupWrapper(name) && (PearIsGLName(name) || !strncmp(name, "OBJC_", 5) ||
                                       !strncmp(name, "EAGL", 4) || !strcmp(name, "eglGetProcAddress")))
        PearEnsureANGLE();
    if (!strcmp(name, "OBJC_CLASS_$_EAGLContext")) return (__bridge void *)PearEAGLContext.class;
    if (!strcmp(name, "OBJC_CLASS_$_EAGLSharegroup")) return (__bridge void *)PearEAGLSharegroup.class;
    if (!strcmp(name, "OBJC_METACLASS_$_EAGLContext")) return (__bridge void *)object_getClass(PearEAGLContext.class);
    if (!strcmp(name, "OBJC_METACLASS_$_EAGLSharegroup")) return (__bridge void *)object_getClass(PearEAGLSharegroup.class);
    if (!strcmp(name, "OBJC_CLASS_$_GLKView")) return (__bridge void *)objc_getClass("PearGLKView");
    if (!strcmp(name, "OBJC_METACLASS_$_GLKView")) return (__bridge void *)objc_getMetaClass("PearGLKView");
    if (!strcmp(name, "OBJC_CLASS_$_GLKViewController")) return (__bridge void *)objc_getClass("PearGLKViewController");
    if (!strcmp(name, "OBJC_METACLASS_$_GLKViewController")) return (__bridge void *)objc_getMetaClass("PearGLKViewController");
    if (!strcmp(name, "glCheckFramebufferStatusOES")) return (void *)PearCompat_glCheckFramebufferStatus;
    if (getenv("PEAR_DEBUG_GL")) {
        if (!strcmp(name, "glCompileShader")) return (void *)PearDiagnosticCompileShader;
        if (!strcmp(name, "glLinkProgram")) return (void *)PearDiagnosticLinkProgram;
        if (!strcmp(name, "glGetUniformLocation")) return (void *)PearDiagnosticUniformLocation;
    }
    if (!strcmp(name, "glMapBufferOES") && getenv("PEAR_DEBUG_GL")) return (void *)PearDiagnosticMapBufferOES;
#define PEAR_ROUTE_FRAMEBUFFER(entry) if (!strcmp(name, #entry)) return (void *)PearCompat_##entry
    PEAR_ROUTE_FRAMEBUFFER(glGenRenderbuffers);
    PEAR_ROUTE_FRAMEBUFFER(glBindRenderbuffer);
    PEAR_ROUTE_FRAMEBUFFER(glIsRenderbuffer);
    PEAR_ROUTE_FRAMEBUFFER(glRenderbufferStorage);
    PEAR_ROUTE_FRAMEBUFFER(glGetRenderbufferParameteriv);
    PEAR_ROUTE_FRAMEBUFFER(glGenFramebuffers);
    PEAR_ROUTE_FRAMEBUFFER(glBindFramebuffer);
    PEAR_ROUTE_FRAMEBUFFER(glDeleteFramebuffers);
    PEAR_ROUTE_FRAMEBUFFER(glIsFramebuffer);
    PEAR_ROUTE_FRAMEBUFFER(glCheckFramebufferStatus);
    PEAR_ROUTE_FRAMEBUFFER(glFramebufferRenderbuffer);
    PEAR_ROUTE_FRAMEBUFFER(glFramebufferTexture2D);
    PEAR_ROUTE_FRAMEBUFFER(glGetFramebufferAttachmentParameteriv);
#undef PEAR_ROUTE_FRAMEBUFFER
    if (!strcmp(name, "glDeleteRenderbuffers") || !strcmp(name, "glDeleteRenderbuffersOES")) return (void *)PearRoutedDeleteRenderbuffers;
    if (!strcmp(name, "glGetString")) return (void *)PearRoutedGetString;
    if (!strcmp(name, "glDiscardFramebufferEXT")) return (void *)PearRoutedDiscardFramebufferEXT;
    if (!strcmp(name, "glGetStringi")) return (void *)PearRoutedGetStringi;
    if (!strcmp(name, "glGetIntegerv")) return (void *)PearRoutedGetIntegerv;
    if (!strcmp(name, "glRenderbufferStorageMultisampleAPPLE")) return (void *)PearRenderbufferStorageMultisampleAPPLE;
    if (!strcmp(name, "glResolveMultisampleFramebufferAPPLE")) return (void *)PearResolveMultisampleFramebufferAPPLE;
    if (!strcmp(name, "EAGLGetVersion")) return (void *)PearRoutedVersion;
    if (!strcmp(name, "EAGLGetProcAddress") || !strcmp(name, "eglGetProcAddress")) return (void *)PearRoutedProc;
    if (!strcmp(name, "NSClassFromString")) return (void *)PearRoutedNSClass;
    if (!strcmp(name, "objc_getClass")) return (void *)PearRoutedGetClass;
    if (!strcmp(name, "objc_lookUpClass")) return (void *)PearRoutedLookupClass;
    if (!strcmp(name, "objc_getMetaClass")) return (void *)PearRoutedMetaClass;
    if (!strcmp(name, "dlsym")) return (void *)PearRoutedDlsym;
    if (!strcmp(name, "dlopen")) return (void *)PearRoutedDlopen;
    if (!strcmp(name, "CFBundleGetFunctionPointerForName")) return (void *)PearRoutedBundleProc;
    if (PearIsGLName(name)) return PearANGLEGetProcAddress(name);
    return nullptr;
}

static bool PearRoutingCandidate(const char *name) {
    if (!name || *name != '_') return false;
    ++name;
    return PearIsGLName(name) || !strncmp(name, "EAGL", 4) ||
        !strcmp(name, "eglGetProcAddress") || !strcmp(name, "OBJC_CLASS_$_EAGLContext") ||
        !strcmp(name, "OBJC_CLASS_$_EAGLSharegroup") || !strcmp(name, "NSClassFromString") ||
        !strcmp(name, "OBJC_METACLASS_$_EAGLContext") || !strcmp(name, "OBJC_METACLASS_$_EAGLSharegroup") ||
        !strcmp(name, "objc_getClass") || !strcmp(name, "objc_lookUpClass") ||
        !strcmp(name, "objc_getMetaClass") ||
        !strcmp(name, "OBJC_CLASS_$_GLKView") || !strcmp(name, "OBJC_METACLASS_$_GLKView") ||
        !strcmp(name, "OBJC_CLASS_$_GLKViewController") || !strcmp(name, "OBJC_METACLASS_$_GLKViewController") ||
        !strcmp(name, "dlsym") || !strcmp(name, "dlopen") || !strcmp(name, "CFBundleGetFunctionPointerForName");
}

struct PearImportPatch { uintptr_t *slot; uintptr_t replacement; };
static bool PearReadULEB(const uint8_t *&p, const uint8_t *end, uint64_t &value) {
    value = 0;
    for (unsigned shift = 0; shift < 64 && p < end; shift += 7) {
        uint8_t byte = *p++;
        if (shift == 63 && (byte & 0x7e)) return false;
        value |= uint64_t(byte & 0x7f) << shift;
        if (!(byte & 0x80)) return true;
    }
    return false;
}
static bool PearReadAddend(const uint8_t *&p, const uint8_t *end, bool &nonzero) {
    nonzero = false;
    for (unsigned index = 0; index < 10 && p < end; ++index) {
        uint8_t byte = *p++;
        if (index == 9 && byte != 0 && byte != 0x7f) return false;
        nonzero |= (byte & 0x7f) != 0;
        if (!(byte & 0x80)) return true;
    }
    return false;
}

static bool PearReadBindings(const uint8_t *begin, size_t size, bool lazy,
    const std::vector<const segment_command_64 *> &segments, intptr_t slide,
    const std::vector<std::string> &dylibs, std::vector<PearImportPatch> &patches) {
    const uint8_t *p = begin, *end = begin + size;
    unsigned segment = UINT_MAX, type = BIND_TYPE_POINTER;
    uint64_t offset = 0;
    bool hasAddend = false;
    int ordinal = 0;
    const char *symbol = nullptr;
    auto bind = [&]() -> bool {
        if (!PearRoutingCandidate(symbol)) return true;
        // A converted image already links our typed exports. Keep those slots
        // intact; only dynamic lookup APIs still need import routing there.
        if (ordinal > 0 && (size_t)ordinal <= dylibs.size() &&
            dylibs[ordinal - 1].find("libPearOpenGLES.dylib") != std::string::npos) return true;
        if (segment >= segments.size() || type != BIND_TYPE_POINTER || hasAddend) {
            PearANGLETrace( "[PearRuntime][angle-route] invalid binding %s segment=%u type=%u addend=%d\n", symbol, segment, type, hasAddend);
            return false;
        }
        const auto *s = segments[segment];
        if (offset > s->vmsize || s->vmsize - offset < sizeof(uintptr_t) || offset % sizeof(uintptr_t)) {
            PearANGLETrace( "[PearRuntime][angle-route] invalid slot %s offset=%llu size=%llu\n", symbol, offset, s->vmsize);
            return false;
        }
        void *replacement = PearANGLEGameSymbol(symbol + 1);
        if (!replacement) { PearANGLETrace( "[PearRuntime][angle-route] unsupported import %s\n", symbol); return false; }
        patches.push_back({(uintptr_t *)(slide + s->vmaddr + offset), (uintptr_t)replacement});
        return true;
    };
    while (p < end) {
        uint8_t instruction = *p++, opcode = instruction & BIND_OPCODE_MASK, imm = instruction & BIND_IMMEDIATE_MASK;
        if (getenv("PEAR_DEBUG_ANGLE_ROUTE")) PearANGLETrace( "[angle-bind] %zu: %02x\n", size_t(p - begin - 1), instruction);
        uint64_t value = 0, count = 0;
        switch (opcode) {
            case BIND_OPCODE_DONE:
                if (!lazy) return true;
                segment = UINT_MAX; offset = 0; symbol = nullptr; type = BIND_TYPE_POINTER; hasAddend = false; ordinal = 0;
                break;
            case BIND_OPCODE_SET_DYLIB_ORDINAL_IMM: ordinal = imm; break;
            case BIND_OPCODE_SET_DYLIB_SPECIAL_IMM: ordinal = imm ? (int8_t)(imm | 0xf0) : 0; break;
            case BIND_OPCODE_SET_DYLIB_ORDINAL_ULEB:
                if (!PearReadULEB(p, end, value) || value > INT_MAX) return false;
                ordinal = (int)value;
                break;
            case BIND_OPCODE_SET_SYMBOL_TRAILING_FLAGS_IMM:
                symbol = (const char *)p;
                while (p < end && *p) ++p;
                if (p == end) return false;
                ++p; break;
            case BIND_OPCODE_SET_TYPE_IMM: type = imm; break;
            case BIND_OPCODE_SET_ADDEND_SLEB:
                // Only zero addends are meaningful for these function/class imports.
                if (!PearReadAddend(p, end, hasAddend)) return false;
                break;
            case BIND_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB:
                segment = imm;
                if (!PearReadULEB(p, end, offset)) return false;
                break;
            case BIND_OPCODE_ADD_ADDR_ULEB:
                // Linkers encode backwards moves as a two's-complement ULEB.
                // Match dyld's unsigned wrap; validate the final slot at bind.
                if (!PearReadULEB(p, end, value)) return false;
                offset += value; break;
            case BIND_OPCODE_DO_BIND:
                if (!bind()) return false;
                offset += sizeof(uintptr_t); break;
            case BIND_OPCODE_DO_BIND_ADD_ADDR_ULEB:
                if (!bind() || !PearReadULEB(p, end, value)) return false;
                offset += value + sizeof(uintptr_t); break;
            case BIND_OPCODE_DO_BIND_ADD_ADDR_IMM_SCALED:
                value = (imm + 1) * sizeof(uintptr_t);
                if (!bind()) return false;
                offset += value; break;
            case BIND_OPCODE_DO_BIND_ULEB_TIMES_SKIPPING_ULEB:
                if (!PearReadULEB(p, end, count) || !PearReadULEB(p, end, value) || count > 1000000) return false;
                for (uint64_t index = 0; index < count; ++index) {
                    if (!bind()) return false;
                    offset += value + sizeof(uintptr_t);
                }
                break;
            default:
                PearANGLETrace( "[PearRuntime][angle-route] unsupported bind opcode 0x%x\n", opcode);
                return false; // Chained/threaded formats need a separate decoder.
        }
    }
    return true;
}

/// Import slots of an image linked with chained fixups (LC_DYLD_CHAINED_FIXUPS).
/// dyld overwrites the chains with final pointers while it loads the image, so
/// the chains are walked in the file on disk; the import table is read from the
/// mapped __LINKEDIT. Only the plain 64-bit pointer formats (arm64) are decoded.
static bool PearReadChainedBindings(const char *path, const linkedit_data_command *fixups, uintptr_t linkeditBase,
    const std::vector<const segment_command_64 *> &segments, intptr_t slide,
    const std::vector<std::string> &dylibs, std::vector<PearImportPatch> &patches) {
    const uint8_t *data = (const uint8_t *)(linkeditBase + fixups->dataoff);
    const uint8_t *dataEnd = data + fixups->datasize;
    if (fixups->datasize < sizeof(dyld_chained_fixups_header)) return false;
    const auto *header = (const dyld_chained_fixups_header *)data;
    if (header->fixups_version != 0 || header->symbols_format != 0 ||
        header->starts_offset >= fixups->datasize || header->imports_offset >= fixups->datasize ||
        header->symbols_offset >= fixups->datasize) {
        PearANGLETrace("[PearRuntime][angle-route] unsupported chained fixups header\n");
        return false;
    }
    size_t importSize = header->imports_format == DYLD_CHAINED_IMPORT ? 4 :
                        header->imports_format == DYLD_CHAINED_IMPORT_ADDEND ? 8 :
                        header->imports_format == DYLD_CHAINED_IMPORT_ADDEND64 ? 16 : 0;
    if (!importSize || header->imports_offset + uint64_t(header->imports_count) * importSize > fixups->datasize) return false;
    const char *symbols = (const char *)data + header->symbols_offset;
    size_t symbolsSize = fixups->datasize - header->symbols_offset;
    // Import ordinal -> (library ordinal, name).
    auto import = [&](uint32_t index, int &library, const char *&name) -> bool {
        if (index >= header->imports_count) return false;
        const uint8_t *entry = data + header->imports_offset + size_t(index) * importSize;
        uint64_t nameOffset;
        if (header->imports_format == DYLD_CHAINED_IMPORT_ADDEND64) {
            const auto *value = (const dyld_chained_import_addend64 *)entry;
            library = (int16_t)value->lib_ordinal; nameOffset = value->name_offset;
            if (value->addend) return false;
        } else {
            uint32_t raw; memcpy(&raw, entry, 4);
            library = (int8_t)(raw & 0xff); nameOffset = raw >> 9;
            if (header->imports_format == DYLD_CHAINED_IMPORT_ADDEND) {
                int32_t addend; memcpy(&addend, entry + 4, 4);
                if (addend) return false;
            }
        }
        if (nameOffset >= symbolsSize || strnlen(symbols + nameOffset, symbolsSize - nameOffset) == symbolsSize - nameOffset) return false;
        name = symbols + nameOffset;
        return true;
    };

    // The original chains live in the arm64 slice of the file on disk.
    int descriptor = open(path, O_RDONLY | O_CLOEXEC);
    if (descriptor < 0) return false;
    struct stat info = {};
    if (fstat(descriptor, &info) != 0 || info.st_size < (off_t)sizeof(mach_header_64)) { close(descriptor); return false; }
    size_t fileSize = (size_t)info.st_size;
    void *mapping = mmap(nullptr, fileSize, PROT_READ, MAP_PRIVATE, descriptor, 0);
    close(descriptor);
    if (mapping == MAP_FAILED) return false;
    struct Unmap { void *address; size_t size; ~Unmap() { munmap(address, size); } } unmap{mapping, fileSize};
    const uint8_t *file = (const uint8_t *)mapping;
    uint64_t slice = 0, sliceSize = fileSize;
    uint32_t magic; memcpy(&magic, file, 4);
    if (magic == FAT_CIGAM || magic == FAT_MAGIC) {
        uint32_t count = OSSwapBigToHostInt32(((const fat_header *)file)->nfat_arch);
        bool found = false;
        for (uint32_t index = 0; index < count && sizeof(fat_header) + (index + 1) * sizeof(fat_arch) <= fileSize; ++index) {
            const fat_arch *arch = (const fat_arch *)(file + sizeof(fat_header)) + index;
            if ((cpu_type_t)OSSwapBigToHostInt32(arch->cputype) == CPU_TYPE_ARM64 &&
                (OSSwapBigToHostInt32(arch->cpusubtype) & ~CPU_SUBTYPE_MASK) != CPU_SUBTYPE_ARM64E) {
                slice = OSSwapBigToHostInt32(arch->offset); sliceSize = OSSwapBigToHostInt32(arch->size); found = true; break;
            }
        }
        if (!found || slice + sliceSize > fileSize) return false;
    }

    const uint8_t *startsBase = data + header->starts_offset;
    if (startsBase + sizeof(uint32_t) > dataEnd) return false;
    const auto *starts = (const dyld_chained_starts_in_image *)startsBase;
    if (starts->seg_count > segments.size() || startsBase + sizeof(uint32_t) * (1 + uint64_t(starts->seg_count)) > dataEnd) return false;
    for (uint32_t segmentIndex = 0; segmentIndex < starts->seg_count; ++segmentIndex) {
        uint32_t infoOffset = starts->seg_info_offset[segmentIndex];
        if (!infoOffset) continue;
        const uint8_t *segmentBase = startsBase + infoOffset;
        if (segmentBase + offsetof(dyld_chained_starts_in_segment, page_start) > dataEnd) return false;
        const auto *chain = (const dyld_chained_starts_in_segment *)segmentBase;
        if (segmentBase + offsetof(dyld_chained_starts_in_segment, page_start) + sizeof(uint16_t) * uint64_t(chain->page_count) > dataEnd) return false;
        if (chain->pointer_format != DYLD_CHAINED_PTR_64 && chain->pointer_format != DYLD_CHAINED_PTR_64_OFFSET) {
            PearANGLETrace("[PearRuntime][angle-route] unsupported chained pointer format %u\n", chain->pointer_format);
            return false;
        }
        const segment_command_64 *segment = segments[segmentIndex];
        for (uint32_t page = 0; page < chain->page_count; ++page) {
            uint16_t start = chain->page_start[page];
            if (start == DYLD_CHAINED_PTR_START_NONE) continue;
            if (start & DYLD_CHAINED_PTR_START_MULTI) return false; // 32-bit formats only
            uint64_t offset = uint64_t(page) * chain->page_size + start;
            for (unsigned guard = 0; guard < 1000000; ++guard) {
                if (offset + 8 > segment->filesize || offset + 8 > segment->vmsize) return false;
                uint64_t fileOffset = segment->fileoff + offset;
                if (fileOffset + 8 > sliceSize) return false;
                uint64_t raw; memcpy(&raw, file + slice + fileOffset, 8);
                if (raw >> 63) { // bind
                    dyld_chained_ptr_64_bind bind; memcpy(&bind, &raw, 8);
                    int library = 0; const char *name = nullptr;
                    if (!import(bind.ordinal, library, name)) {
                        PearANGLETrace("[PearRuntime][angle-route] invalid chained import %u\n", bind.ordinal);
                        return false;
                    }
                    bool bridged = library > 0 && (size_t)library <= dylibs.size() &&
                                   dylibs[library - 1].find("libPearOpenGLES.dylib") != std::string::npos;
                    if (PearRoutingCandidate(name) && !bridged) {
                        if (bind.addend || offset % sizeof(uintptr_t)) return false;
                        void *replacement = PearANGLEGameSymbol(name + 1);
                        if (!replacement) { PearANGLETrace("[PearRuntime][angle-route] unsupported import %s\n", name); return false; }
                        patches.push_back({(uintptr_t *)(slide + segment->vmaddr + offset), (uintptr_t)replacement});
                    }
                }
                uint64_t next = (raw >> 51) & 0xfff;
                if (!next) break;
                offset += next * 4;
            }
        }
    }
    return true;
}

static bool PearApplyPatches(const std::vector<PearImportPatch> &patches) {
    std::map<mach_vm_address_t, vm_prot_t> pages;
    mach_vm_size_t pageSize = (mach_vm_size_t)getpagesize();
    bool ready = true;
    for (const auto &patch : patches) {
        mach_vm_address_t page = (uintptr_t)patch.slot & ~(pageSize - 1);
        if (pages.count(page)) continue;
        mach_vm_address_t region = page;
        mach_vm_size_t regionSize = 0;
        vm_region_basic_info_data_64_t info = {};
        mach_msg_type_number_t count = VM_REGION_BASIC_INFO_COUNT_64;
        mach_port_t object = MACH_PORT_NULL;
        kern_return_t status = mach_vm_region(mach_task_self(), &region, &regionSize, VM_REGION_BASIC_INFO_64,
            (vm_region_info_t)&info, &count, &object);
        if (object != MACH_PORT_NULL) mach_port_deallocate(mach_task_self(), object);
        if (status != KERN_SUCCESS || region > page || regionSize < page - region + pageSize) {
            PearANGLETrace( "[PearRuntime][angle-route] region query failed %d\n", status); ready = false; break;
        }
        status = mach_vm_protect(mach_task_self(), page, pageSize, false, info.protection | VM_PROT_WRITE | VM_PROT_COPY);
        if (status != KERN_SUCCESS) {
            PearANGLETrace( "[PearRuntime][angle-route] import page protection failed %d\n", status); ready = false; break;
        }
        pages[page] = info.protection;
    }
    if (ready) for (const auto &patch : patches)
        __atomic_store_n(patch.slot, patch.replacement, __ATOMIC_RELEASE);
    for (const auto &page : pages)
        if (mach_vm_protect(mach_task_self(), page.first, pageSize, false, page.second) != KERN_SUCCESS) ready = false;
    return ready;
}

static void PearRouteImage(const mach_header *header, intptr_t slide) {
    Dl_info image = {};
    if (!dladdr(header, &image) || !image.dli_fname) return;
    const std::string &root = PearAppRoot();
    std::string path = image.dli_fname;
    if (path != root && path.compare(0, root.size() + 1, root + "/") != 0) return;
    const std::string leaf = path.substr(path.find_last_of('/') + 1);
    if (leaf == "libPearRuntime.dylib" || leaf == "libcxxcompat.dylib" || leaf == "libswiftUIKit.dylib" ||
        path.find("MarketplaceKit.framework/") != std::string::npos ||
        leaf == "libEGL.dylib" || leaf == "libGLESv2.dylib" || leaf == "libPearOpenGLES.dylib") return;
    auto fail = [&]() { PearANGLETrace( "[PearRuntime][angle-route] REFUSED image %s; no mixed GL fallback\n", image.dli_fname); _exit(78); };
    if (header->magic != MH_MAGIC_64) { fail(); return; }
    const mach_header_64 *h = (const mach_header_64 *)header;
    if (h->cputype != CPU_TYPE_ARM64 || (h->cpusubtype & ~CPU_SUBTYPE_MASK) == CPU_SUBTYPE_ARM64E) { fail(); return; }
    const uint8_t *cursor = (const uint8_t *)(h + 1), *commandsEnd = cursor + h->sizeofcmds;
    std::vector<const segment_command_64 *> segments;
    std::vector<std::string> dylibs;
    const segment_command_64 *linkedit = nullptr;
    const dyld_info_command *dyld = nullptr;
    const linkedit_data_command *chained = nullptr;
    for (uint32_t index = 0; index < h->ncmds; ++index) {
        if (cursor + sizeof(load_command) > commandsEnd) { fail(); return; }
        const load_command *command = (const load_command *)cursor;
        if (command->cmdsize < sizeof(load_command) || command->cmdsize > (size_t)(commandsEnd - cursor)) { fail(); return; }
        if (command->cmd == LC_SEGMENT_64 && command->cmdsize >= sizeof(segment_command_64)) {
            const auto *s = (const segment_command_64 *)command;
            segments.push_back(s);
            if (!strncmp(s->segname, SEG_LINKEDIT, 16)) linkedit = s;
        }
        if ((command->cmd == LC_DYLD_INFO || command->cmd == LC_DYLD_INFO_ONLY) && command->cmdsize >= sizeof(dyld_info_command))
            dyld = (const dyld_info_command *)command;
        if (command->cmd == LC_DYLD_CHAINED_FIXUPS && command->cmdsize >= sizeof(linkedit_data_command))
            chained = (const linkedit_data_command *)command;
        if ((command->cmd == LC_LOAD_DYLIB || command->cmd == LC_LOAD_WEAK_DYLIB ||
             command->cmd == LC_REEXPORT_DYLIB || command->cmd == LC_LOAD_UPWARD_DYLIB || command->cmd == LC_LAZY_LOAD_DYLIB) &&
            command->cmdsize >= sizeof(dylib_command)) {
            auto *d = (const dylib_command *)command;
            if (d->dylib.name.offset >= command->cmdsize) { fail(); return; }
            const char *name = (const char *)command + d->dylib.name.offset;
            size_t maximum = command->cmdsize - d->dylib.name.offset;
            if (strnlen(name, maximum) == maximum) { fail(); return; }
            dylibs.emplace_back(name);
        }
        cursor += command->cmdsize;
    }
    if (!linkedit || (!chained && !dyld)) {
        PearANGLETrace( "[PearRuntime][angle-route] metadata chained=%d linkedit=%d classic=%d\n", chained != nullptr, linkedit != nullptr, dyld != nullptr);
        fail(); return;
    }
    uintptr_t base = slide + linkedit->vmaddr - linkedit->fileoff;
    std::vector<PearImportPatch> patches;
    if (chained) {
        if (chained->dataoff < linkedit->fileoff ||
            uint64_t(chained->dataoff) + chained->datasize > linkedit->fileoff + linkedit->filesize ||
            !PearReadChainedBindings(image.dli_fname, chained, base, segments, slide, dylibs, patches)) {
            PearANGLETrace("[PearRuntime][angle-route] chained fixups could not be routed\n");
            fail(); return;
        }
        if (!PearApplyPatches(patches)) { fail(); return; }
        PearANGLETrace("[PearRuntime][angle-route] bound %zu imports in %s (chained fixups)\n", patches.size(), image.dli_fname);
        return;
    }
    auto read = [&](uint32_t offset, uint32_t size, bool lazy) {
        if (!size) return true;
        if (offset < linkedit->fileoff || uint64_t(offset) + size > linkedit->fileoff + linkedit->filesize) {
            PearANGLETrace("[PearRuntime][angle-route] invalid bind range %u+%u linkedit=%llu+%llu\n", offset,size,linkedit->fileoff,linkedit->filesize);
            return false;
        }
        return PearReadBindings((const uint8_t *)(base + offset), size, lazy, segments, slide, dylibs, patches);
    };
    if (!read(dyld->bind_off, dyld->bind_size, false)) {
        PearANGLETrace("[PearRuntime][angle-route] regular bindings failed off=%u size=%u linkeditVM=%llx file=%llu slide=%lx\n",dyld->bind_off,dyld->bind_size,linkedit->vmaddr,linkedit->fileoff,(unsigned long)slide);
        fail(); return;
    }
    if (!read(dyld->lazy_bind_off, dyld->lazy_bind_size, true)) { PearANGLETrace("[PearRuntime][angle-route] lazy bindings failed\n"); fail(); return; }
    if (!read(dyld->weak_bind_off, dyld->weak_bind_size, false)) { PearANGLETrace("[PearRuntime][angle-route] weak bindings failed\n"); fail(); return; }
    if (!PearApplyPatches(patches)) { fail(); return; }
    PearANGLETrace( "[PearRuntime][angle-route] bound %zu imports in %s\n", patches.size(), image.dli_fname);
}

__attribute__((constructor)) static void PearEnableGameRouting() {
    const char *backend = getenv("PEAR_GRAPHICS_BACKEND");
    for (uint32_t index = 0; index < _dyld_image_count(); ++index) {
        const char *path = _dyld_get_image_name(index);
        const char *leaf = strrchr(path, '/');
        if (leaf && !strcmp(leaf + 1, "libPearOpenGLES.dylib")) { PearBridgePath() = path; break; }
    }
    // An app that only reaches OpenGL ES at run time (dlopen, environment
    // injection) does not load the bridge at launch. The installer puts it
    // next to the runtime in every compatibility app; arm routing with it so
    // a later dlopen("OpenGLES") gets the bridge, never the system framework.
    std::string runtimeDirectory;
    {
        Dl_info self = {};
        if (dladdr((const void *)&PearEnableGameRouting, &self) && self.dli_fname) {
            runtimeDirectory = self.dli_fname;
            size_t slash = runtimeDirectory.rfind('/');
            runtimeDirectory = slash == std::string::npos ? std::string() : runtimeDirectory.substr(0, slash);
        }
    }
    bool lazy = false;
    if (PearBridgePath().empty() && !runtimeDirectory.empty()) {
        std::string sibling = runtimeDirectory + "/libPearOpenGLES.dylib";
        if (access(sibling.c_str(), R_OK) == 0) { PearBridgePath() = sibling; lazy = !backend; }
    }
    if (backend && strcmp(backend, "angle")) {
        PearANGLETrace("[PearRuntime][angle-route] unsupported backend; only ANGLE is available\n");
        _exit(78);
    }
    if (!backend && PearBridgePath().empty()) return;
    if (!PearRuntimeModuleEnabled("graphics-gl")) {
        if (lazy) {
            // The app does not link GL; a core-only or Metal session must keep running.
            PearANGLETrace("[PearRuntime][angle-route] graphics-gl disabled; GL routing not armed\n");
            return;
        }
        PearANGLETrace("[PearRuntime][angle-route] ANGLE requires the graphics-gl module\n");
        _exit(78);
    }
    @autoreleasepool {
        char executable[PATH_MAX]; uint32_t length = sizeof(executable);
        if (_NSGetExecutablePath(executable, &length) != 0) _exit(78);
        char canonical[PATH_MAX];
        if (!realpath(executable, canonical)) _exit(78);
        PearAppRoot() = canonical;
        size_t app = PearAppRoot().find(".app/");
        if (app != std::string::npos) PearAppRoot().resize(app + 4);
        // ANGLE lives next to the runtime (Frameworks/PearANGLE); the app root
        // layout remains the fallback, PEAR_ANGLE_LIB_DIR the override.
        const char *directory = getenv("PEAR_ANGLE_LIB_DIR");
        NSString *libraries = nil;
        if (directory) {
            libraries = [NSString stringWithUTF8String:directory];
        } else if (!runtimeDirectory.empty() &&
                   access((runtimeDirectory + "/PearANGLE/libEGL.dylib").c_str(), R_OK) == 0) {
            libraries = [[NSString stringWithUTF8String:runtimeDirectory.c_str()] stringByAppendingPathComponent:@"PearANGLE"];
        } else {
            libraries = [[NSString stringWithUTF8String:PearAppRoot().c_str()] stringByAppendingPathComponent:@"Frameworks/PearANGLE"];
        }
        if (lazy) {
            PearLazyANGLEDirectory() = libraries.fileSystemRepresentation;
        } else {
            NSError *error = nil;
            if (!PearANGLEInitialize(libraries, &error)) {
                PearANGLETrace("[PearRuntime][angle-route] initialization failed: %s\n", error.localizedDescription.UTF8String);
                _exit(78);
            }
        }
        gRoutingReady.store(true, std::memory_order_release);
        _dyld_register_func_for_add_image(PearRouteImage);
        auto installGeometry = (void (*)(void))dlsym(RTLD_DEFAULT, "PearANGLEInstallGeometryHooks");
        if (installGeometry) installGeometry();
        PearANGLETrace(lazy ? "[PearRuntime][angle-route] ANGLE routing armed; starts on first GL use\n"
                            : "[PearRuntime][angle-route] ANGLE is the only GL backend\n");
    }
}
