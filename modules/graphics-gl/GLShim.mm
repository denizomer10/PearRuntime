// GLShim.mm: geometry and touch bridge for the source-built ANGLE renderer.
// System OpenGLES/EAGL implementations are never queried or used as fallback.
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#import <string.h>
#import <math.h>
#import <atomic>
#import "PearRuntime.h"
#import "PearTouchGeometry.h"
#import "PearANGLEGameRouting.h"
#import "PearEAGLContext.h"
#import "PearGLTouch.h"

extern void PearRecordFramePresented(void);

static BOOL PearGLDebug(void);

static CGSize PearDeclaredDrawableSize(CGSize size) {
    static BOOL landscape, portrait;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSDictionary *info = NSBundle.mainBundle.infoDictionary;
        // A conflicting phone/tablet declaration is deliberately ambiguous.
        for (NSString *key in @[@"UISupportedInterfaceOrientations", @"UISupportedInterfaceOrientations~ipad"]) {
            id orientations = info[key];
            if (![orientations isKindOfClass:NSArray.class]) continue;
            for (id value in orientations) {
                if (![value isKindOfClass:NSString.class]) continue;
                if ([value isEqualToString:@"UIInterfaceOrientationLandscapeLeft"] ||
                    [value isEqualToString:@"UIInterfaceOrientationLandscapeRight"]) landscape = YES;
                if ([value isEqualToString:@"UIInterfaceOrientationPortrait"] ||
                    [value isEqualToString:@"UIInterfaceOrientationPortraitUpsideDown"]) portrait = YES;
            }
        }
    });
    return PearOrientDrawableSize(size, landscape, portrait);
}

static void *PearGLProc(const char *name) { return PearANGLEGetProcAddress(name); }
static CGSize gGLDrawableSize = { 0, 0 };
/// The size the engine itself created its drawable at. This is the coordinate
/// space its hit-testing uses. It must not follow window resizes: the fit
/// transform is visual only, and allowing the view's bounds to grow with the
/// window makes every touch land in the wrong place (most visible in full
/// screen).
static CGSize gNativeDrawableSize = { 0, 0 };
static __weak CALayer *gDrawableLayer;
static int gLastCropViewport[4] = { -1, -1, -1, -1 };
static int gLastPresentViewport[4] = { -1, -1, -1, -1 };
static uint64_t gPresentCount = 0;
static BOOL gCropApplied = NO;

static void PearFitDrawable(CALayer *layer);

/// Shows only the region the engine actually rendered, stretched to the
/// drawable. Legacy titles keep a design-resolution viewport in the corner of
/// a window-sized framebuffer; cropping to that viewport makes the visible
/// image fill the window, and the same scale aligns touch with what is shown.
/// Opt out with PEAR_NO_GL_VIEWPORT_FIT=1.
/// Bumped (from any thread) whenever the window geometry may have changed.
/// The present path re-fits only after a bump, and keeps re-fitting for a
/// short settling period because UIKit applies its new window size a little
/// after the AppKit resize notification. Without this, every presented frame
/// queued a main-thread block, which competes with the game's own main-thread
/// work (Unity and most iOS engines render on the main thread) and shows up as
/// frame-time spikes.
static std::atomic<uint32_t> gGeometryGeneration{1};
static uint32_t gSeenGeometryGeneration = 0;
static int gFitSettleFrames = 0;

static void PearGLGeometryChanged(void) {
    gGeometryGeneration.fetch_add(1, std::memory_order_relaxed);
}

static void PearObserveGLGeometry(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        for (NSString *name in @[ @"NSWindowDidResizeNotification",
                                  @"NSWindowDidEnterFullScreenNotification",
                                  @"NSWindowDidExitFullScreenNotification",
                                  @"NSWindowDidChangeBackingPropertiesNotification",
                                  UIWindowDidBecomeKeyNotification,
                                  UISceneDidActivateNotification ]) {
            [NSNotificationCenter.defaultCenter addObserverForName:name object:nil queue:nil
                usingBlock:^(__unused NSNotification *note) { PearGLGeometryChanged(); }];
        }
    });
}

