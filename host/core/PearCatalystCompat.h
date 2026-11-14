// Lets platform-neutral Catalyst modules (Foundation/Security only, no UIKit)
// build into PearHost unchanged. Included with -include for those files only.
#pragma once
#import <Foundation/Foundation.h>
#include "PearHost.h"

// Catalyst modules log through PearLog(NSString *format, ...) with their own
// "[PearRuntime][area]" prefix; PearHostLog(NULL, ...) writes it unchanged.
#define PearLog(fmt, ...) PearHostLog(NULL, "%s", [[NSString stringWithFormat:(fmt), ##__VA_ARGS__] UTF8String])
