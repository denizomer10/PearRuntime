// Module registry: decides once per process which PearRuntime modules run.
#import "PearRuntimeModules.h"
#include <stdlib.h>
#include <string.h>
#include <dispatch/dispatch.h>

#ifndef PEAR_RUNTIME_VERSION
#define PEAR_RUNTIME_VERSION "dev"
#endif

static const char *const kModules[] = {"core", "input", "graphics", "device", "storage", "services"};
enum { kModuleCount = sizeof(kModules) / sizeof(kModules[0]) };
static int gEnabled[kModuleCount];
static char gEnabledList[128];

static int ListContains(const char *list, const char *name) {
    if (!list || !*list) return 0;
    size_t length = strlen(name);
    for (const char *cursor = list; *cursor;) {
        while (*cursor == ',' || *cursor == ' ') cursor++;
        const char *end = cursor;
        while (*end && *end != ',' && *end != ' ') end++;
        if ((size_t)(end - cursor) == length && strncmp(cursor, name, length) == 0) return 1;
        cursor = end;
    }
    return 0;
}

static void LoadModules(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        const char *disabled = getenv("PEAR_RUNTIME_DISABLE");
        const char *only = getenv("PEAR_RUNTIME_ONLY");
        gEnabledList[0] = 0;
        for (int i = 0; i < kModuleCount; i++) {
            int enabled = 1;
            if (i > 0) {  // core is always on
                if (only && *only) enabled = ListContains(only, kModules[i]);
                if (ListContains(disabled, kModules[i])) enabled = 0;
            }
            gEnabled[i] = enabled;
            if (enabled) {
                if (gEnabledList[0]) strlcat(gEnabledList, ",", sizeof(gEnabledList));
                strlcat(gEnabledList, kModules[i], sizeof(gEnabledList));
            }
        }
    });
}

int PearRuntimeModuleEnabled(const char *name) {
    LoadModules();
    for (int i = 0; i < kModuleCount; i++) {
        if (strcmp(kModules[i], name) == 0) return gEnabled[i];
    }
    return 1;  // unknown names are not gated
}

const char *PearRuntimeEnabledModules(void) {
    LoadModules();
    return gEnabledList;
}

const char *PearRuntimeVersion(void) { return PEAR_RUNTIME_VERSION; }