static void PearUpdateDrawableCrop(void) {
    static const BOOL disabled = getenv("PEAR_NO_GL_VIEWPORT_FIT") != NULL;
    if (disabled) return;
    CALayer *layer = gDrawableLayer;
    if (!layer) return;

    gPresentCount++;
    if (PearGLDebug() && gPresentCount % 120 == 0) {
        typedef void (*PearGetIntegerv)(int, int *);
        typedef void (*PearReadPixels)(int, int, int, int, unsigned, unsigned, void *);
        static PearGetIntegerv getIntegervDebug;
        static PearReadPixels readPixelsDebug;
        if (!getIntegervDebug) getIntegervDebug = (PearGetIntegerv)PearGLProc("glGetIntegerv");
        if (!readPixelsDebug) readPixelsDebug = (PearReadPixels)PearGLProc("glReadPixels");
        int vp[4] = { 0, 0, 0, 0 };
        unsigned char center[4] = { 0, 0, 0, 0 };
        unsigned char corner[4] = { 0, 0, 0, 0 };
        if (getIntegervDebug && readPixelsDebug) {
            getIntegervDebug(0x0BA2 /* GL_VIEWPORT */, vp);
            if (vp[2] > 0 && vp[3] > 0) {
                readPixelsDebug(vp[0] + vp[2] / 2, vp[1] + vp[3] / 2, 1, 1,
                                0x1908 /* GL_RGBA */, 0x1401 /* GL_UNSIGNED_BYTE */, center);
                readPixelsDebug(vp[0] + 4, vp[1] + vp[3] - 4, 1, 1,
                                0x1908, 0x1401, corner);
            }
        }
        PearLog(@"[PearRuntime][gl] present #%llu center=%02x%02x%02x%02x corner=%02x%02x%02x%02x",
                (unsigned long long)gPresentCount,
                center[0], center[1], center[2], center[3],
                corner[0], corner[1], corner[2], corner[3]);
    }

    // The window can be resized (or rotated) after the renderbuffer was
    // created. Re-fit after every geometry change (and briefly afterwards, while
    // UIKit settles) so the visible scale follows the window; a stale scale
    // makes taps land away from the controls. A slow periodic re-fit covers
    // changes that arrive without a notification.
    uint32_t generation = gGeometryGeneration.load(std::memory_order_relaxed);
    if (generation != gSeenGeometryGeneration) {
        gSeenGeometryGeneration = generation;
        gFitSettleFrames = 45;
    }
    if (gFitSettleFrames > 0) {
        gFitSettleFrames--;
        PearFitDrawable(layer);
    } else if (gPresentCount % 120 == 0) {
        PearFitDrawable(layer);
    }

    typedef void (*PearGetIntegervFn)(int pname, int *params);
    static PearGetIntegervFn getIntegerv;
    if (getIntegerv == NULL) {
        getIntegerv = (PearGetIntegervFn)PearGLProc("glGetIntegerv");
    }
    if (getIntegerv == NULL) return;

    int viewport[4] = { 0, 0, 0, 0 };
    getIntegerv(0x0BA2 /* GL_VIEWPORT */, viewport);
    if (viewport[2] <= 0 || viewport[3] <= 0) return;

    int drawableWidth = (int)llround(gGLDrawableSize.width);
    int drawableHeight = (int)llround(gGLDrawableSize.height);
    if (drawableWidth <= 0 || drawableHeight <= 0) return;

    if (PearGLDebug() &&
        (viewport[0] != gLastPresentViewport[0] ||
         viewport[1] != gLastPresentViewport[1] ||
         viewport[2] != gLastPresentViewport[2] ||
         viewport[3] != gLastPresentViewport[3])) {
        memcpy(gLastPresentViewport, viewport, sizeof(viewport));
        PearLog(@"[PearRuntime][gl] present viewport=(%d,%d,%d,%d) drawable=%dx%d",
                viewport[0], viewport[1], viewport[2], viewport[3],
                drawableWidth, drawableHeight);
    }

    BOOL small = drawableWidth >= viewport[2] * 1.4 &&
                 drawableHeight >= viewport[3] * 1.4;

    if (!small) {
        if (!gCropApplied) return;
        gCropApplied = NO;
        gLastCropViewport[0] = -1;
        dispatch_async(dispatch_get_main_queue(), ^{
            CALayer *current = gDrawableLayer;
            if (!current) return;
            current.contentsRect = CGRectMake(0, 0, 1, 1);
        });
        return;
    }

    if (viewport[0] == gLastCropViewport[0] &&
        viewport[1] == gLastCropViewport[1] &&
        viewport[2] == gLastCropViewport[2] &&
        viewport[3] == gLastCropViewport[3]) return;

    memcpy(gLastCropViewport, viewport, sizeof(viewport));
    gCropApplied = YES;

    int top = drawableHeight - (viewport[1] + viewport[3]);
    CGRect rect = CGRectMake((CGFloat)viewport[0] / drawableWidth,
                             (CGFloat)top / drawableHeight,
                             (CGFloat)viewport[2] / drawableWidth,
                             (CGFloat)viewport[3] / drawableHeight);

    if (PearGLDebug()) {
        PearLog(@"[PearRuntime][gl] crop %dx%d at (%d,%d) from drawable %dx%d",
                viewport[2], viewport[3], viewport[0], viewport[1],
                drawableWidth, drawableHeight);
    }

    dispatch_async(dispatch_get_main_queue(), ^{
        CALayer *current = gDrawableLayer;
        if (!current) return;
        current.contentsRect = rect;
        current.contentsGravity = kCAGravityResize;
        current.magnificationFilter = kCAFilterLinear;
    });
}

