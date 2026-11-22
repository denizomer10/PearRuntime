// Second iOS dylib fixture (libobjc only, no Foundation) exercising M7 step 2:
// custom retain/release, atomic/nonatomic/copy properties, @synchronized,
// associated objects (+ dealloc cleanup), blocks (stack->heap, __block,
// block property), imp_implementationWithBlock + class_addMethod, swizzling,
// reflection counts/offsets/type encodings, and runtime class creation.
// fixture2_objc() returns a bit per passing check (511 when all 9 pass).
#import <objc/NSObject.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import <objc/objc-sync.h>

extern void *malloc(unsigned long size);
extern void free(void *p);
extern int strcmp(const char *a, const char *b);
extern void *_Block_copy(const void *aBlock);
extern void _Block_release(const void *aBlock);
extern id objc_retain(id obj);
extern void objc_release(id obj);

// 1. Custom retain/release
static int custom_retains = 0;
static int custom_releases = 0;

@interface CustomRR : NSObject
@end

@implementation CustomRR
@end

static void *custom_retain_imp(void *self, SEL _cmd) {
    custom_retains++;
    return self;
}
static void custom_release_imp(void *self, SEL _cmd) {
    custom_releases++;
}

// 2. Properties (atomic, nonatomic, copy) + 3. @synchronized + 4. Associated objects
static int copy_calls = 0;
static int assoc_deallocs = 0;

@interface CopyableBox : NSObject
@property (nonatomic, assign) int value;
@end

@implementation CopyableBox
- (id)copyWithZone:(void *)zone {
    copy_calls++;
    CopyableBox *c = [CopyableBox new];
    c.value = self.value + 10;
    return c;
}
- (void)dealloc {
    assoc_deallocs++;
}
@end

@interface PropHolder : NSObject {
@public
    int _rawIvar;
}
@property (atomic, strong) CopyableBox *atomicBox;
@property (nonatomic, copy) CopyableBox *copiedBox;
@property (nonatomic, copy) int (^blockProp)(int);
@end

@implementation PropHolder
- (int)baseMethod:(int)x { return x + 1; }
- (int)altMethod:(int)x { return x * 10; }
@end

static const char kAssocKey = 0;

@protocol SampleProto <NSObject>
- (int)protoMethod:(int)a withB:(int)b;
@end

@interface ProtoImpl : NSObject <SampleProto>
@end

@implementation ProtoImpl
- (int)protoMethod:(int)a withB:(int)b { return a + b; }
@end

static int dyn_imp(id self, SEL _cmd, int v) {
    Ivar iv = class_getInstanceVariable(object_getClass(self), "_dynVal");
    if (!iv) return -1;
    id boxed = object_getIvar(self, iv);
    return boxed ? ((CopyableBox *)boxed).value + v : v;
}

