#include "../core/PearHost.h"

__attribute__((constructor)) static void PearMetalKitLoad(void) {
    PearMissingInstallForImage((const void *)&PearMetalKitLoad);
}
