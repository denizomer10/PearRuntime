#include <dlfcn.h>

__attribute__((visibility("hidden")))
extern const char *pear_guest_framework_path(const char *path);

static void *guest_dlopen(const char *path, int mode) {
    path = pear_guest_framework_path(path);
    // dyld resolves @rpath and @loader_path from the caller's image.
    __attribute__((musttail)) return dlopen(path, mode);
}

__attribute__((used, section("__DATA,__interpose")))
static const struct {
    void *(*replacement)(const char *, int);
    void *(*original)(const char *, int);
} interpose_dlopen = {guest_dlopen, dlopen};
