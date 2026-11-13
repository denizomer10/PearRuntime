# PearRuntime

**The compatibility layer that runs arm64 iOS and iPadOS apps on Apple silicon Macs.**

PearRuntime is a Mac Catalyst dylib (`libPearRuntime.dylib`) that loads into an
iOS app's own process and fills the gaps between an iPhone or iPad environment
and macOS: input, graphics, device identity, storage and system services. Apple's
`/System/iOSSupport` frameworks still provide UIKit and Metal; PearRuntime
adjusts specific APIs instead of replacing them.

It is the in-process engine of [PearSilicon](https://github.com/denizomer10/PearSilicon),
which prepares apps and injects the runtime. 32-bit iOS apps run on
[PearLegacyEngine](https://github.com/denizomer10/PearLegacyEngine) instead.

## Modules

Each module lives in `modules/<name>/`; all of them build into the same dylib
and any of them can be switched off at launch without rebuilding.

| Module | Responsibility |
| --- | --- |
| `core` | bootstrap, logging, host-service IPC, Foundation guards (always on) |
| `input` | keyboard and mouse to touch, keymapper, GameController bridge, virtual gamepad, pointer lock, web input |
| `graphics` | shared window and screen geometry, legacy window bridging, FPS HUD |
| `graphics-gl` | OpenGL ES: EAGL on ANGLE, presentation, fit and touch conversion |
| `graphics-metal` | `CAMetalLayer` repair and drawable diagnostics |
| `device` | iPhone/iPad device identity, platform reporting, device profile snapshot |
| `storage` | sandbox paths, keychain, iCloud key-value and CloudKit stand-ins, credentials |
| `services` | network, on-demand resources, consent prompts, system services, beep suppression |
| `performance` | keeps game threads off App Nap, timer coalescing and efficiency cores |

```sh
PEAR_RUNTIME_DISABLE=services,graphics-gl   # switch modules off
PEAR_RUNTIME_ONLY=input,graphics-metal      # allow-list (core is always on)
```

Selecting `graphics` includes both backends; explicit disables win. The
selection is fixed for the life of the process. The first log line of every
launch names the version and the enabled modules.

## OpenGL ES on Metal (ANGLE)

iOS OpenGL ES games run only on [PearANGLE](https://github.com/denizomer10/PearANGLE),
the ANGLE fork pinned as `third_party/angle`; the system OpenGL ES framework is
never used.

- `libPearOpenGLES.dylib` is a GLES/EAGL bridge built by `build-angle-bridge.sh`
  from generated forwarders. The installer copies it and `PearANGLE/`
  (`libEGL`, `libGLESv2`) into every prepared app.
- `ANGLEGameRouting.mm` patches the app's Mach-O import slots, `dlsym`,
  `CFBundle` function lookup and dynamic class lookup so GL calls and EAGL
  classes reach ANGLE. Unsupported images fail explicitly; there is no silent
  fallback to system GL.
- `PearEAGLContext` implements the public context, sharegroup, storage and
  presentation selectors over EGL contexts in one share group.
  `PearANGLEDrawable` allocates renderbuffers and presents them to the native
  layer. Geometry is published as immutable, generation-tagged snapshots so the
  input thread never needs a GL context.

## Game controllers

- Controllers are discovered at bootstrap, so a game that asks during launch
  sees them.
- Controllers other than Xbox and PlayStation pads are reported as Xbox
  controllers, because many iOS games only enable controls for those
  (`PEAR_CONTROLLER_NATIVE=1` turns this off).
- A game that registers its connect observer late gets the missed connect
  exactly once; a game that already knows the pad never gets a duplicate, which
  would otherwise put the same pad into a second player slot in Unreal and Unity
  games (`PEAR_CONTROLLER_REPLAY=0|broadcast`).
- With a keymap, a game without controller support can be driven by a pad
  through touch. The fallback turns itself off once the game shows its own
  controller support (`PEAR_CONTROLLER_KEYMAP=force` keeps it on).

## Building

Requirements: macOS 26 or later and Xcode or the Command Line Tools (the macOS
SDK contains the iOSSupport frameworks). ANGLE must be built first; from a
PearSilicon checkout run `bash scripts/build-angle-metal.sh --sync`.

```sh
git submodule update --init --depth 1 third_party/angle
./build.sh                          # output in ./build
PEAR_BUILD_DIR=/tmp/out ./build.sh
```

Outputs:

| File | Purpose |
| --- | --- |
| `libPearRuntime.dylib` | the runtime |
| `libPearOpenGLES.dylib` | GLES/EAGL bridge to ANGLE |
| `PearANGLE/` | ANGLE dylibs, `LICENSE.ANGLE` and `THIRD_PARTY_LICENSES.txt` |
| `libcxxcompat.dylib` | C++ runtime compatibility shim |
| `libswiftUIKit.dylib` | UIKit Swift overlay for Catalyst |
| `MarketplaceKit.framework` | distribution metadata stub |

The build targets `arm64-apple-ios15.0-macabi` with `-O2 -mcpu=apple-m1`, the
baseline every Apple silicon Mac implements; `PEAR_RUNTIME_CPU` and
`PEAR_RUNTIME_OPT` override it. `collect-angle-licenses.py` reads ANGLE's link
inputs and gathers the license of every third-party library inside the dylibs,
so the notices always match the binaries.

## Logging

`PearLog` writes to the unified log and to `PEAR_LOG_FILE`, with a
`[PearRuntime][<area>]` prefix that [PearDebug](https://github.com/denizomer10/PearDebug)
parses and analyzes.

## The Pear stack

| Project | Role |
| --- | --- |
| [PearSilicon](https://github.com/denizomer10/PearSilicon) | macOS app: library, installer, launcher |
| **PearRuntime** | compatibility layer for arm64 iOS apps |
| [PearANGLE](https://github.com/denizomer10/PearANGLE) | OpenGL ES on Metal (ANGLE fork) |
| [PearLegacyEngine](https://github.com/denizomer10/PearLegacyEngine) | 32-bit iOS engine (touchHLE fork) |
| [PearLegacyJIT](https://github.com/denizomer10/PearLegacyJIT) | ARMv7 recompiler (Dynarmic fork) |
| [PearDebug](https://github.com/denizomer10/PearDebug) | logging and diagnostics |

## License

PearRuntime is MIT licensed — see [LICENSE](LICENSE). PearANGLE is BSD-3-Clause
and keeps its own license files; see `third_party/angle/LICENSE`.