int fixture2_objc(void) {
    int score = 0;

    // Bit 0: custom retain/release
    {
        class_addMethod([CustomRR class], sel_registerName("retain"), (IMP)custom_retain_imp, "@@:");
        class_addMethod([CustomRR class], sel_registerName("release"), (IMP)custom_release_imp, "v@:");
        CustomRR *c = [CustomRR new];
        id r = objc_retain(c);
        objc_release(r);
        if (custom_retains >= 1 && custom_releases >= 1) score |= (1 << 0);
    }

    // Bit 1: properties (atomic, copy)
    {
        PropHolder *h = [PropHolder new];
        CopyableBox *b = [CopyableBox new];
        b.value = 5;
        h.atomicBox = b;
        h.copiedBox = b;
        if (h.atomicBox == b && h.copiedBox != b && h.copiedBox.value == 15 && copy_calls == 1) {
            score |= (1 << 1);
        }
    }

    // Bit 2: @synchronized (nested / re-entrant)
    {
        PropHolder *lockObj = [PropHolder new];
        int inside = 0;
        @synchronized(lockObj) {
            @synchronized(lockObj) {
                inside = 42;
            }
        }
        if (inside == 42) score |= (1 << 2);
    }

    // Bit 3: associated objects + release on dealloc
    {
        int before = assoc_deallocs;
        @autoreleasepool {
            PropHolder *host = [PropHolder new];
            CopyableBox *tag = [CopyableBox new];
            tag.value = 77;
            objc_setAssociatedObject(host, &kAssocKey, tag, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
            CopyableBox *got = objc_getAssociatedObject(host, &kAssocKey);
            if (!got || got.value != 77) before = -999;
        }
        if (assoc_deallocs == before + 1) score |= (1 << 3);
    }

    // Bit 4: blocks (stack -> heap, __block capture, block property)
    {
        __block int counter = 10;
        int (^stackBlk)(int) = ^(int x) {
            counter += x;
            return counter;
        };
        int (^heapBlk)(int) = (__bridge_transfer int (^)(int))_Block_copy((__bridge const void *)stackBlk);
        int r1 = heapBlk(5);
        counter += 100;
        int r2 = heapBlk(2);

        PropHolder *h = [PropHolder new];
        h.blockProp = ^(int k) { return k * 3; };
        int r3 = h.blockProp(7);

        if (r1 == 15 && r2 == 117 && counter == 117 && r3 == 21) {
            score |= (1 << 4);
        }
    }

    // Bit 5: imp_implementationWithBlock + class_addMethod + imp_getBlock / removeBlock
    {
        int (^fn)(id, int) = ^(id selfObj, int n) {
            return n + 100;
        };
        IMP imp = imp_implementationWithBlock(fn);
        SEL s = sel_registerName("dynBlockCall:");
        BOOL added = class_addMethod([PropHolder class], s, imp, "i@:i");
        PropHolder *h = [PropHolder new];
        int res = ((int (*)(id, SEL, int))objc_msgSend)(h, s, 23);
        id gotBlk = imp_getBlock(imp);
        BOOL removed = imp_removeBlock(imp);
        if (added && res == 123 && gotBlk != nil && removed) {
            score |= (1 << 5);
        }
    }

    // Bit 6: method swizzling
    {
        Method m1 = class_getInstanceMethod([PropHolder class], @selector(baseMethod:));
        Method m2 = class_getInstanceMethod([PropHolder class], @selector(altMethod:));
        method_exchangeImplementations(m1, m2);
        PropHolder *h = [PropHolder new];
        int a = [h baseMethod:3];
        int b = [h altMethod:3];
        method_exchangeImplementations(m1, m2);
        if (a == 30 && b == 4) score |= (1 << 6);
    }

    // Bit 7: reflection (methods, type encodings, ivars, properties, protocols)
    {
        unsigned int mc = 0;
        Method *mlist = class_copyMethodList([PropHolder class], &mc);
        Method bm = class_getInstanceMethod([PropHolder class], @selector(baseMethod:));
        unsigned int nargs = method_getNumberOfArguments(bm);
        char *ret = method_copyReturnType(bm);
        char *arg2 = method_copyArgumentType(bm, 2);

        unsigned int ic = 0;
        Ivar *ilist = class_copyIvarList([PropHolder class], &ic);
        Ivar rawIv = class_getInstanceVariable([PropHolder class], "_rawIvar");
        ptrdiff_t off = ivar_getOffset(rawIv);

        unsigned int pc = 0;
        objc_property_t *plist = class_copyPropertyList([PropHolder class], &pc);
        objc_property_t p = class_getProperty([PropHolder class], "atomicBox");
        unsigned int ac = 0;
        objc_property_attribute_t *alist = property_copyAttributeList(p, &ac);

        Protocol *pr = objc_getProtocol("SampleProto");
        struct objc_method_description md = protocol_getMethodDescription(pr, @selector(protoMethod:withB:), YES, YES);

        if (mc >= 4 && nargs == 3 && ret && strcmp(ret, "i") == 0 && arg2 && strcmp(arg2, "i") == 0 &&
            ic >= 3 && off > 0 && pc >= 3 && ac >= 2 && pr != NULL && md.name == @selector(protoMethod:withB:)) {
            score |= (1 << 7);
        }
        free(mlist); free(ret); free(arg2); free(ilist); free(plist); free(alist);
    }

    // Bit 8: runtime class creation (objc_allocateClassPair + class_addIvar + objc_registerClassPair)
    {
        Class dyn = objc_allocateClassPair([NSObject class], "DynRuntimeClass", 0);
        BOOL ivOk = class_addIvar(dyn, "_dynVal", sizeof(id), 3, "@");
        BOOL mOk = class_addMethod(dyn, sel_registerName("compute:"), (IMP)dyn_imp, "i@:i");
        objc_registerClassPair(dyn);
        Class lookedUp = objc_getClass("DynRuntimeClass");
        id inst = [lookedUp new];
        Ivar iv = class_getInstanceVariable(lookedUp, "_dynVal");
        CopyableBox *box = [CopyableBox new];
        box.value = 50;
        object_setIvar(inst, iv, box);
        int out = ((int (*)(id, SEL, int))objc_msgSend)(inst, sel_registerName("compute:"), 7);
        if (dyn && ivOk && mOk && lookedUp == dyn && out == 57) {
            score |= (1 << 8);
        }
    }

    return score;
}
