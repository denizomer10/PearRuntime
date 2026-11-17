# PearRuntime

**Independent Apple-silicon, Metal-only compatibility runtime for iOS and iPadOS apps on macOS.**

PearRuntime unifies the 64-bit **PearHost** platform layer (**100% Rust** with minimal ObjC/ASM ABI shims, zero `/System/iOSSupport`, zero Swift), [PearANGLE](https://github.com/denizomer10/PearANGLE) (OpenGL ES on Metal), and [PearLegacyEngine](https://github.com/denizomer10/PearLegacyEngine) + [PearLegacyJIT](https://github.com/denizomer10/PearLegacyJIT) (32-bit ARMv7 → arm64) under a single build and orchestration layer for [PearSilicon](https://github.com/denizomer10/PearSilicon).

## Architecture

| Layer | Language | Responsibility |
| --- | --- | --- |
| `rust/crates/pear-uikit` & `host/uikit/` | **Rust** (with minimal ObjC/ASM ABI shims) | Clean-room `UIKit.framework` (35/35 census classes, 0 stubs), AppKit windowing, display link, keyboard/text & pointer routing, `UINib` instantiation |
| `rust/crates/pear-metalkit` & `host/metalkit/` | **Rust** | `UIView`-backed `MTKView` on Metal (`CAMetalLayer`) |
| `rust/crates/pear-gles` & `host/opengles/` | **Rust** | `EAGLContext` / `EAGLSharegroup` over Metal-only `PearANGLE` (`EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE`), re-exporting `libGLESv2` |
| `rust/crates/pear-nib` | **Rust** | Pure-Rust `NIBArchive` binary parser |
| `rust/crates/pear-objc` | **Rust** | Zero-dependency `libobjc.A.dylib` FFI & runtime registration helpers |
| `rust/crates/pear-core` | **Rust** | Thread-safe logging (`PearHostLogMessage`), missing-call tracker (`PearMissingReport`), `dladdr` symbolication, JSONL reporting |
| `rust/crates/pear-macho` | **Rust** | ARM64 Mach-O header/platform inspector, FairPlay `cryptid` check, `LC_BUILD_VERSION` → `macOS 14.0` retargeter |
| `third_party/angle` | **C++ / Metal** | [PearANGLE](https://github.com/denizomer10/PearANGLE) — Metal-only arm64 Apple build (`libEGL.dylib`, `libGLESv2.dylib`) |
| `third_party/legacy-engine` & `legacy-jit` | **Rust / C++** | 32-bit `armv6`/`armv7` iOS execution (`PearLegacyEngine` + `PearLegacyJIT`) |
| `modules/` | **ObjC / C++** | Legacy Mac Catalyst in-process dylib (`libPearRuntime.dylib`, built via `./build.sh`) |

## Building

Requirements: Apple silicon Mac, macOS 14+, Xcode / Command Line Tools, and Rust (`cargo`).

### Unified build (`PearHost` + `PearLegacyEngine`)

```sh
git submodule update --init --depth 1 third_party/angle
bash host/build-pearangle.sh --sync   # first time: sync & build Metal-only PearANGLE
bash build-all.sh                     # builds Rust PearHost frameworks & PearLegacyEngine
```

Outputs in `build/host/`:
- `UIKit.framework`, `MetalKit.framework`, `OpenGLES.framework`, `iAd.framework`
- `PearANGLE/` (`libEGL.dylib`, `libGLESv2.dylib`, `LICENSE.ANGLE`)
- `PearLegacyEngine/touchHLE`

### Preparing and running an IPA directly (no Catalyst / no iOSSupport)

```sh
tools/host-prepare.sh ~/Downloads/App.ipa build/host-apps/app.app
tools/host-run.sh build/host-apps/app.app build/host-apps/runs/latest 60
```

Or enable **PearHost mode (no iOSSupport)** per app in [PearSilicon](https://github.com/denizomer10/PearSilicon).

## Logging & Diagnostics

`pear-core` writes `[PearRuntime][<area>]` logs to `stderr` and `PEAR_LOG_FILE`, and emits `PEAR_MISSING_REPORT` JSONL (`first` + `summary`) for any unimplemented selector or behavior (analyzed by [PearDebug](https://github.com/denizomer10/PearDebug)).

## The Pear stack

| Project | Role |
| --- | --- |
| [PearSilicon](https://github.com/denizomer10/PearSilicon) | macOS Swift app: library, installer, launcher |
| **PearRuntime** | Unified runtime: 100% Rust PearHost + PearLegacyEngine bridge |
| [PearANGLE](https://github.com/denizomer10/PearANGLE) | OpenGL ES on Metal (ANGLE fork, arm64 Metal-only) |
| [PearLegacyEngine](https://github.com/denizomer10/PearLegacyEngine) | 32-bit iOS engine (Rust) |
| [PearLegacyJIT](https://github.com/denizomer10/PearLegacyJIT) | ARMv7 recompiler (Dynarmic fork) |
| [PearDebug](https://github.com/denizomer10/PearDebug) | logging and diagnostics |
