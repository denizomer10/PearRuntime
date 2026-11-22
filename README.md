# PearRuntime

**Independent Apple-silicon, Metal-only compatibility runtime for iOS and iPadOS apps on macOS.**

PearRuntime unifies the 64-bit **PearHost** platform layer (**100% Rust** with minimal ObjC/ASM ABI shims, zero `/System/iOSSupport`, zero Swift), [PearANGLE](third_party/angle) (OpenGL ES on Metal), and [PearLegacyEngine](third_party/legacy-engine) + [PearLegacyJIT](third_party/legacy-jit) (32-bit ARMv7 → arm64) under a single build and orchestration layer for [PearSilicon](https://github.com/denizomer10/PearSilicon).

## Architecture

| Layer | Language | Responsibility |
| --- | --- | --- |
| `rust/crates/pear-uikit` | **Rust** | Clean-room `UIKit.framework` (35/35 census classes, 0 stubs), AppKit windowing, display link, keyboard/text & pointer routing, `UINib` instantiation |
| `rust/crates/pear-metalkit` | **Rust** | `UIView`-backed `MTKView` on Metal (`CAMetalLayer`) |
| `rust/crates/pear-gles` | **Rust** | `EAGLContext` / `EAGLSharegroup` over Metal-only `PearANGLE` (`EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE`), re-exporting `libGLESv2` |
| `rust/crates/pear-nib` | **Rust** | Pure-Rust `NIBArchive` binary parser |
| `rust/crates/pear-objc` | **Rust** | Zero-dependency `libobjc.A.dylib` FFI & runtime registration helpers |
| `rust/crates/pear-core` | **Rust** | Thread-safe logging (`PearHostLogMessage`), missing-call tracker (`PearMissingReport`), Objective-C forwarding/KVC hooks (`PearMissingInstallForImage`), zero-result IMP (`pear_missing_zero`), `dladdr` symbolication, JSONL reporting |
| `rust/crates/pear-macho` | **Rust** | ARM64 Mach-O header/platform inspector, FairPlay `cryptid` check, `LC_BUILD_VERSION` → `macOS 26.0` retargeter |
| `third_party/angle` | **C++ / Metal** | PearANGLE — Metal-only arm64 Apple build (`libEGL.dylib`, `libGLESv2.dylib`) |
| `third_party/legacy-engine` & `legacy-jit` | **Rust / C++** | 32-bit `armv6`/`armv7` iOS execution (`PearLegacyEngine` + `PearLegacyJIT`) |
| `rust/crates/pear-catalyst` | **Rust** | Mac Catalyst in-process runtime (`libPearRuntime.dylib`, built via `./build.sh`): core, input, graphics, graphics-gl (ANGLE/EAGL/GLKit), graphics-metal, device, storage, services, performance |
| `rust/crates/pear-opengles` | **Rust** | Catalyst GLES/EAGL ABI bridge (`libPearOpenGLES.dylib`): register-preserving trampolines for every GLES prototype in ANGLE's headers |
| `rust/crates/pear-keychain` | **Rust** | Keychain store shared by PearHost and the Catalyst runtime |
| `rust/crates/pear-cxxcompat` | **Rust** | `libcxxcompat.dylib` (re-exports the system C++ runtime, supplies `std::piecewise_construct`) |
| `compat/swift/` | **Swift** | Swift ABI overlays (`libswiftUIKit.dylib`, `MarketplaceKit.framework` stub); Swift ABI symbols are emitted by `swiftc` |

### Mac Catalyst runtime: no Objective-C

`libPearRuntime.dylib` and `libPearOpenGLES.dylib` contain no Objective-C or
C++ code. Classes iOS binaries bind to at load time (`EAGLContext`,
`EAGLSharegroup`, `GLKView`, `GLKViewController`) are emitted from Rust as
static class data (`__objc_classlist`), C variadics (`PearLog`,
`PearANGLETrace`) are Rust `extern "C" fn(…, ...)`, and `dlsym`/`dlopen`
routing tail-calls through naked functions so caller-image semantics hold.
`include/` keeps the C/Objective-C declarations of the exported API for
clients and tests. Only the Swift overlays in `compat/swift/` and
`third_party/` are not Rust.

### PearHost: the Objective-C ABI in Rust

PearHost has no Objective-C, C or assembly sources; cargo builds every
framework (clang only links). What iOS binaries bind to is emitted from Rust
the way clang emits it from Objective-C (`pear_objc::abi`):

- **Class symbols** (`_OBJC_CLASS_$_UIView`, `MTKView`, `EAGLContext`, …) that
  iOS binaries bind to and subclass, as static class metadata with the layout
  clang gave them: `pear-uikit/src/objc_classes.rs`, `pear_objc::empty_class!`.
- **Constant `NSString`s** (`UIApplicationDidBecomeActiveNotification`, …):
  `pear_objc::nsstring_constant!` (`pear-uikit/src/globals.rs`, generated
  `generated_stubs.rs`).
- **`@try`/`@catch` boundaries** (launch, KVC probes): Rust cannot catch an
  Objective-C exception, so `pear_objc::abi::{try_call, try_send}` are
  `global_asm!` frames with the Objective-C personality, as clang generates.

`tools/host-abi-census.py` records a host build's exported symbols, constant
values and Objective-C classes (ivars, properties, methods and where each
method's implementation lives) and diffs two builds.

## Building

Requirements: Apple silicon Mac, macOS 26+, Xcode / Command Line Tools, and Rust (`cargo`).

### Unified build (`PearHost` + `PearLegacyEngine`)

```sh
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

`pear-core` writes `[PearRuntime][<area>]` logs to `stderr` and `PEAR_LOG_FILE`, and emits `PEAR_MISSING_REPORT` JSONL (`first` + `summary`) for any unimplemented selector or behavior (analyzed by [PearDebug](debug)).

## The Pear stack

| Project | Role |
| --- | --- |
| [PearSilicon](https://github.com/denizomer10/PearSilicon) | macOS Swift app: library, installer, launcher |
| **PearRuntime** | Unified runtime: 100% Rust PearHost + PearLegacyEngine bridge |
| [PearANGLE](third_party/angle) (`third_party/angle`) | OpenGL ES on Metal (ANGLE fork, arm64 Metal-only) |
| [PearLegacyEngine](third_party/legacy-engine) (`third_party/legacy-engine`) | 32-bit iOS engine (Rust) |
| [PearLegacyJIT](third_party/legacy-jit) (`third_party/legacy-jit`) | ARMv7 recompiler (Dynarmic fork) |
| [PearDebug](debug) (`debug/`) | logging and diagnostics |

These four are part of this repository. Where each part comes from and
under which license: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
PearLegacyEngine's and PearLegacyJIT's earlier history, with original commit
IDs (including the commits PearSilicon releases up to v0.9.2 name as the
engine's source), is kept under the tag `archive/2026-11-22`; any of those
commits opens as `https://github.com/denizomer10/PearRuntime/tree/<commit>`.
