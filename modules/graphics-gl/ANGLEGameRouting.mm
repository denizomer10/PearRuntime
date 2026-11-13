// Original, opt-in import-slot router using public Mach-O bind opcodes.
// Only the main app and its embedded images are modified, never system images.
#import "PearANGLEGameRouting.h"
#import "PearEAGLContext.h"
#include "PearRuntimeModules.h"
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <mach/mach_vm.h>
#include <objc/runtime.h>
#include <dlfcn.h>
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
static void PearRoutedDeleteRenderbuffers(GLsizei count, const GLuint *names) {
    [PearANGLEContext.currentContext forgetDrawableRenderbuffers:names count:count];
    static auto original = (void (*)(GLsizei, const GLuint *))PearANGLEGetProcAddress("glDeleteRenderbuffers");
    original(count, names);
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
    if (!strcmp(name, "glDeleteRenderbuffers") || !strcmp(name, "glDeleteRenderbuffersOES")) return (void *)PearRoutedDeleteRenderbuffers;
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
    bool chained = false;
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
        if (command->cmd == LC_DYLD_CHAINED_FIXUPS) chained = true;
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
    if (chained || !linkedit || !dyld) {
        PearANGLETrace( "[PearRuntime][angle-route] metadata chained=%d linkedit=%d classic=%d\n", chained, linkedit != nullptr, dyld != nullptr);
        fail(); return;
    }
    uintptr_t base = slide + linkedit->vmaddr - linkedit->fileoff;
    std::vector<PearImportPatch> patches;
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