extern "C" void *EAGLGetProcAddress(const char *name) {
    return PearANGLEGameSymbol(name);
}

#pragma mark - Drawable fit

static BOOL PearGLDebug(void) {
    static const BOOL debug = getenv("PEAR_DEBUG_GL") != NULL;
    return debug;
}

static UIWindow *PearGLKeyWindow(void);

static UIWindow *PearGLWindowForLayer(CALayer *layer) {
    id delegate = layer.delegate;
    if ([delegate isKindOfClass:UIView.class]) return ((UIView *)delegate).window;
    return nil;
}

static UIWindow *PearGLKeyWindow(void) {
    return PearPrimaryWindow();
}

/// Whether a large (maximised / full-screen) window keeps the drawable at the
/// size the engine first created and only scales it up visually.
///
/// That freeze exists for legacy GL titles (the legacy window profile), which
/// lay their controls out once and mis-place every touch if the view grows.
/// Modern engines re-allocate their renderbuffer and re-layout on resize; for
/// them the freeze produced a pillar-boxed, upscaled (blurry) image in a
/// maximised or full-screen window. They now follow the window at its real
/// pixel size. PEAR_GL_FREEZE_LARGE_WINDOW=1/0 forces either behaviour.
static BOOL PearGLFreezesLargeWindows(void) {
    static BOOL freeze;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        const char *forced = getenv("PEAR_GL_FREEZE_LARGE_WINDOW");
        if (forced && *forced) {
            freeze = strcmp(forced, "0") != 0;
        } else {
            freeze = getenv("PEAR_LEGACY_ATTACH_LAYER") != NULL;
        }
        PearLog(@"[PearRuntime][gl] large windows: %s",
                freeze ? "keep the launch drawable (legacy profile)" : "re-render at window size");
    });
    return freeze;
}

