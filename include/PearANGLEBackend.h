#pragma once
#import <Foundation/Foundation.h>
#include <EGL/egl.h>

// Original loader boundary for the upcoming EAGL adapter. No system-EAGL
// fallback: callers must handle initialization failure explicitly.
BOOL PearANGLEInitialize(NSString *directory, NSError *__autoreleasing *error);
void *PearANGLEGetProcAddress(const char *name);
EGLDisplay PearANGLEDisplay(void);
void PearANGLETrace(const char *format, ...) __attribute__((format(printf, 1, 2)));
