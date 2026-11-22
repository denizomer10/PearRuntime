# Changelog

## Unreleased

- The Mac Catalyst runtime is Rust end to end (`rust/crates/pear-catalyst`):
  the remaining Objective-C++ modules (core bootstrap and logging, keymapper,
  GameController bridge, virtual gamepad, touch synthesis, consent and cloud
  compatibility, and the whole ANGLE/EAGL/GLKit graphics-gl stack) were
  ported with log and hook parity against the Objective-C++ build.
- `libPearOpenGLES.dylib` is a Rust bridge (`rust/crates/pear-opengles`): one
  register-preserving trampoline per GLES prototype in ANGLE's headers,
  replacing the generator that needed an unpinned Khronos registry.
  `libcxxcompat.dylib` is Rust as well.
- Build: the runtime build fails on unresolved flat-namespace imports (one
  such import made dyld drop every interpose tuple), links IOKit explicitly,
  and supports Rust 1.99 (stable C variadics).
- ANGLE tests link the built runtime (`PEAR_RUNTIME_BUILD`); exclusivity is
  checked on what the client resolves, since macOS 27's iOSSupport UIKit maps
  the system OpenGLES image itself.

## 0.8.0

- PearRuntime is now its own repository, split from PearSilicon's `runtime/`
  folder with its history.
- Modules: sources are grouped into `core`, `input`, `graphics`, `device`,
  `storage` and `services`. Modules can be switched off at launch with
  `PEAR_RUNTIME_DISABLE` / `PEAR_RUNTIME_ONLY`.
- Apple silicon: optimized build (`-O2 -mcpu=apple-m1`, previously unoptimized),
  parallel per-module compilation, dead-strip.
- Controllers: removed the 3-second re-post of controller connects that put the pad
  into a second player slot in Unreal/Unity games (GTA: Vice City Definitive
  Edition, Genshin Impact). Missed connects are delivered only to late
  observers. Pad-to-touch translation stops once a game drives
  controllers itself. Game pad handlers are kept per controller.
- Controller queries no longer block the main thread for 1.2 s each time the list is empty.
