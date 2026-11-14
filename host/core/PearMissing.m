// Missing-call reporter. Clean-room: written from the public objc runtime API
// (<objc/runtime.h>) and dyld's public <mach-o/dyld.h> / <dlfcn.h>.
#import <Foundation/Foundation.h>
#import <objc/runtime.h>
#import <objc/message.h>
#include <dlfcn.h>
#include <execinfo.h>
#include <signal.h>
#include <mach-o/dyld.h>
#include <os/lock.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "PearHost.h"

static os_unfair_lock gLogLock = OS_UNFAIR_LOCK_INIT;
static FILE *gLogFile;

void PearHostLog(const char *area, const char *fmt, ...) {
    char msg[2048];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof msg, fmt, ap);
    va_end(ap);
    os_unfair_lock_lock(&gLogLock);
    if (!gLogFile) {
        const char *path = getenv("PEAR_LOG_FILE");
        gLogFile = path && *path ? fopen(path, "a") : NULL;
        if (!gLogFile) gLogFile = (FILE *)-1;
    }
    // area NULL: the message carries its own prefix (Catalyst modules).
    const char *pre = area ? "[PearRuntime]" : "";
    char tag[64] = "";
    if (area && *area) snprintf(tag, sizeof tag, "[%s]", area);
    const char *sep = area ? " " : "";
    fprintf(stderr, "%s%s%s%s\n", pre, tag, sep, msg);
    if (gLogFile != (FILE *)-1) {
        fprintf(gLogFile, "%s%s%s%s\n", pre, tag, sep, msg);
        fflush(gLogFile);
    }
    os_unfair_lock_unlock(&gLogLock);
}

// ---------------------------------------------------------------------------
// Report table

typedef struct PearMissingEntry {
    struct PearMissingEntry *next;
    PearMissingKind kind;
    char *key;      // "-[UIView foo:]" / "UIFoo()"
    char *caller;   // first caller, "Gameface+0x1234"
    uint64_t count;
} PearMissingEntry;

static os_unfair_lock gMissingLock = OS_UNFAIR_LOCK_INIT;
static PearMissingEntry *gMissing;
static FILE *gReport;
static int gReportOpened;

static const char *kKindNames[] = {"objc-instance", "objc-class", "c-function", "behavior"};

static void describeCaller(const void *caller, char *out, size_t len) {
    Dl_info info;
    if (caller && dladdr(caller, &info) && info.dli_fname) {
        const char *base = strrchr(info.dli_fname, '/');
        base = base ? base + 1 : info.dli_fname;
        snprintf(out, len, "%s+0x%lx", base, (unsigned long)((const char *)caller - (const char *)info.dli_fbase));
    } else {
        snprintf(out, len, "%p", caller);
    }
}

static void jsonEscape(const char *s, char *out, size_t len) {
    size_t o = 0;
    for (; *s && o + 7 < len; s++) {
        unsigned char c = (unsigned char)*s;
        if (c == '"' || c == '\\') { out[o++] = '\\'; out[o++] = (char)c; }
        else if (c < 0x20) o += (size_t)snprintf(out + o, len - o, "\\u%04x", c);
        else out[o++] = (char)c;
    }
    out[o] = 0;
}

static void writeSummary(void) {
    static int written;
    if (__atomic_exchange_n(&written, 1, __ATOMIC_SEQ_CST)) return;
    os_unfair_lock_lock(&gMissingLock);
    size_t n = 0;
    for (PearMissingEntry *e = gMissing; e; e = e->next) n++;
    if (gReport && gReport != (FILE *)-1) {
        for (PearMissingEntry *e = gMissing; e; e = e->next) {
            char key[1024], caller[256];
            jsonEscape(e->key, key, sizeof key);
            jsonEscape(e->caller, caller, sizeof caller);
            fprintf(gReport, "{\"event\":\"summary\",\"kind\":\"%s\",\"api\":\"%s\",\"count\":%llu,\"first_caller\":\"%s\"}\n",
                    kKindNames[e->kind], key, (unsigned long long)e->count, caller);
        }
        fflush(gReport);
    }
    os_unfair_lock_unlock(&gMissingLock);
    PearHostLog("missing", "%zu distinct missing APIs were called in this run", n);
}

