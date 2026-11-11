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
