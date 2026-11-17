#import "../uikit/include/PearUIKit.h"
#include "../core/PearHost.h"

#define PEAR_MTK_EXPORT __attribute__((visibility("default")))

PEAR_MTK_EXPORT
@interface MTKView : UIView
@end
@implementation MTKView
@end

extern void pear_metalkit_register_classes(void);

__attribute__((constructor)) static void PearMetalKitLoad(void) {
    pear_metalkit_register_classes();
    PearMissingInstallForImage((const void *)&PearMetalKitLoad);
}
