// PearHostLog: printf-style front end for C/Objective-C callers (Catalyst
// modules shared with PearHost). C variadics cannot be defined in stable Rust;
// the logger itself is Rust (pear-core PearHostLogMessage).
#include <stdarg.h>
#include <stdio.h>

#include "PearHost.h"

void PearHostLog(const char *area, const char *fmt, ...) {
    char msg[2048];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof msg, fmt, ap);
    va_end(ap);
    PearHostLogMessage(area, msg);
}
