// PerformanceShim.mm
//
// Apple silicon scheduling and latency tuning for iOS games on the Mac.
//
// Three things make an iOS game hitch on a Mac that never make it hitch on a
// phone:
//
//  1. App Nap and timer coalescing. A Catalyst process is an ordinary Mac app
//     to the scheduler: when it is partly covered or the user is idle, timers
//     are coalesced by up to several milliseconds and the process can be
//     napped. Game loops driven by NSTimer / dispatch timers then miss frames.
//     A latency-critical, user-initiated activity turns both off for the life
//     of the game.
//
//  2. Efficiency cores. Engines create their render, job and streaming
//     threads with plain pthread_create and no QoS. On Apple silicon such
//     threads are free to run on the E-cores; a render thread that lands there
//     for a frame produces a visible spike. Threads created without a QoS (and
//     without an explicit scheduling policy) are promoted: user-initiated by
//     default, user-interactive in Game Mode. Threads that ask for a QoS or a
//     real-time policy (audio) are never touched.
//
//  3. Game Mode. macOS gives the foreground game priority on the CPU and GPU
//     and halves Bluetooth controller latency, but only for apps that declare
//     the games category (PearSilicon writes that into the bundle). The
//     runtime side of Game Mode is the stronger thread QoS above.
//
// Environment:
//   PEAR_GAME_MODE=0          disable Game Mode tuning (default: on)
//   PEAR_THREAD_QOS=off|initiated|interactive   override the promotion
//   PEAR_NO_LATENCY_ACTIVITY  do not hold the latency-critical activity
#import <Foundation/Foundation.h>
#import <pthread.h>
#import <pthread/qos.h>
#import <strings.h>
#import "PearRuntime.h"

static BOOL PearGameModeEnabled(void) {
    static BOOL enabled = YES;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        const char *value = getenv("PEAR_GAME_MODE");
        if (value && (strcmp(value, "0") == 0 || strcasecmp(value, "off") == 0 ||
                      strcasecmp(value, "false") == 0)) {
            enabled = NO;
        }
    });
    return enabled;
}

/// QoS given to threads created without one. QOS_CLASS_UNSPECIFIED disables
/// the promotion. Resolved once, before the first interposed call can race.
static qos_class_t gThreadQoS = QOS_CLASS_UNSPECIFIED;

static void PearResolveThreadQoS(void) {
    if (!PearRuntimeModuleEnabled(PEAR_RUNTIME_MODULE)) return;
    const char *value = getenv("PEAR_THREAD_QOS");
    if (value && (strcasecmp(value, "off") == 0 || strcmp(value, "0") == 0)) return;
    if (value && strcasecmp(value, "interactive") == 0) {
        gThreadQoS = QOS_CLASS_USER_INTERACTIVE;
    } else if (value && strcasecmp(value, "initiated") == 0) {
        gThreadQoS = QOS_CLASS_USER_INITIATED;
    } else {
        gThreadQoS = PearGameModeEnabled() ? QOS_CLASS_USER_INTERACTIVE : QOS_CLASS_USER_INITIATED;
    }
}

static int PearPthreadCreate(pthread_t *thread, const pthread_attr_t *attr,
                             void *(*start)(void *), void *arg) {
    const qos_class_t target = gThreadQoS;
    if (target == QOS_CLASS_UNSPECIFIED) return pthread_create(thread, attr, start, arg);

    pthread_attr_t local;
    if (attr) {
        qos_class_t requested = QOS_CLASS_UNSPECIFIED;
        int relative = 0;
        if (pthread_attr_get_qos_class_np(const_cast<pthread_attr_t *>(attr), &requested, &relative) == 0 &&
            requested != QOS_CLASS_UNSPECIFIED) {
            return pthread_create(thread, attr, start, arg);
        }
        int inherit = PTHREAD_INHERIT_SCHED;
        if (pthread_attr_getinheritsched(attr, &inherit) == 0 && inherit == PTHREAD_EXPLICIT_SCHED) {
            // An explicit policy (real-time audio, fixed priority) wins.
            return pthread_create(thread, attr, start, arg);
        }
        local = *attr;
    } else {
        pthread_attr_init(&local);
    }
    pthread_attr_set_qos_class_np(&local, target, 0);
    int result = pthread_create(thread, &local, start, arg);
    if (!attr) pthread_attr_destroy(&local);
    return result;
}

// dyld applies interposing to every image loaded at launch except this one, so
// the pthread_create calls above reach the real implementation.
__attribute__((used)) static const struct {
    const void *replacement;
    const void *replacee;
} kPearInterposePthreadCreate __attribute__((section("__DATA,__interpose"))) = {
    (const void *)(unsigned long)&PearPthreadCreate,
    (const void *)(unsigned long)&pthread_create,
};

static id gLatencyActivity;

static void PearBeginLatencyActivity(void) {
    if (getenv("PEAR_NO_LATENCY_ACTIVITY")) return;
    // NSActivityLatencyCritical is declared for macOS only, but the process is
    // a Mac process and Foundation honours the bit under Catalyst as well.
    const NSActivityOptions latencyCritical = (NSActivityOptions)0xFF00000000ULL;
    NSActivityOptions options = NSActivityUserInitiatedAllowingIdleSystemSleep | latencyCritical;
    gLatencyActivity = [NSProcessInfo.processInfo beginActivityWithOptions:options
                                                                    reason:@"PearRuntime: game loop timing"];
}

__attribute__((constructor))
static void PearPerformanceShimInit(void) {
    PearResolveThreadQoS();
    PEAR_MODULE_GUARD();
    @autoreleasepool {
        // The main thread drives most iOS game loops (CADisplayLink on the main
        // run loop); make sure nothing launched it below user-interactive.
        pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
        PearBeginLatencyActivity();
        const char *qos = gThreadQoS == QOS_CLASS_USER_INTERACTIVE ? "user-interactive"
                        : gThreadQoS == QOS_CLASS_USER_INITIATED ? "user-initiated" : "off";
        PearLog(@"[PearRuntime][perf] game mode=%s thread qos=%s latency activity=%s",
                PearGameModeEnabled() ? "on" : "off", qos, gLatencyActivity ? "on" : "off");
    }
}