void PearMissingReport(PearMissingKind kind, const char *owner, const char *name, const void *caller) {
    char key[1024];
    switch (kind) {
    case PearMissingObjCInstance: snprintf(key, sizeof key, "-[%s %s]", owner, name); break;
    case PearMissingObjCClass: snprintf(key, sizeof key, "+[%s %s]", owner, name); break;
    case PearMissingCFunction: snprintf(key, sizeof key, "%s()", name); break;
    default: snprintf(key, sizeof key, "%s: %s", owner ? owner : "?", name); break;
    }

    os_unfair_lock_lock(&gMissingLock);
    for (PearMissingEntry *e = gMissing; e; e = e->next) {
        if (e->kind == kind && strcmp(e->key, key) == 0) {
            e->count++;
            os_unfair_lock_unlock(&gMissingLock);
            return;
        }
    }
    char where[256];
    describeCaller(caller, where, sizeof where);
    PearMissingEntry *e = calloc(1, sizeof *e);
    e->kind = kind;
    e->key = strdup(key);
    e->caller = strdup(where);
    e->count = 1;
    e->next = gMissing;
    gMissing = e;
    if (!gReportOpened) {
        gReportOpened = 1;
        const char *path = getenv("PEAR_MISSING_REPORT");
        gReport = path && *path ? fopen(path, "w") : (FILE *)-1;
        if (!gReport) gReport = (FILE *)-1;
        atexit(writeSummary);
        // Harness and user stops arrive as SIGTERM/SIGINT; still write the summary.
        static dispatch_source_t term, intr;
        signal(SIGTERM, SIG_IGN);
        signal(SIGINT, SIG_IGN);
        term = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGTERM, 0, dispatch_get_global_queue(0, 0));
        intr = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGINT, 0, dispatch_get_global_queue(0, 0));
        dispatch_source_set_event_handler(term, ^{ writeSummary(); _exit(0); });
        dispatch_source_set_event_handler(intr, ^{ writeSummary(); _exit(0); });
        dispatch_resume(term);
        dispatch_resume(intr);
    }
    if (gReport != (FILE *)-1) {
        char k[1024], c[256];
        jsonEscape(key, k, sizeof k);
        jsonEscape(where, c, sizeof c);
        fprintf(gReport, "{\"event\":\"first\",\"kind\":\"%s\",\"api\":\"%s\",\"caller\":\"%s\",\"thread_main\":%s}\n",
                kKindNames[kind], k, c, pthread_main_np() ? "true" : "false");
        fflush(gReport);
    }
    os_unfair_lock_unlock(&gMissingLock);

    PearHostLog("missing", "%s %s caller=%s", kKindNames[kind], key, where);
    const char *fatal = getenv("PEAR_MISSING_FATAL");
    if (fatal && *fatal == '1') {
        PearHostLog("missing", "PEAR_MISSING_FATAL=1: aborting");
        abort();
    }
}

// ---------------------------------------------------------------------------
// Objective-C forwarding hook
//
// respondsToSelector: does not consult forwarding, so games that probe for
// optional APIs still see "not implemented". Only an actual message send of an
// unknown selector reaches forwardingTargetForSelector:. There it is reported,
// a zero-returning IMP is added to the receiver's class so later sends are
// cheap, and the message is retried on self.

static const char *pearOwnerName(Class cls) {
    // Name the first class up the chain that PearHost defines (UIView rather
    // than the game's IOSView); fall back to the receiver's own class.
    for (Class c = cls; c; c = class_getSuperclass(c)) {
        Dl_info info;
        if (dladdr((__bridge const void *)c, &info) && info.dli_fname && strstr(info.dli_fname, "UIKit.framework/UIKit"))
            return class_getName(c);
    }
    return class_getName(cls);
}

