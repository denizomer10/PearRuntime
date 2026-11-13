// FPSHUD.mm
//
// One performance HUD for every iOS game: Apple's Metal HUD (MTL_HUD_ENABLED).
//
// Metal HUD attaches to the CAMetalLayer an app presents to. OpenGL ES games
// present through EAGL instead, so the HUD either never appeared or vanished
// as soon as a Metal-drawn screen (a store/sign-in UI, a launcher) closed and
// only the GL view was left. Earlier builds drew their own "FPS" label for GL
// titles, which then showed next to Metal HUD on mixed screens.
//
// Instead, GL titles get a transparent CAMetalLayer on top of the window that
// presents one cleared frame for every frame the game presents (from the EAGL
// present hook). Metal HUD attaches to that layer, so it stays visible and
// its FPS / frame interval are the game's own. While the game itself presents
// to another Metal layer (a Flutter or Metal UI), the carrier stays idle so
// only one HUD is ever shown.
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import <Metal/Metal.h>
#import <atomic>
#import "PearRuntime.h"

static BOOL PearHUDRequested(void) {
    static BOOL requested;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        const char *value = getenv("MTL_HUD_ENABLED");
        requested = value && strcmp(value, "0") != 0 && *value;
    });
    return requested;
}

static CAMetalLayer *gCarrier = nil;
static std::atomic<void *> gCarrierPointer{nullptr};
static id<MTLCommandQueue> gCarrierQueue = nil;
static std::atomic<int> gCarrierInFlight{0};
static std::atomic<bool> gCarrierInstalling{false};
/// Last time (CLOCK_UPTIME_RAW ns) any other Metal layer asked for a drawable.
static std::atomic<uint64_t> gLastAppMetalDrawable{0};

static uint64_t PearNowNanos(void) { return clock_gettime_nsec_np(CLOCK_UPTIME_RAW); }

/// Called from the CAMetalLayer nextDrawable hook for every layer.
void PearMetalHUDNoteDrawable(void *layer) {
    if (layer != gCarrierPointer.load(std::memory_order_relaxed)) {
        gLastAppMetalDrawable.store(PearNowNanos(), std::memory_order_relaxed);
    }
}

static void PearSizeCarrier(void) {
    CAMetalLayer *carrier = gCarrier;
    UIWindow *window = PearPrimaryWindow();
    if (!carrier || !window) return;
    CALayer *host = window.layer;
    if (carrier.superlayer != host) {
        [carrier removeFromSuperlayer];
        [host addSublayer:carrier];
    }
    CGRect bounds = host.bounds;
    CGFloat scale = window.traitCollection.displayScale > 0 ? window.traitCollection.displayScale : 1.0;
    CGSize drawable = CGSizeMake(round(bounds.size.width * scale), round(bounds.size.height * scale));
    if (!CGRectEqualToRect(carrier.frame, bounds) || !CGSizeEqualToSize(carrier.drawableSize, drawable)) {
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        carrier.frame = bounds;
        carrier.contentsScale = scale;
        if (drawable.width >= 1 && drawable.height >= 1) carrier.drawableSize = drawable;
        [CATransaction commit];
    }
}

static void PearInstallCarrier(void) {
    if (gCarrier) return;
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    UIWindow *window = PearPrimaryWindow();
    if (!device || !window) {
        gCarrierInstalling.store(false);
        return;
    }
    CAMetalLayer *carrier = [CAMetalLayer layer];
    carrier.device = device;
    carrier.pixelFormat = MTLPixelFormatBGRA8Unorm;
    carrier.opaque = NO;
    carrier.framebufferOnly = YES;
    carrier.zPosition = 10000;
    carrier.name = @"PearMetalHUDCarrier";
    carrier.allowsNextDrawableTimeout = YES;
    gCarrierQueue = [device newCommandQueue];
    gCarrier = carrier;
    gCarrierPointer.store((__bridge void *)carrier);
    PearSizeCarrier();

    for (NSString *name in @[ @"NSWindowDidResizeNotification",
                              @"NSWindowDidEnterFullScreenNotification",
                              @"NSWindowDidExitFullScreenNotification",
                              @"NSWindowDidChangeBackingPropertiesNotification",
                              UIWindowDidBecomeKeyNotification ]) {
        [NSNotificationCenter.defaultCenter addObserverForName:name object:nil
            queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *note) {
                PearSizeCarrier();
                dispatch_async(dispatch_get_main_queue(), ^{ PearSizeCarrier(); });
            }];
    }
    // UIKit settles a new window size a little after AppKit reports it, and a
    // scene can swap its root window; a slow check covers both.
    static dispatch_source_t timer;
    timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
    dispatch_source_set_timer(timer, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC),
                              NSEC_PER_SEC, NSEC_PER_SEC / 4);
    dispatch_source_set_event_handler(timer, ^{ PearSizeCarrier(); });
    dispatch_resume(timer);
    PearLog(@"[PearRuntime][hud] Metal HUD carrier installed for OpenGL ES presents");
}

/// Called from the EAGL `presentRenderbuffer:` hook after a successful present.
void PearRecordFramePresented(void) {
    if (!PearHUDRequested()) return;
    CAMetalLayer *carrier = gCarrier;
    if (!carrier) {
        if (!gCarrierInstalling.exchange(true)) {
            dispatch_async(dispatch_get_main_queue(), ^{ @autoreleasepool { PearInstallCarrier(); } });
        }
        return;
    }
    // The game is drawing a Metal screen right now; Metal HUD is on that layer.
    if (PearNowNanos() - gLastAppMetalDrawable.load(std::memory_order_relaxed) < 300 * NSEC_PER_MSEC) {
        return;
    }
    // Never block the game's render thread: at most two carrier frames in
    // flight, extra presents are skipped.
    if (gCarrierInFlight.fetch_add(1) >= 2) {
        gCarrierInFlight.fetch_sub(1);
        return;
    }
    @autoreleasepool {
        id<CAMetalDrawable> drawable = [carrier nextDrawable];
        id<MTLCommandBuffer> commands = drawable ? [gCarrierQueue commandBuffer] : nil;
        if (!commands) {
            gCarrierInFlight.fetch_sub(1);
            return;
        }
        MTLRenderPassDescriptor *pass = [MTLRenderPassDescriptor renderPassDescriptor];
        pass.colorAttachments[0].texture = drawable.texture;
        pass.colorAttachments[0].loadAction = MTLLoadActionClear;
        pass.colorAttachments[0].storeAction = MTLStoreActionStore;
        pass.colorAttachments[0].clearColor = MTLClearColorMake(0, 0, 0, 0);
        [[commands renderCommandEncoderWithDescriptor:pass] endEncoding];
        [commands addCompletedHandler:^(__unused id<MTLCommandBuffer> buffer) {
            gCarrierInFlight.fetch_sub(1);
        }];
        [commands presentDrawable:drawable];
        [commands commit];
    }
}

/// Kept for the bootstrap call; nothing to install until a GL frame arrives.
void PearInstallFPSHUD(void) {
    if (PearHUDRequested()) {
        PearLog(@"[PearRuntime][hud] Metal HUD requested; OpenGL ES titles get a carrier layer");
    }
}
