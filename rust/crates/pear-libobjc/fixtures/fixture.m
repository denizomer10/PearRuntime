// An iOS dylib exercising the Objective-C runtime without Foundation: only
// libobjc's NSObject. Classes and metaclasses, ivars, class and instance
// methods, super calls, a category, a protocol, introspection, ARC
// (strong ivars released by .cxx_destruct), weak references and
// autorelease pools, +load. fixture_objc() returns a bit per passing check
// (255 when all pass).
#import <objc/NSObject.h>
#import <objc/runtime.h>

@protocol Shape <NSObject>
- (int)area;
@end

@interface Rect : NSObject <Shape> {
@public
    int _w, _h;
}
+ (instancetype)rectWithW:(int)w h:(int)h;
@end

static int loads;

@implementation Rect
+ (void)load { loads++; }
+ (instancetype)rectWithW:(int)w h:(int)h {
    Rect *r = [[self alloc] init];
    r->_w = w;
    r->_h = h;
    return r;
}
- (int)area { return _w * _h; }
@end

@interface Square : Rect
@end

@implementation Square
- (int)area { return [super area] + 1; }
@end

@interface Rect (Scaling)
- (int)doubledArea;
@end

@implementation Rect (Scaling)
+ (void)load { loads += 10; }
- (int)doubledArea { return [self area] * 2; }
@end

static int deallocs;

@interface Tracked : NSObject
@property (nonatomic, strong) id child;
@end

@implementation Tracked
- (void)dealloc { deallocs++; }
@end

int fixture_objc(void) {
    int score = 0;
    Rect *r = [Rect rectWithW:3 h:4];
    if ([r area] == 12) score |= 1;
    Square *s = [Square rectWithW:2 h:2];
    if ([s area] == 5) score |= 2;
    if ([s doubledArea] == 10) score |= 4;
    if ([s isKindOfClass:[Rect class]] && ![r isKindOfClass:[Square class]]) score |= 8;
    if ([r conformsToProtocol:@protocol(Shape)] && [r respondsToSelector:@selector(area)] &&
        ![r respondsToSelector:@selector(volume)])
        score |= 16;
    __weak Tracked *weak;
    @autoreleasepool {
        Tracked *t = [Tracked new];
        t.child = [Tracked new];
        weak = t;
        if (weak) score |= 32;
    }
    if (!weak && deallocs == 2) score |= 64;
    if (loads == 11) score |= 128; // +load of the class, then of its category
    return score;
}
