// PearHost: platform layer for running iOS binaries without /System/iOSSupport.
//
// Clean-room rule for everything under host/: implementations come from public
// SDK headers, Apple's public documentation, open-source Apple code (with its
// license kept) and black-box tests. Never from disassembly of Apple binaries.
// See docs/independent-runtime.md.
#pragma once

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

#define PEAR_EXPORT __attribute__((visibility("default")))

// Log with the "[PearRuntime][<area>]" prefix PearDebug parses. Goes to stderr
// and, when PEAR_LOG_FILE is set, is appended to that file.
PEAR_EXPORT void PearHostLog(const char *area, const char *fmt, ...) __attribute__((format(printf, 2, 3)));

// Missing-call reporting. Every call that reaches an API PearHost does not
// implement yet is reported once with its caller (image+offset), then counted.
// PEAR_MISSING_REPORT=<path> also writes one JSON line per first hit and a
// summary at exit. PEAR_MISSING_FATAL=1 aborts on the first missing call.
typedef enum {
    PearMissingObjCInstance = 0,
    PearMissingObjCClass = 1,
    PearMissingCFunction = 2,
    PearMissingBehavior = 3,  // implemented, but a requested behavior is not
} PearMissingKind;

PEAR_EXPORT void PearMissingReport(PearMissingKind kind, const char *owner, const char *name, const void *caller);

// Install forwarding hooks on every Objective-C class defined in the image that
// contains `addressInImage`, so unimplemented selectors are reported and
// answered with zero instead of raising.
PEAR_EXPORT void PearMissingInstallForImage(const void *addressInImage);

// Returns 0 in x0/x1 and 0.0 in d0-d3. Used as the IMP of unimplemented
// selectors and as the tail of generated C function stubs.
PEAR_EXPORT void pear_missing_zero(void);

#ifdef __cplusplus
}
#endif
