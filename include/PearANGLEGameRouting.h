#pragma once
#import <Foundation/Foundation.h>
BOOL PearANGLEGameRoutingEnabled(void);
// Returns only ANGLE GL/EAGL replacements. Null for unrelated symbols.
void *PearANGLEGameSymbol(const char *name);