static CGSize gLastFitWindowSize = { 0, 0 };
static CGSize gLastFitLayerSize = { 0, 0 };
static CGFloat gLastFitScale = 0;
static BOOL gFitApplied = NO;

/// The window the app laid its controls out for when it created the drawable.
/// Legacy renderers convert touches with the window's point space, so after a
/// Mac window resize (full screen, zoom) the touch must be reported in this
/// reference space or every control is offset by the difference in scale. The
/// visible image is fitted by the layer transform below.
static CGSize gReferenceWindowSize = { 0, 0 };
/// A window resize animates through intermediate sizes. Recording one of those
/// as the reference would leave every touch slightly offset, so the size only
/// becomes the reference after it has stayed put briefly.
static CGSize gPendingWindowSize = { 0, 0 };
static CFAbsoluteTime gPendingWindowSince = 0;
static BOOL gTouchRemapInstalled = NO;

/// Legacy GL titles ask for `locationInView:window` (or `nil`, which means the
/// window) and divide that point by their design scale. When the Mac window is
/// resized, the view is fitted but that window point is not, so the engine sees
/// a shifted coordinate. Rewrite only the window-space answer; the touch's
/// location in the GL view itself is left untouched.
CGPoint PearGLMapTouchPoint(UITouch *touch, UIView *view, CGPoint point) {
    if (!gFitApplied || !NSThread.isMainThread) return point;
    CALayer *drawable = gDrawableLayer;
    id delegate = drawable.delegate;
    UIView *renderView = [delegate isKindOfClass:UIView.class] ? delegate : nil;
    UIView *target = touch.view;
    if (!renderView || !target || ![target isDescendantOfView:renderView]) return point;
    UIWindow *window = renderView.window ?: PearGLKeyWindow();
    if (!window) return point;
    BOOL windowSpace = !view || [view isKindOfClass:UIWindow.class] ||
        (view.layer != drawable && view.window == window && CGSizeEqualToSize(view.bounds.size, window.bounds.size));
    if (!windowSpace) return point;
    // Root-view coordinates may include an offset inside the window.
    CGPoint windowPoint = view && view != window ? [view.layer convertPoint:point toLayer:window.layer] : point;
    CGPoint remapped = [drawable convertPoint:windowPoint fromLayer:window.layer];
    if (getenv("PEAR_DEBUG_TOUCH") && touch.phase != UITouchPhaseMoved &&
        (fabs(remapped.x - point.x) > 0.5 || fabs(remapped.y - point.y) > 0.5))
        PearLog(@"[PearRuntime][touch] mapped %@: %@ -> %@", NSStringFromClass(view.class) ?: @"nil",
                NSStringFromCGPoint(point), NSStringFromCGPoint(remapped));
    return remapped;
}

static void PearInstallTouchRemap(void) {
    if (gTouchRemapInstalled) return;
    Class touchClass = NSClassFromString(@"UITouch");
    if (!touchClass) return;
    gTouchRemapInstalled = YES;
    for (NSString *name in @[@"locationInView:", @"previousLocationInView:"]) {
        SEL selector = NSSelectorFromString(name);
        Method method = class_getInstanceMethod(touchClass, selector);
        if (!method) continue;
        IMP original = method_getImplementation(method);
        IMP replacement = imp_implementationWithBlock(^CGPoint(UITouch *touch, UIView *view) {
            CGPoint point = ((CGPoint (*)(id, SEL, UIView *))original)(touch, selector, view);
            return PearGLMapTouchPoint(touch, view, point);
        });
        if (!class_addMethod(touchClass, selector, replacement, method_getTypeEncoding(method)))
            method_setImplementation(method, replacement);
    }
    PearLog(@"[PearRuntime][touch] current/previous window-space touch mapping installed");
}

