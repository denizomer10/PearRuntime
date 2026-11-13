# PearRuntime

PearRuntime is the compatibility layer that runs modern **arm64 iOS and
iPadOS apps on Apple silicon Macs**. It is a single Mac Catalyst dylib
(`libPearRuntime.dylib`) that loads into the app process and fills the gaps between
an iPhone/iPad environment and macOS: input, graphics, device identity, storage,
and system services.

It is the engine [PearSilicon](https://github.com/denizomer10/PearSilicon) uses
for modern iOS apps. 32-bit iOS apps run on
[PearLegacyEngine](https://github.com/denizomer10/PearLegacyEngine) instead.

## Modules

Each module lives in `modules/<name>/`. All of them build into the same dylib.
You can switch any of them off at launch without rebuilding.

| Module | What it does |
|---|---|
| `core` | Bootstrap, logging, host service IPC, Foundation guards. Always on. |
| `input` | Keyboard/mouse to touch, keymapper, GameController bridge, virtual gamepad, pointer lock, web input |
| `graphics` | OpenGL ES (EAGL) fixes, Metal layer guards, screen geometry, legacy window bridging, FPS HUD |
| `device` | iPhone/iPad device identity, platform reporting, device profile snapshot |
| `storage` | Sandbox paths, keychain, iCloud key-value and CloudKit stand-ins, credentials |
| `services` | Network, on-demand resources, consent/tracking prompts, system services, beep suppression |

```sh
PEAR_RUNTIME_DISABLE=services,graphics   # turn modules off
PEAR_RUNTIME_ONLY=input                  # allow-list (core is always on)
```

The first log line of every launch names the version and the enabled modules:
`[PearRuntime] version 0.8.0 modules=core,input,graphics,device,storage,services`.

## Apple silicon

The build targets `arm64-apple-ios15.0-macabi` with `-O2 -mcpu=apple-m1`.
`apple-m1` is the instruction-set baseline that every Apple silicon Mac has,
so the code is tuned for Apple's cores but runs on M1 through the newest chips.
Set `PEAR_RUNTIME_CPU` to try another tuning.

Hot paths avoid blocking the app's main thread:
- The first controller query waits for discovery only during the first 1.5 s
  after launch and only on the main thread. Later queries never block.
- Per-frame accessors such as a controller's vendor name do not log.

## Game controllers

- Controllers are discovered at bootstrap, so a game that asks during launch sees them.
- Non-Xbox/PlayStation controllers are reported as Xbox controllers, because
  many iOS games only enable controls for those. `PEAR_CONTROLLER_NATIVE=1`
  turns this off.
- **No duplicate connects.** A game that registers its connect observer after a
  controller appeared, and never queried the list, gets the missed connect
  once. A game that already knows the controller never gets it again. Engines
  such as Unreal and Unity otherwise put the same pad into a second player slot
  and ignore its input. `PEAR_CONTROLLER_REPLAY=0` turns delivery off, and
  `PEAR_CONTROLLER_REPLAY=broadcast` brings back the old re-post to every observer.
- Pad to touch: when a keymap exists, a game with no controller support can be
  driven by a pad through touch. Once the game shows its own controller support,
  this fallback turns off so the two do not fight. `PEAR_CONTROLLER_KEYMAP=force`
  keeps it on.

## Build

Requires macOS 26 or later and Xcode or the Command Line Tools (the macOS SDK
includes the iOSSupport frameworks).

```sh
./build.sh                       # output in ./build
PEAR_BUILD_DIR=/tmp/out ./build.sh
```

Outputs `libPearRuntime.dylib`, `libcxxcompat.dylib`, `libswiftUIKit.dylib` and
a `MarketplaceKit.framework` stub.

## Logging

`PearLog` writes to the unified log and to `PEAR_LOG_FILE`. Logs use the
`[PearRuntime][<area>]` prefix that [PearDebug](https://github.com/denizomer10/PearDebug) parses.

## License

MIT. See [LICENSE](LICENSE).

### Independent graphics backends

`graphics` owns shared window and screen geometry. Backend hooks live in
`modules/graphics-gl` (EAGL allocation, presentation, fit and touch conversion)
and `modules/graphics-metal` (CAMetalLayer repair and drawable diagnostics).
They are independently gated at launch:

- `PEAR_RUNTIME_ONLY=graphics-gl,input`: shared geometry + GL + input, no Metal repair hooks.
- `PEAR_RUNTIME_ONLY=graphics-metal,input`: shared geometry + Metal + input, no EAGL hooks.
- `PEAR_RUNTIME_DISABLE=graphics-gl`: keep other modules, disable EAGL hooks.
- `PEAR_RUNTIME_DISABLE=graphics`: disable shared geometry and both backend hooks.

Selecting `graphics` includes both backends; explicit disables take precedence.
Selection is immutable within the process. The shared Metal HUD carrier remains
an instrumentation surface for GL frames, not a change of the game's renderer.
These switches are diagnostic isolation controls, not guarantees of title compatibility.

### ANGLE context core (not game routing yet)

`include/PearANGLEContext.h` and `modules/graphics-gl/ANGLEContext.mm` provide
original EGL context/sharegroup ownership, pixel-sized pbuffers, and native
CALayer/CAMetalLayer surface lifecycle.
The backend is explicit and immutable; no system EAGL context or GL object is
mixed with ANGLE. Cross-thread use is rejected while a context is current.
Presentation currently covers matching RGBA8 color attachment 0; GLES2 copies
require identical pixel extents, while GLES3 core blit can scale. Native bounds
or backing-scale changes require explicit detach/rebind at a frame boundary;
the EAGL adapter must rebuild renderbuffer and input geometry together. GLES1
presentation remains unsupported. The renderbuffer/import adapters are described below.

Use the pinned ANGLE fork's thread-exit and host-layer fixes (`f405241`). Old
staged libraries reproduce EGL_BAD_ACCESS after a current context's thread
exits. The PearSilicon parent repository contains the source-build script and
real Catalyst/Metal shader, framebuffer and thread-lifecycle tests.

### EAGL selector and drawable adapters

`PearEAGLContext.h` implements public context/sharegroup/storage/presentation
selectors using separately named original classes. `PearANGLEDrawable.h`
provides RGBA8 single-sample renderbuffer allocation and immediate native-layer
presentation for GLES2/3. The cached presentation FBO preserves caller GL state;
retained backing keeps the source renderbuffer contents.

Successful allocations publish immutable logical-bounds/pixel-size/scale
snapshots with generations. Input-thread queries do not require a GL context;
old generations and changed layer geometry are rejected. Routed GL deletion
invalidates storage metadata before names can be reused. RGB565/sRGB,
multisampling, scheduled presentation and GLES1 presentation are not implemented.

The classes retain separate Objective-C names. Opt-in game import routing can
now bind EAGL class references and GL/proc lookups to them; system images and
default launches remain unchanged.

### Experimental game import routing

Set `PEAR_GRAPHICS_BACKEND=angle` and an absolute `PEAR_ANGLE_LIB_DIR` when
injecting this runtime before app startup. `ANGLEGameRouting.mm` patches classic
Mach-O app/embedded-image import slots, including GL calls, EAGL classes,
proc-address/dlsym/CFBundle function lookup and dynamic class lookup. The GL
geometry shim uses ANGLE queries in this mode. Renderbuffer deletion invalidates
its geometry record. Unsupported images fail explicitly; no system-GL fallback.
Chained/threaded fixups and arm64e remain unsupported. `graphics-gl` must be on.

Native and synthetic current/previous touch coordinates share the GL layer
conversion. Transient resize rejects a frame without erasing valid storage.
The shared screen cache is refreshed on the main thread for worker consumers.

In isolated test copies, the user confirmed Cut the Rope's ANGLE layout/touch
and Minecraft's native Metal layout/input/pointer lock. This does not translate
Minecraft's native Metal calls into ANGLE or establish compatibility for every
game. The parent repository documents test commands and remaining packaging work.
