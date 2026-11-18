// MetalKit.framework ABI shim: the exported MTKView class symbol iOS binaries
// bind to (_OBJC_CLASS_$_MTKView, a UIView subclass). Every method, and the
// image initializer that registers them, is Rust (rust/crates/pear-metalkit).
#import "../uikit/include/PearUIKit.h"

__attribute__((visibility("default")))
@interface MTKView : UIView
@end
@implementation MTKView
@end