/// Fits the drawable into the key window while keeping the engine's coordinate
/// space intact. The visual size is changed with a layer transform; the layer's
/// bounds stay at the size the engine allocated its renderbuffer at.
///
/// This distinction matters on a window resize (full screen in particular):
/// UIKit autoresizing sets the view's *frame*, and with a transform applied the
/// view then reports `bounds = frame / scale`. The engine keeps hit-testing in
/// its original space while touches start arriving in the inflated one, so
/// every control is offset. Restoring the native bounds on the main queue keeps
/// `locationInView:` in the space the engine was built for, while the transform
/// still makes the frame fill the window.
static void PearFitDrawable(CALayer *layer) {
    static const BOOL disabled = getenv("PEAR_NO_GL_FIT") != NULL;
    if (!layer || disabled) return;
    CGSize native = gNativeDrawableSize;
    if (native.width < 1 || native.height < 1) return;

    dispatch_async(dispatch_get_main_queue(), ^{
        CALayer *current = gDrawableLayer;
        UIWindow *window = PearGLKeyWindow();
        if (!current || !window) return;

        CGRect windowBounds = window.bounds;
        if (windowBounds.size.width < 1 || windowBounds.size.height < 1) return;
        CALayer *parent = current.superlayer;
        if (!parent) return;
        CGRect container = [parent convertRect:windowBounds fromLayer:window.layer];
        if (CGRectIsEmpty(container) || CGRectIsInfinite(container)) return;
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        @try {

        // Remember the last window the app was actually laid out in. A window
        // that is much larger than the drawable in both axes is the full-screen
        // (or otherwise maximised) form and must not become the reference.
        BOOL fullScreenLike = PearGLFreezesLargeWindows() &&
                              windowBounds.size.width > native.width * 1.4 &&
                              windowBounds.size.height > native.height * 1.4;
        if (!fullScreenLike) {
            if (gReferenceWindowSize.width < 1) gReferenceWindowSize = windowBounds.size;
            if (!CGSizeEqualToSize(gPendingWindowSize, windowBounds.size)) {
                gPendingWindowSize = windowBounds.size;
                gPendingWindowSince = CFAbsoluteTimeGetCurrent();
            } else if (CFAbsoluteTimeGetCurrent() - gPendingWindowSince > 0.4) {
                gReferenceWindowSize = windowBounds.size;
            }
        }

        // In the windowed form the app keeps following the real window, as it
        // did before the fit. Only the full-screen form is frozen at the last
        // windowed geometry: the app laid its controls out there, and growing
        // UIScreen to the display makes those controls move while the drawable
        // stays native.
        if (fullScreenLike && gReferenceWindowSize.width > 0) {
            PearSetScreenOverride(gReferenceWindowSize);
        } else {
            PearClearScreenOverride();
        }

        // Undo any resize the app (or autoresizing) applied to the view once the
        // window is in its full-screen form. The renderbuffer still has the
        // native size, so the engine's coordinates must stay native too. In the
        // windowed form the app keeps its normal resize behaviour.
        CGSize basis = native;
        if (fullScreenLike) {
            CGRect bounds = current.bounds;
            if (fabs(bounds.size.width - native.width) > 0.5 ||
                fabs(bounds.size.height - native.height) > 0.5) {
                current.bounds = CGRectMake(0, 0, native.width, native.height);
            }
        } else {
            // UIKit autoresizing changes bounds through the existing fit
            // transform. Those bounds are not a new engine coordinate space:
            // only a successful renderbuffer allocation can establish one.
            CGFloat pixelScale = current.contentsScale > 0 ? current.contentsScale : 1;
            basis = CGSizeMake(gGLDrawableSize.width / pixelScale,
                               gGLDrawableSize.height / pixelScale);
            if (basis.width < 1 || basis.height < 1) return;
            if (!CGSizeEqualToSize(current.bounds.size, basis))
                current.bounds = CGRectMake(0, 0, basis.width, basis.height);

            // Small fixed drawables still need centering and enlargement.
            // Removing the fit when the window grows leaves the game in the
            // old corner and changes its input coordinate space mid-resize.

        }

        CGRect fitted = PearAspectFitRect(basis, container);
        CGFloat scale = fitted.size.width / basis.width;
        if (scale <= 0) return;

        // Re-apply the position even when the scale is unchanged: a window
        // resize can move the view before the next present.
        current.position = CGPointMake(fitted.origin.x + current.anchorPoint.x * fitted.size.width,
                                       fitted.origin.y + current.anchorPoint.y * fitted.size.height);

        if (gFitApplied &&
            CGSizeEqualToSize(gLastFitWindowSize, windowBounds.size) &&
            CGSizeEqualToSize(gLastFitLayerSize, basis) &&
            fabs(gLastFitScale - scale) < 0.001 &&
            fabs(current.transform.m11 - scale) < 0.001 &&
            fabs(current.transform.m22 - scale) < 0.001) {
            return;
        }

        gFitApplied = YES;
        gLastFitWindowSize = windowBounds.size;
        gLastFitLayerSize = basis;
        gLastFitScale = scale;

        current.transform = CATransform3DMakeScale(scale, scale, 1);
        if (current.superlayer) current.superlayer.backgroundColor = UIColor.blackColor.CGColor;
        window.backgroundColor = UIColor.blackColor;

        if (PearGLDebug()) {
            PearLog(@"[PearRuntime][gl] fit drawable %@ x%.3f into window %@",
                    NSStringFromCGSize(native), scale, NSStringFromCGRect(windowBounds));
        }
        } @finally {
            [CATransaction commit];
        }
    });
}



