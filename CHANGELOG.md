# Changelog

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
