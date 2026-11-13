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

static std::atomic<bool> gRoutingReady{false};
static std::string &PearAppRoot() { static std::string root; return root; }
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
static void *PearRoutedBundleProc(CFBundleRef bundle, CFStringRef name) {
    char symbol[256];
    if (name && CFStringGetCString(name, symbol, sizeof(symbol), kCFStringEncodingUTF8)) {
        void *replacement = PearANGLEGameSymbol(symbol);
        if (replacement || PearIsGLName(symbol)) return replacement;
    }
    return CFBundleGetFunctionPointerForName(bundle, name);
}

void *PearANGLEGameSymbol(const char *name) {
    if (!name || !PearANGLEGameRoutingEnabled()) return nullptr;
    if (!strcmp(name, "OBJC_CLASS_$_EAGLContext")) return (__bridge void *)PearEAGLContext.class;
    if (!strcmp(name, "OBJC_CLASS_$_EAGLSharegroup")) return (__bridge void *)PearEAGLSharegroup.class;
    if (!strcmp(name, "OBJC_METACLASS_$_EAGLContext")) return (__bridge void *)object_getClass(PearEAGLContext.class);
    if (!strcmp(name, "OBJC_METACLASS_$_EAGLSharegroup")) return (__bridge void *)object_getClass(PearEAGLSharegroup.class);
    if (!strcmp(name, "glDeleteRenderbuffers") || !strcmp(name, "glDeleteRenderbuffersOES")) return (void *)PearRoutedDeleteRenderbuffers;
    if (!strcmp(name, "EAGLGetVersion")) return (void *)PearRoutedVersion;
    if (!strcmp(name, "EAGLGetProcAddress") || !strcmp(name, "eglGetProcAddress")) return (void *)PearRoutedProc;
    if (!strcmp(name, "NSClassFromString")) return (void *)PearRoutedNSClass;
    if (!strcmp(name, "objc_getClass")) return (void *)PearRoutedGetClass;
    if (!strcmp(name, "objc_lookUpClass")) return (void *)PearRoutedLookupClass;
    if (!strcmp(name, "objc_getMetaClass")) return (void *)PearRoutedMetaClass;
    if (!strcmp(name, "dlsym")) return (void *)PearRoutedDlsym;
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
        !strcmp(name, "dlsym") || !strcmp(name, "CFBundleGetFunctionPointerForName");
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
    std::vector<PearImportPatch> &patches) {
    const uint8_t *p = begin, *end = begin + size;
    unsigned segment = UINT_MAX, type = BIND_TYPE_POINTER;
    uint64_t offset = 0;
    bool hasAddend = false;
    const char *symbol = nullptr;
    auto bind = [&]() -> bool {
        if (!PearRoutingCandidate(symbol)) return true;
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
                segment = UINT_MAX; offset = 0; symbol = nullptr; type = BIND_TYPE_POINTER; hasAddend = false;
                break;
            case BIND_OPCODE_SET_DYLIB_ORDINAL_IMM: case BIND_OPCODE_SET_DYLIB_SPECIAL_IMM: break;
            case BIND_OPCODE_SET_DYLIB_ORDINAL_ULEB:
                if (!PearReadULEB(p, end, value)) return false;
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
        leaf == "libEGL.dylib" || leaf == "libGLESv2.dylib") return;
    auto fail = [&]() { PearANGLETrace( "[PearRuntime][angle-route] REFUSED image %s; no mixed GL fallback\n", image.dli_fname); _exit(78); };
    if (header->magic != MH_MAGIC_64) { fail(); return; }
    const mach_header_64 *h = (const mach_header_64 *)header;
    if (h->cputype != CPU_TYPE_ARM64 || (h->cpusubtype & ~CPU_SUBTYPE_MASK) == CPU_SUBTYPE_ARM64E) { fail(); return; }
    const uint8_t *cursor = (const uint8_t *)(h + 1), *commandsEnd = cursor + h->sizeofcmds;
    std::vector<const segment_command_64 *> segments;
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
        return PearReadBindings((const uint8_t *)(base + offset), size, lazy, segments, slide, patches);
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
    if (!backend || strcmp(backend, "angle")) return;
    if (!PearRuntimeModuleEnabled("graphics-gl")) {
        PearANGLETrace("[PearRuntime][angle-route] ANGLE requires the graphics-gl module\n");
        _exit(78);
    }
    @autoreleasepool {
        const char *directory = getenv("PEAR_ANGLE_LIB_DIR");
        NSError *error = nil;
        if (!directory || !PearANGLEInitialize([NSString stringWithUTF8String:directory], &error)) {
            PearANGLETrace( "[PearRuntime][angle-route] initialization failed: %s\n", error.localizedDescription.UTF8String ?: "PEAR_ANGLE_LIB_DIR missing");
            _exit(78);
        }
        char executable[PATH_MAX]; uint32_t length = sizeof(executable);
        if (_NSGetExecutablePath(executable, &length) != 0) _exit(78);
        char canonical[PATH_MAX];
        if (!realpath(executable, canonical)) _exit(78);
        PearAppRoot() = canonical;
        size_t app = PearAppRoot().find(".app/");
        if (app != std::string::npos) PearAppRoot().resize(app + 4);
        gRoutingReady.store(true, std::memory_order_release);
        _dyld_register_func_for_add_image(PearRouteImage);
        auto installGeometry = (void (*)(void))dlsym(RTLD_DEFAULT, "PearANGLEInstallGeometryHooks");
        if (installGeometry) installGeometry();
        PearANGLETrace( "[PearRuntime][angle-route] enabled explicit ANGLE routing\n");
    }
}