#pragma mark - Legacy touch diagnostics

// The opt-in router installs these before the game allocates its first drawable.
static void PearInstallEAGLHook(void);
extern "C" void PearANGLEInstallGeometryHooks(void) { PearInstallEAGLHook(); }

/// Logs touches the legacy GL view receives, before the engine converts them.
/// The stream is what tells a delivery problem from a coordinate-scale problem.
static void PearInstallEAGLTouchLogging(void) {
    static BOOL installed = NO;
    if (installed) return;
    Class viewClass = NSClassFromString(@"EAGLView");
    if (!viewClass) return;
    installed = YES;

    SEL selectors[] = {
        @selector(touchesBegan:withEvent:),
        @selector(touchesEnded:withEvent:)
    };
    for (int i = 0; i < 2; i++) {
        SEL selector = selectors[i];
        Method method = class_getInstanceMethod(viewClass, selector);
        if (!method) continue;
        IMP original = method_getImplementation(method);
        const char *types = method_getTypeEncoding(method);
        IMP replacement = imp_implementationWithBlock(^(id self, NSSet *touches, UIEvent *event) {
            UITouch *touch = touches.anyObject;
            CGPoint point = [touch locationInView:self];
            CGRect bounds = [self bounds];
            PearLog(@"[PearRuntime][touch] %@ point=(%.1f,%.1f) bounds=%@ scale=%.2f",
                    NSStringFromSelector(selector), point.x, point.y,
                    NSStringFromCGRect(bounds), [self contentScaleFactor]);
            ((void (*)(id, SEL, NSSet *, UIEvent *))original)(self, selector, touches, event);
        });
        if (!class_addMethod(viewClass, selector, replacement, types)) {
            method_setImplementation(method, replacement);
        }
    }
    PearLog(@"[PearRuntime][touch] EAGLView touch logging installed");
}

#pragma mark - EAGL hook

