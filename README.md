# PearRuntime

**Clean-room Rust compatibility runtime for running 64-bit (`arm64`) and 32-bit (`armv6`/`armv7`) iOS and iPadOS applications on macOS and Linux — with zero `/System/iOSSupport` and zero Mac Catalyst.**

PearRuntime provides an independent, modular compatibility stack powering [PearSilicon](https://github.com/denizomer10/PearSilicon). It unifies:

1. **Standalone Portable Runtime (`pearruntime`)** — a pure-Rust, zero-Apple-framework-dependency Mach-O loader, Apple 64-bit Objective-C & Swift runtime, Foundation/CoreGraphics/Networking/Security layer, and Metal-to-Vulkan graphics pipeline targeting macOS and Linux (`arm64` native and `x86_64` via JIT).
2. **Native macOS `PearHost` Frameworks (`build/host/`)** — clean-room `UIKit`, `MetalKit`, `OpenGLES` (backed by Metal-only [`PearANGLE`](third_party/angle)), `GLKit`, Keychain, and controller engines emitted directly from Rust with static Mach-O Objective-C ABI tables (no Objective-C source files, no `/System/iOSSupport`).
3. **32-bit Legacy & Dynamic Recompilation** — [`PearLegacyEngine`](third_party/legacy-engine) and [`PearLegacyJIT`](third_party/legacy-jit) (Dynarmic-based ARMv7 → `arm64`/`x86_64` recompiler) integrated through `pear-legacy-host` and `pear-jit`.

---

## Architecture Overview

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│           iOS / iPadOS Application (.app + @rpath/*.framework)               │
│                 64-bit arm64 Mach-O  │  32-bit armv6 / armv7                 │
├──────────────────────────────────────┼───────────────────┬───────────────────┤
│  Standalone Portable Runtime         │  Native macOS     │  32-bit Legacy    │
│  (`pearruntime` CLI)                 │  PearHost         │  & JIT            │
├──────────────────────────────────────┼───────────────────┼───────────────────┤
│  Loader & Language ABI               │  PearHost FWs     │  pear-legacy-host │
│   • pear-macho / pear-dyld           │   • pear-objc     │  pear-jit         │
│     (@rpath, fixups, TLV, unwind)    │   • pear-uikit    │                   │
│   • pear-libobjc (Apple 64-bit ABI)  │   • pear-metalkit │  third_party/     │
│   • pear-swift-rt (Swift 5 interop)  │   • pear-gles     │   • legacy-engine │
│                                      │   • pear-glkit    │   • legacy-jit    │
│  System & Graphics Stack             │   • pear-nib      │     (Dynarmic)    │
│   • pear-runtime (libSystem/dispatch │   • pear-keychain │                   │
│     libc++ std::__1, Security, Audio │   • pear-         │                   │
│     Accelerate, CoreMotion, GameCtrl)│     controller    │                   │
│   • pear-foundation (CF, Foundation, │   • pear-host-    │                   │
│     NSURLSession, NSURLConnection)   │     frameworks    │                   │
│   • pear-graphics / pear-image       │   • pear-cxxcompat│                   │
│   • pear-metal-vk (Metal → Vulkan,   │   • pear-swift-   │                   │
│     MTLB/AIR → SPIR-V, ASTC, raster) │     support       │                   │
├──────────────────────────────────────┴───────────────────┴───────────────────┤
│  Host Abstraction & Platform Backends                                        │
│   • pear-host (HostPlatform trait) → pear-host-macos / pear-host-linux       │
│   • Metal-only PearANGLE (libEGL.dylib / libGLESv2.dylib)                    │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Core Architectural Highlights

- **Pure-Rust Multi-Image Mach-O Loader & Language ABIs:** [`pear-macho`](rust/crates/pear-macho) and [`pear-dyld`](rust/crates/pear-dyld) map guest arm64 images, resolve `LC_RPATH` / `@rpath` / `@executable_path` / `@loader_path` dependencies, apply chained/rebase/bind fixups, wire Thread-Local Variables (TLV), and register `__unwind_info` / `__eh_frame` without invoking system `dyld`. [`pear-libobjc`](rust/crates/pear-libobjc) implements the Apple 64-bit Objective-C ABI (ARC, blocks, exception frames, reflection, dynamic class realization), while [`pear-swift-rt`](rust/crates/pear-swift-rt) provides the Swift 5 runtime and `_TtCs12_SwiftObject` interop layer.
- **Metal-to-Vulkan & Graphics Pipeline (`pear-metal-vk`):** Clean-room Metal Objective-C API backed by Vulkan 1.2/1.3 (`vk`), `MTLB` / Apple IR bitcode parsing & SPIR-V translation (`mtlb`, `spirv`), full bit-exact Khronos ASTC LDR decompression (`astc`), `RwLock`-aliased live texture views, 3D clip/projection guards, and a division-free specialized scanline software rasterizer (`objc_metal`), complemented by [`pear-graphics`](rust/crates/pear-graphics) and [`pear-image`](rust/crates/pear-image) (PNG with Apple's `CgBI` premultiplied BGRA variant, JPEG).
- **Networking, Security & Darwin System Emulation:** Clean-room `NSURLSession` / `NSURLSessionTask` / `NSURLConnection` / `NSHTTPURLResponse` / `NSError` HTTP & HTTPS stack in [`pear-foundation`](rust/crates/pear-foundation); POSIX/Darwin `libSystem`, `libdispatch`, Apple `libc++` (`std::__1`), `Security` (`SecItem*`/`SecKey*` + TLS), `Accelerate`, `AudioToolbox`/`AVAudioSession`, `CoreMotion`, and `GameController` shims in [`pear-runtime`](rust/crates/pear-runtime).
- **Zero-Objective-C `PearHost` Frameworks on macOS:** [`pear-uikit`](rust/crates/pear-uikit), [`pear-metalkit`](rust/crates/pear-metalkit), [`pear-gles`](rust/crates/pear-gles), and [`pear-glkit`](rust/crates/pear-glkit) emit static Mach-O Objective-C class metadata, constant `NSString`s, and `@try`/`@catch` personality frames straight from Rust via [`pear-objc`](rust/crates/pear-objc). Hardware controller support in [`pear-controller`](rust/crates/pear-controller) covers Switch 2 Joy-Cons (BLE), DualSense, Xbox, Switch Pro/Joy-Con, HID gamepads, keymapping, and virtual gamepads.

---

## Workspace Crates (`rust/crates/`)

All 27 workspace crates in [`rust/Cargo.toml`](rust/Cargo.toml) are organized across five layers:

### 1. Portable Runtime & Language ABI

| Crate | Role |
| --- | --- |
| [`pear-macho`](rust/crates/pear-macho) | Pure-Rust ARM64 Mach-O parser, export/fixup decoder, FairPlay `cryptid` inspector, `LC_BUILD_VERSION` macOS retargeter |
| [`pear-dyld`](rust/crates/pear-dyld) | Standalone multi-image Mach-O loader (`@rpath`, fixups, TLV, initializers) without host `dyld` |
| [`pear-libobjc`](rust/crates/pear-libobjc) | Clean-room Apple 64-bit Objective-C runtime (`abi`, `blocks`, `exceptions`, `message`, `object`, `props`, `reflection`, `runtime`) |
| [`pear-swift-rt`](rust/crates/pear-swift-rt) | Clean-room Swift 5 ABI runtime, metadata lookup, and ObjC interop (`_TtCs12_SwiftObject`) |
| [`pear-runtime`](rust/crates/pear-runtime) | Unified runtime facade & `pearruntime` / `pear-run` CLI orchestrating Darwin/C++ ABI, `libdispatch`, `Security`, `Accelerate`, and framework bridges |
| [`pear-core`](rust/crates/pear-core) | Thread-safe `[PearRuntime][<area>]` logging, ObjC forwarding hooks, missing-call tracker, `dladdr` symbolication, JSONL reports |

### 2. Portable Frameworks & Graphics

| Crate | Role |
| --- | --- |
| [`pear-foundation`](rust/crates/pear-foundation) | Pure-Rust CoreFoundation & Foundation (`NSString`, `NSArray`, `NSDictionary`, `NSData`, `NSURLSession`, `NSURLConnection`, `NSRunLoop`, `NSUserDefaults`) |
| [`pear-graphics`](rust/crates/pear-graphics) | Portable CoreGraphics bitmap contexts, colors, images, and basic glyph rasterizer with zero Apple deps |
| [`pear-image`](rust/crates/pear-image) | Portable PNG (including Apple `CgBI` variant) and JPEG decoders to RGBA |
| [`pear-metal-vk`](rust/crates/pear-metal-vk) | Metal ObjC API → Vulkan, `MTLB`/AIR → SPIR-V translator, Khronos ASTC decoder, live texture view aliasing, and scanline rasterizer |
| [`pear-keychain`](rust/crates/pear-keychain) | Local file-backed `SecItem*` / `SecKey*` Keychain store shared by `PearHost` and the portable runtime |

### 3. Host Platforms & Input

| Crate | Role |
| --- | --- |
| [`pear-host`](rust/crates/pear-host) | Platform-agnostic `HostPlatform` trait defining windowing, surfaces, events, and lifecycle |
| [`pear-host-macos`](rust/crates/pear-host-macos) | macOS `HostPlatform` on AppKit, QuartzCore, and IOKit, plus HUD, keymapper, and physical/virtual gamepad integration |
| [`pear-host-linux`](rust/crates/pear-host-linux) | Linux `HostPlatform` backend supporting X11/Wayland surfaces, XDG conventions, and run loop control |
| [`pear-controller`](rust/crates/pear-controller) | Pure-Rust low-latency HID/BLE gamepad engine (Switch 2 Joy-Cons BLE, DualSense, Xbox, Switch Pro/Joy-Con) |

### 4. Native macOS `PearHost` Frameworks

| Crate | Role |
| --- | --- |
| [`pear-objc`](rust/crates/pear-objc) | Minimal `libobjc.A.dylib` FFI & static ObjC ABI metadata emitters (class tables, `nsstring_constant!`, `@try`/`@catch` ASM frames) |
| [`pear-uikit`](rust/crates/pear-uikit) | Clean-room `UIKit.framework` backed by `pear-objc` and AppKit windowing/event routing |
| [`pear-metalkit`](rust/crates/pear-metalkit) | `UIView`-backed `MTKView` (`MetalKit.framework`) over `CAMetalLayer` |
| [`pear-gles`](rust/crates/pear-gles) | `EAGLContext` / `EAGLSharegroup` (`OpenGLES.framework`) over Metal-only `PearANGLE`, re-exporting `libGLESv2` |
| [`pear-glkit`](rust/crates/pear-glkit) | `GLKView` and `GLKViewController` (`GLKit.framework`) backed by `pear-gles` |
| [`pear-nib`](rust/crates/pear-nib) | Pure-Rust `NIBArchive` binary parser for nib/storyboard loading |
| [`pear-host-frameworks`](rust/crates/pear-host-frameworks) | Census-generated `PearHost` stub and overlay frameworks built by `host/build-host.sh` |
| [`pear-empty-framework`](rust/crates/pear-empty-framework) | Empty Mach-O framework images for iOS-only frameworks linked but uncalled by guest apps |
| [`pear-cxxcompat`](rust/crates/pear-cxxcompat) | C++ runtime compatibility shim (`libcxxcompat.dylib`) exporting symbols such as `std::__1::piecewise_construct` |
| [`pear-swift-support`](rust/crates/pear-swift-support) | Data symbols for the `aarch64-apple-ios-macabi` UIKit Swift overlay (`compat/swift/`) |

### 5. 32-bit Legacy & Dynamic Recompilation

| Crate | Role |
| --- | --- |
| [`pear-legacy-host`](rust/crates/pear-legacy-host) | `pear-host` adapter bridging windows, surfaces, and input queues for `PearLegacyEngine` |
| [`pear-jit`](rust/crates/pear-jit) | `PearLegacyJIT` dynamic execution planner and Dynarmic (`A32`/`A64` → `x86_64`/`arm64`) JIT bridge |

---

## Building

### Prerequisites

- **Rust toolchain** (`cargo`, 2021 edition)
- **macOS (Apple Silicon):** Xcode / Command Line Tools (macOS 26+ recommended for full `PearHost` framework builds)
- **Linux (ARM64 / x86_64):** Vulkan loader/headers and X11/Wayland development packages for windowed execution

### 1. Standalone Portable Runtime (`pearruntime`)

Build the standalone release runner:

```sh
cargo build --manifest-path rust/Cargo.toml --release -p pear-runtime --bin pearruntime
```

This produces `rust/target/release/pearruntime` (also defined as the `pear-run` binary target).

### 2. Unified macOS `PearHost` + `PearANGLE` + `PearLegacyEngine`

Run [`build-all.sh`](build-all.sh) to build the macOS `PearHost` frameworks, Metal-only [`PearANGLE`](third_party/angle), and 32-bit [`PearLegacyEngine`](third_party/legacy-engine) + [`PearLegacyJIT`](third_party/legacy-jit):

```sh
bash build-all.sh --sync-angle   # First time: sync & build Metal-only PearANGLE + PearHost + PearLegacyEngine
bash build-all.sh                # Incremental rebuild (pass --no-legacy to skip 32-bit engine)
```

Outputs are staged under `build/host/`:
- `UIKit.framework`, `MetalKit.framework`, `OpenGLES.framework`, `iAd.framework`
- `PearANGLE/` (`libEGL.dylib`, `libGLESv2.dylib`, `LICENSE.ANGLE`)
- `PearLegacyEngine/touchHLE`

---

## Usage

### Standalone `pearruntime` CLI

> **Note:** Pass the Mach-O binary or `.app` bundle path as the **first** argument before any inspection flags.

```sh
# Boot a 64-bit or 32-bit iOS .app bundle
rust/target/release/pearruntime path/to/App.app

# Summarize unresolved imports & unrealized ObjC classes
rust/target/release/pearruntime path/to/App.app --list-missing-summary
```

| Flag | Description |
| --- | --- |
| `--plan` | Detect guest architecture and print the `ExecutionPlan` summary without loading |
| `--inspect-only` | Inspect Mach-O headers, segments, dependencies, and exit |
| `--check-load` | Map the guest binary & `@rpath` dependencies via `pear-dyld` and verify loadability |
| `--list-missing` | Print every unresolved external symbol and unrealized ObjC class |
| `--list-missing-summary` | Print aggregated counts of loaded images, classes, unrealized classes, selectors, and missing symbols |
| `--window-test` | Open a minimal `HostPlatform` window to test surface initialization |

### Direct macOS `PearHost` IPA Workflow

Prepare an `.ipa` (retargeting `LC_BUILD_VERSION` without `/System/iOSSupport` or Mac Catalyst) and launch it directly on macOS:

```sh
tools/host-prepare.sh ~/Downloads/App.ipa build/host-apps/app.app
tools/host-run.sh build/host-apps/app.app build/host-apps/runs/latest 60
```

You can also enable **PearHost mode** per app inside [PearSilicon](https://github.com/denizomer10/PearSilicon).

---

## Testing

The 12 portable pure-Rust core crates (loader, ObjC/Swift runtime, Foundation, Graphics, Image, Metal-to-Vulkan, and CLI) run cleanly on both macOS and Linux via:

```sh
cargo test --manifest-path rust/Cargo.toml \
  -p pear-image -p pear-dyld -p pear-libobjc -p pear-foundation \
  -p pear-graphics -p pear-metal-vk -p pear-swift-rt -p pear-host \
  -p pear-host-linux -p pear-runtime -p pear-core -p pear-macho \
  --lib --bins --tests
```

---

## Diagnostics & Environment Variables

| Variable | Purpose |
| --- | --- |
| `PEAR_HEADLESS` | Run without creating a host GUI window (`1` to enable) |
| `PEAR_HEADLESS_FRAMES` | Number of frames to render in headless mode before exiting cleanly |
| `PEAR_DUMP_FRAME` | File path to write the rendered frame as a binary `P6` PPM image |
| `PEAR_NET_TRACE` | Log `NSURLSession`, `NSURLConnection`, `SecureTransport` (`SSL*`), `CFHTTPMessage`, and `Network.framework` activity (`1`) |
| `PEAR_METAL_TRACE` | Trace Metal command buffers, pipelines, textures, and draws |
| `PEAR_DYLD_TRACE` | Trace `pear-dyld` image loading, `@rpath` resolution, fixups, and initializers |
| `PEAR_LOG_FILE` | Mirror `[PearRuntime][<area>]` logs from `stderr` to a file |
| `PEAR_MISSING_REPORT` | Write JSONL (`first` + `summary`) unimplemented-selector/behavior reports for [`debug/`](debug) |
| `PEAR_SOFT_SELECTORS` | Allow missing Objective-C selectors to warn and return zero rather than trapping |
| `PEAR_STRICT_SELECTORS` | Fail fast on unimplemented selectors instead of soft-stubbing |
| `PEAR_BUNDLE_PATH` | Override guest `.app` bundle root path |
| `PEAR_CONTAINER_DIR` | Override sandbox container directory (`Documents/`, `Library/`, `tmp/`) |
| `PEAR_KEYCHAIN_DIR` | Override backing directory for `pear-keychain` persistence |

---

## Repository Structure

```text
PearRuntime/
├── rust/                Workspace Cargo.toml & 27 crates (crates/pear-*)
├── host/                macOS PearHost framework build scripts & ANGLE sync
├── tools/               IPA preparation, ABI census, stub generators & test probes
├── compat/              Swift ABI overlay sources (libswiftUIKit, stubs)
├── debug/               PearDebug log & JSONL missing-call analyzers
├── docs/                Architecture docs, Linux/Metal/Swift roadmap, census data
├── third_party/         PearANGLE, PearLegacyEngine, PearLegacyJIT (ARMv7 Dynarmic)
└── build-all.sh         Unified PearANGLE + PearHost + PearLegacyEngine build script
```

---

## The Pear Stack

| Project | Role |
| --- | --- |
| [PearSilicon](https://github.com/denizomer10/PearSilicon) | macOS Swift application: game library, installer, and launcher |
| **PearRuntime** | Clean-room Rust portable runtime, `PearHost` frameworks, and `PearLegacyEngine` bridge |
| [`PearANGLE`](third_party/angle) | Metal-only `arm64` ANGLE fork (`libEGL.dylib`, `libGLESv2.dylib`) |
| [`PearLegacyEngine`](third_party/legacy-engine) | 32-bit `armv6`/`armv7` iOS compatibility engine (Rust) |
| [`PearLegacyJIT`](third_party/legacy-jit) | ARMv7/ARM64 dynamic recompiler (Dynarmic fork) |
| [`PearDebug`](debug) | Logging and JSONL missing-call diagnostics |

---

## Documentation, Clean-Room Policy & Licensing

- **Architecture & Roadmap:** See [docs/linux-metal-swift-roadmap.md](docs/linux-metal-swift-roadmap.md) and [docs/independent-runtime.md](docs/independent-runtime.md) for clean-room boundaries and multi-stage runtime milestones.
- **Release History:** See [CHANGELOG.md](CHANGELOG.md).
- **Licensing:** PearRuntime's original Rust codebase is licensed under the [MIT License](LICENSE). Third-party components and notices are documented in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
- **Historical Archive:** Prior `PearLegacyEngine` and `PearLegacyJIT` commit history (referenced in `PearSilicon` releases up to v0.9.2) is preserved under the `archive/2026-11-22` tag (`https://github.com/denizomer10/PearRuntime/tree/<commit>`).