static BOOL shouldPassThrough(SEL sel) {
    const char *n = sel_getName(sel);
    // Leave runtime and Foundation-internal probes alone.
    return n[0] == '_' || strncmp(n, "CA", 2) == 0;
}

// The frame that sent the message: skip our own frames and the runtime's
// forwarding machinery (CoreFoundation, libobjc).
static const void *messageSender(void) {
    void *frames[12];
    int n = backtrace(frames, 12);
    Dl_info self;
    dladdr((const void *)&messageSender, &self);
    for (int i = 1; i < n; i++) {
        Dl_info info;
        if (!dladdr(frames[i], &info) || !info.dli_fname) return frames[i];
        if (info.dli_fbase == self.dli_fbase) continue;
        if (strstr(info.dli_fname, "CoreFoundation") || strstr(info.dli_fname, "libobjc")) continue;
        return frames[i];
    }
    return NULL;
}

// Receives every unimplemented selector after it has been reported. Its IMPs
// are all pear_missing_zero, so the original send returns zero/nil. The
// receiver's own class is left untouched: respondsToSelector: keeps answering
// NO and every later call is still counted.
@interface PearMissingSink : NSObject
@end
@implementation PearMissingSink
@end

static id pearForwardingTarget(id self, SEL _cmd, SEL sel) {
    if (shouldPassThrough(sel)) return nil;
    Class cls = object_getClass(self);
    BOOL isMeta = class_isMetaClass(cls);
    const char *owner = pearOwnerName(isMeta ? (Class)self : cls);
    PearMissingReport(isMeta ? PearMissingObjCClass : PearMissingObjCInstance, owner, sel_getName(sel), messageSender());
    static PearMissingSink *sink;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ sink = [PearMissingSink new]; });
    class_addMethod([PearMissingSink class], sel, (IMP)pear_missing_zero, "@@:");
    return sink;
}

// Key-value coding on keys we do not implement: report and answer nil instead
// of raising NSUnknownKeyException.
static id pearValueForUndefinedKey(id self, SEL _cmd, NSString *key) {
    PearMissingReport(PearMissingBehavior, pearOwnerName(object_getClass(self)),
                      [NSString stringWithFormat:@"valueForKey:%@", key].UTF8String, messageSender());
    return nil;
}
static void pearSetValueForUndefinedKey(id self, SEL _cmd, id value, NSString *key) {
    PearMissingReport(PearMissingBehavior, pearOwnerName(object_getClass(self)),
                      [NSString stringWithFormat:@"setValue:forKey:%@", key].UTF8String, messageSender());
}

void PearMissingInstallForImage(const void *addressInImage) {
    Dl_info info;
    if (!dladdr(addressInImage, &info) || !info.dli_fname) return;
    unsigned int count = 0;
    const char **names = objc_copyClassNamesForImage(info.dli_fname, &count);
    unsigned int installed = 0;
    for (unsigned int i = 0; i < count; i++) {
        Class cls = objc_getClass(names[i]);
        if (!cls || cls == [PearMissingSink class]) continue;
        SEL sel = @selector(forwardingTargetForSelector:);
        if (class_addMethod(cls, sel, (IMP)pearForwardingTarget, "@@::")) installed++;
        class_addMethod(object_getClass(cls), sel, (IMP)pearForwardingTarget, "@@::");
        class_addMethod(cls, @selector(valueForUndefinedKey:), (IMP)pearValueForUndefinedKey, "@@:@");
        class_addMethod(cls, @selector(setValue:forUndefinedKey:), (IMP)pearSetValueForUndefinedKey, "v@:@@");
    }
    free(names);
    PearHostLog("missing", "reporter installed on %u classes of %s", installed, info.dli_fname);
}