// Most EAGL apps size their renderbuffer with
// `-[EAGLContext renderbufferStorage:fromDrawable:]`, which takes the size from
// the drawable layer. Fixing the layer before the call makes the renderbuffer
// match the window; logging it shows the surface the app actually renders at.
static void PearInstallEAGLHook(void) {
    static BOOL installed = NO;
    if (installed) return;
    if (!PearANGLEGameRoutingEnabled()) return;
    Class context = PearEAGLContext.class;
    if (!context) return;
    SEL selector = NSSelectorFromString(@"renderbufferStorage:fromDrawable:");
    Method method = class_getInstanceMethod(context, selector);
    if (!method) return;
    installed = YES;

    IMP original = method_getImplementation(method);
    method_setImplementation(method, imp_implementationWithBlock(^BOOL(id self, NSUInteger target, id drawable) {
        if ([drawable isKindOfClass:CALayer.class]) {
            CALayer *layer = (CALayer *)drawable;
            CGSize oriented = PearDeclaredDrawableSize(layer.bounds.size);
            if ([NSThread isMainThread] && !CGSizeEqualToSize(oriented, layer.bounds.size)) {
                if (PearGLDebug()) PearLog(@"[PearRuntime][gl] corrected fixed-orientation drawable %@ -> %@", NSStringFromCGSize(layer.bounds.size), NSStringFromCGSize(oriented));
                CGRect bounds = layer.bounds;
                bounds.size = oriented;
                layer.bounds = bounds;
            }
            if (PearGLDebug()) {
                PearLog(@"[PearRuntime][gl] renderbufferStorage fromDrawable bounds=%@ pos=%@ contentsScale=%.2f",
                        NSStringFromCGRect(layer.bounds), NSStringFromCGPoint(layer.position), layer.contentsScale);
            }
            if (gNativeDrawableSize.width < 1 || gNativeDrawableSize.height < 1) {
                gNativeDrawableSize = layer.bounds.size;
                PearInstallTouchRemap();
            } else if ([NSThread isMainThread] &&
                       (fabs(layer.bounds.size.width - gNativeDrawableSize.width) > 0.5 ||
                        fabs(layer.bounds.size.height - gNativeDrawableSize.height) > 0.5)) {
                // The app resized its view for a full-screen window and is
                // allocating a new drawable at the inflated size. Keep the
                // renderbuffer and the touch space at the engine's native size;
                // the transform handles the visible size. A normal windowed
                // resize is left alone.
                UIWindow *window = PearGLWindowForLayer(layer);
                BOOL fullScreenLike = window && PearGLFreezesLargeWindows() &&
                    layer.bounds.size.width > gNativeDrawableSize.width * 1.4 &&
                    layer.bounds.size.height > gNativeDrawableSize.height * 1.4;
                if (fullScreenLike) {
                    layer.bounds = CGRectMake(0, 0, gNativeDrawableSize.width,
                                              gNativeDrawableSize.height);
                }
            }
            gGLDrawableSize = CGSizeMake(layer.bounds.size.width * layer.contentsScale,
                                         layer.bounds.size.height * layer.contentsScale);
            gDrawableLayer = layer;
            PearObserveGLGeometry();
            PearGLGeometryChanged();
            PearFitDrawable(layer);
        } else if (PearGLDebug()) {
            PearLog(@"[PearRuntime][gl] renderbufferStorage fromDrawable %@", NSStringFromClass([drawable class]));
        }
        BOOL result = ((BOOL (*)(id, SEL, NSUInteger, id))original)(self, selector, target, drawable);
        if (result && [drawable isKindOfClass:CALayer.class]) {
            typedef void (*GetRenderbufferParameter)(unsigned int, unsigned int, int *);
            static GetRenderbufferParameter query;
            if (!query) query = (GetRenderbufferParameter)PearGLProc("glGetRenderbufferParameteriv");
            if (query) {
                int width = 0, height = 0;
                query((unsigned int)target, 0x8D42 /* GL_RENDERBUFFER_WIDTH */, &width);
                query((unsigned int)target, 0x8D43 /* GL_RENDERBUFFER_HEIGHT */, &height);
                if (width > 0 && height > 0) {
                    gGLDrawableSize = CGSizeMake(width, height);
                    if (PearGLDebug()) PearLog(@"[PearRuntime][gl] allocated renderbuffer=%dx%d", width, height);
                }
            }
        }
        return result;
    }));

    PearLog(@"[PearRuntime][gl] EAGL drawable fit installed");

    // The viewport is only final while the engine is presenting. Reading it
    // here catches both directly linked GL calls and engines that resolve GL
    // through their own table.
    SEL presentSelector = NSSelectorFromString(@"presentRenderbuffer:");
    Method presentMethod = class_getInstanceMethod(context, presentSelector);
    if (presentMethod) {
        IMP presentOriginal = method_getImplementation(presentMethod);
        method_setImplementation(presentMethod, imp_implementationWithBlock(^BOOL(id self, NSUInteger target) {
            PearUpdateDrawableCrop();
            BOOL presented = ((BOOL (*)(id, SEL, NSUInteger))presentOriginal)(self, presentSelector, target);
            if (presented) PearRecordFramePresented();
            return presented;
        }));
        PearLog(@"[PearRuntime][gl] EAGL present hook installed");
    }
}

#pragma mark - Legacy touch diagnostics

/// Logs the coordinates the legacy view hands to its engine. The engine's
/// `createZUITouch:withIndex:` is an interface point; observing its input and
/// output shows whether a mismatch is in UIKit delivery or in the engine's own
/// coordinate conversion.
static void PearInstallLegacyTouchLogging(void) {
    static BOOL installed = NO;
    if (installed) return;
    Class viewClass = NSClassFromString(@"OpenGLView");
    if (!viewClass) return;
    SEL selector = NSSelectorFromString(@"createZUITouch:withIndex:");
    Method method = class_getInstanceMethod(viewClass, selector);
    if (!method) return;
    installed = YES;

    Ivar widthIvar = class_getInstanceVariable(viewClass, "m_width");
    Ivar heightIvar = class_getInstanceVariable(viewClass, "m_height");
    Ivar scaleIvar = class_getInstanceVariable(viewClass, "m_contentScale");
    IMP original = method_getImplementation(method);

    method_setImplementation(method, imp_implementationWithBlock(^void *(id self, UITouch *touch, int index) {
        CGPoint point = [touch locationInView:self];
        CGRect bounds = [self bounds];
        int width = widthIvar ? *(int *)((char *)(__bridge void *)self + ivar_getOffset(widthIvar)) : 0;
        int height = heightIvar ? *(int *)((char *)(__bridge void *)self + ivar_getOffset(heightIvar)) : 0;
        float contentScale = scaleIvar
            ? *(float *)((char *)(__bridge void *)self + ivar_getOffset(scaleIvar)) : 0;

        void *zTouch = ((void *(*)(id, SEL, UITouch *, int))original)(self, selector, touch, index);
        float zx = 0, zy = 0;
        if (zTouch) {
            zx = *(float *)((char *)zTouch + 0xc);
            zy = *(float *)((char *)zTouch + 0x10);
        }
        PearLog(@"[PearRuntime][legacytouch] view=%@ bounds=%@ point=(%.1f,%.1f) "
                "m=%dx%d scale=%.2f index=%d -> z=(%.4f,%.4f) phase=%ld",
                NSStringFromClass([self class]), NSStringFromCGRect(bounds), point.x, point.y,
                width, height, contentScale, index, zx, zy, (long)touch.phase);
        return zTouch;
    }));

    PearLog(@"[PearRuntime][legacytouch] diagnostics installed");
}

__attribute__((constructor))
static void PearGLShimInit(void) {
    PEAR_MODULE_GUARD();
    if (!PearANGLEGameRoutingEnabled()) return;
    // Retry while the application connects its own drawable/window.
    for (int i = 0; i < 40; i++) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(i * 0.5 * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{
            dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
                @autoreleasepool { PearInstallEAGLHook(); }
            });
        });
    }
    if (getenv("PEAR_DEBUG_TOUCH")) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(1.0 * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{
            PearInstallEAGLTouchLogging();
            PearInstallLegacyTouchLogging();
        });
    }
}
