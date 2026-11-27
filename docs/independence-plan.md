# PearRuntime Independence Plan

> **Current day-to-day state & commands:** See [`docs/HANDOFF.md`](HANDOFF.md). Clean-room policy and third-party license boundaries live in [`docs/independent-runtime.md`](independent-runtime.md) §2–3; Linux ARM64 Metal + Swift roadmap stages live in [`docs/linux-metal-swift-roadmap.md`](linux-metal-swift-roadmap.md).

**Status date:** 2026-11-27

**Goal:** PearRuntime as an independent, clean-room, Rust-first iOS compatibility runtime running 32-bit and 64-bit ARM iOS apps and games on **macOS** and **Linux**, with **zero `/System/iOSSupport`**, **zero Mac Catalyst**, and **zero dependency on Apple system frameworks** along the portable execution path.

---

## 1. Where the Runtime Stands Today (2026-11-27)

Three active execution paths share the unified workspace in [`rust/Cargo.toml`](../rust/Cargo.toml) (27 crates):

| Path | How Guest Code Runs | iOS / Darwin APIs Provided By | Status |
| --- | --- | --- | --- |
| **Standalone Portable Runtime (`pearruntime`)** | [`pear-dyld`](../rust/crates/pear-dyld) maps the guest `arm64` Mach-O & `@rpath` frameworks natively in process (zero `dyld`) | Clean-room pure Rust: [`pear-libobjc`](../rust/crates/pear-libobjc), [`pear-swift-rt`](../rust/crates/pear-swift-rt), [`pear-foundation`](../rust/crates/pear-foundation), [`pear-graphics`](../rust/crates/pear-graphics), [`pear-image`](../rust/crates/pear-image), [`pear-metal-vk`](../rust/crates/pear-metal-vk), [`pear-runtime`](../rust/crates/pear-runtime), [`pear-host`](../rust/crates/pear-host) | **Active primary track (Phases 1–5.19 landed):** `GenshinImpact.app` reports **0** missing symbols & **0** unrealized classes; 68/68 pure-Rust tests pass; boots Unity `il2cpp`, completes live HTTPS `query_region_list` (`200 OK`), and renders `1280×720` in-game UI |
| **Native macOS `PearHost` (`build/host/`)** | macOS `dyld` loads `LC_BUILD_VERSION`-retargeted Mach-Os on Apple Silicon (`tools/host-prepare.sh`) | [`pear-uikit`](../rust/crates/pear-uikit), [`pear-metalkit`](../rust/crates/pear-metalkit), [`pear-gles`](../rust/crates/pear-gles) (Metal-only [`PearANGLE`](../third_party/angle)), [`pear-glkit`](../rust/crates/pear-glkit), [`pear-host-macos`](../rust/crates/pear-host-macos), [`pear-controller`](../rust/crates/pear-controller), generated overlays | **Active macOS host path:** zero `/System/iOSSupport`; GTA: Vice City DE reaches gameplay, Minecraft loads worlds |
| **32-bit Legacy (`PearLegacyEngine`)** | Own Mach-O loader + Dynarmic ARMv7 JIT ([`third_party/legacy-engine`](../third_party/legacy-engine), [`third_party/legacy-jit`](../third_party/legacy-jit)) | `PearLegacyEngine` + [`pear-legacy-host`](../rust/crates/pear-legacy-host) + [`pear-jit`](../rust/crates/pear-jit) | **Active 32-bit path:** integrated into `build-all.sh` and `pearruntime`; Miku Flick boots and renders |

> **Retired:** The historical Mac Catalyst compatibility track (`pear-catalyst`, `pear-opengles`) has been completely retired and removed from the workspace.

---

## 2. Architecture & Dependency Inventory

### 2.1 Portable Zero-Apple-Dependency Stack (12 Core Crates)

These crates compile and pass unit/integration tests identically on **macOS** and **Linux ARM64** with zero `crates.io` dependencies in the pure-Rust core:

- **Loader & Binary Format:** [`pear-macho`](../rust/crates/pear-macho), [`pear-dyld`](../rust/crates/pear-dyld)
- **Language & Object Runtime:** [`pear-libobjc`](../rust/crates/pear-libobjc), [`pear-swift-rt`](../rust/crates/pear-swift-rt), [`pear-core`](../rust/crates/pear-core)
- **Apple API Layer:** [`pear-foundation`](../rust/crates/pear-foundation), [`pear-graphics`](../rust/crates/pear-graphics), [`pear-image`](../rust/crates/pear-image), [`pear-metal-vk`](../rust/crates/pear-metal-vk), [`pear-runtime`](../rust/crates/pear-runtime)
- **Host Boundary & Linux Backend:** [`pear-host`](../rust/crates/pear-host), [`pear-host-linux`](../rust/crates/pear-host-linux)

### 2.2 macOS-Native `PearHost` & Hardware Input Crates

- [`pear-objc`](../rust/crates/pear-objc) (static Mach-O ObjC class/constant/exception emission from Rust), [`pear-uikit`](../rust/crates/pear-uikit), [`pear-metalkit`](../rust/crates/pear-metalkit), [`pear-gles`](../rust/crates/pear-gles), [`pear-glkit`](../rust/crates/pear-glkit), [`pear-nib`](../rust/crates/pear-nib), [`pear-keychain`](../rust/crates/pear-keychain), [`pear-host-macos`](../rust/crates/pear-host-macos) (the sole AppKit/QuartzCore/IOKit adapter), [`pear-controller`](../rust/crates/pear-controller) (pure-Rust HID/BLE engine including Switch 2 Joy-Con BLE), [`pear-host-frameworks`](../rust/crates/pear-host-frameworks), [`pear-empty-framework`](../rust/crates/pear-empty-framework), [`pear-cxxcompat`](../rust/crates/pear-cxxcompat), [`pear-swift-support`](../rust/crates/pear-swift-support).

### 2.3 Non-Rust Components & Tooling

| Item | Language | Role |
| --- | --- | --- |
| [`third_party/angle`](../third_party/angle) | C++ / Metal / Vulkan | Metal-only `arm64` ANGLE for macOS (`libEGL.dylib`, `libGLESv2.dylib`) + Linux Vulkan backend config (`host/build-pearangle.sh`) |
| [`third_party/legacy-engine`](../third_party/legacy-engine) & [`third_party/legacy-jit`](../third_party/legacy-jit) | Rust / C++ | 32-bit iOS engine (`touchHLE` fork) + Dynarmic JIT |
| [`compat/swift/`](../compat/swift) | Swift | Swift ABI overlays (`libswiftUIKit.dylib`, stubs) |
| [`tools/`](../tools) | Python / Bash / Swift | Census (`ios-import-census.py`), ABI/behavior probes, IPA prep (`host-prepare.sh`), stub generation (`gen-uikit-stubs.py`, `gen-host-frameworks.py`) |
| [`debug/`](../debug) | Swift | `PearDebug` structured log and `PEAR_MISSING_REPORT` JSONL analyzer |

### 2.4 Darwin-on-Linux ABI Requirements (Tracked in `pear-runtime`)

Running native `arm64` iOS binaries on non-Darwin OSes (Linux ARM64) requires bridging several ABI boundaries in `pear-dyld` and `pear-runtime`:
1. **Variadic calling convention:** Darwin `arm64` passes variadic arguments on the stack; Linux AAPCS64 passes them in registers (`printf`, `open`, `fcntl`, `ioctl`, `sysctl` shims).
2. **`long double`:** 64-bit on Darwin `arm64` vs. 128-bit on Linux `aarch64`.
3. **TLS / user-space ceiling:** Darwin reads `TPIDRRO_EL0` / reserves `x18`, while `objc4` assumes tagged user pointers below `2^47`. Linux `aarch64` maps high in 48-bit VA space, so `pear-dyld` deliberately maps guest images below `2^47`.
4. **Struct layouts & constants:** `stat`, `dirent`, `sockaddr` (`sa_len`), `errno`, signal numbers, and `fcntl`/`O_*` flags are translated to Darwin layouts at the `pear-runtime` boundary.
5. **Unwind & C++ EH:** Guest `__unwind_info` / `__eh_frame` sections are invisible to host `libunwind`/`libgcc` unless explicitly registered — handled by `pear-runtime/src/unwind.rs` and `pear-libobjc/src/exceptions.rs`.
6. **Page size:** iOS uses 16 KiB pages; Linux ARM64 often uses 4 KiB (`pear-dyld` and `vm_page_size` align cleanly across both).

---

## 3. Host Abstraction Boundary (`pear-host`)

`pear-host` defines `HostPlatform` (`initialize`, `screen`, `create_window`, `set_root_layer`, `show_window`, `display_link`, `open_url`, `set_idle_timer_disabled`, `present_alert`, `run`, `pasteboard_*`, `pick_documents`, `share`, `screen_reader_running`, `surface`) and the `GuestEvents` sink (`TouchPhase`, `KeyEvent`, `EditCommand`, `Lifecycle`), along with `NullHost` for tests and headless execution:

- **macOS (`pear-host-macos`):** Implements `HostPlatform` on AppKit/QuartzCore/IOKit (`MacHost`, `PearHostView`, `PearBackend*` exports) with zero `PearUIKit*` symbol coupling back into `pear-uikit`.
- **Linux (`pear-host-linux`):** Implements `HostPlatform` over dynamic `libX11.so.6` / Wayland (`NativeSurface::Xlib` / `Wayland`), `PEAR_HOST_SCREEN`, XDG paths, `xdg-open`, and headless fallback.
- **32-bit Legacy (`pear-legacy-host`):** Adapts `HostPlatform` windows, surfaces, and input queues for `PearLegacyEngine`.

---

## 4. Prioritized Milestones (M1–M10 + Phases 1–5)

| Milestone | Scope | Outcome |
| --- | --- | --- |
| **M1** | Inventory, app census (`docs/census/*.json`), baseline (`docs/runs/baseline-2026-11-21.md`) | **Done** |
| **M2** | Retire Mac Catalyst preparation/runtime (`pear-catalyst`, `pear-opengles` removed) | **Done** |
| **M3** | `pear-host` trait, `GuestEvents` sink, `NativeSurface`, AppKit isolation in `pear-host-macos` | **Done** (Steps 1–5) |
| **M4** | CoreText-only `UIFont`, `UIFoundation` re-export, pure-Rust `pear-image` (PNG/CgBI/JPEG) & `pear-graphics` | **Done** (Steps 1–3) |
| **M5** | Vulkan backend selection in `PearANGLE` + `NativeSurface::egl_window()` bridge | **Done** |
| **M6** | Own 64-bit Mach-O loader (`pear-macho` + `pear-dyld`: chained fixups, re-exports, `@rpath` image sets, TLV) | **Done** |
| **M7** | Own 64-bit Apple-ABI ObjC runtime (`pear-libobjc`: lock-free ARM64 msg cache, ARC, blocks, DWARF LSDA EH, reflection) | **Done** |
| **M8** | Guest `Foundation` / `CoreFoundation` (`pear-foundation`: collections, URL/networking, run loops, defaults, file manager) | **Done** |
| **M9** | Linux host backend (`pear-host-linux`) & unified runtime (`pear-runtime` / `pearruntime` CLI) | **Done** |
| **M10** | Host portability (`Linux-first`; Windows cancelled by user decision on 2026-11-21; x86-64 A64 Dynarmic translation via `pear-jit`) | **Linux-first infrastructure ready; x86-64 A64 ongoing** |

### Phases 1–5 (Genshin Impact v7.2.0 Verification Track)

- **Phase 1:** Multi-image `@rpath` bundle loader (`pear-dyld`), `apple_libcxx` (`std::__1`), compact unwind registration, Darwin/C stubs → **Done**.
- **Phase 2:** Clean-room Metal → Vulkan + `MTLB`/AIR → SPIR-V + Khronos ASTC LDR decoder + scanline fallback (`pear-metal-vk`) → **Done**.
- **Phase 3:** Clean-room Swift 5 ABI runtime & ObjC interop (`pear-swift-rt`) → **Done**.
- **Phase 4:** Portable `Security`, `CoreGraphics`, `CFNetwork`, `Accelerate`, `Audio`, `CoreMotion`, `GameController`, `UIKit` bridges → **`0` missing symbols on `GenshinImpact.app` (`--list-missing-summary`)** → **Done**.
- **Phase 5.1–5.19:** Soft/strict selector infrastructure, real `libdispatch` sync primitives, `SceneDelegate` lifecycle, live `RwLock` texture view aliasing, ARC `newTextureViewWithPixelFormat:` ownership fix, division-free monomorphized scanline rasterizer, real curl-backed `NSURLSession` / `NSURLConnection` HTTP/HTTPS stack + `PEAR_NET_TRACE=1`, and `NSDictionary keyEnumerator` deadlock fix → **Done**.
- **Phase 5.20+ (Next):** Resolve post-dispatch `4204` / `SecKey` RSA payload decryption (`pear-runtime/src/security.rs`), accelerate 1:1 opaque blit scanlines in `pear-metal-vk`, and complete Linux ARM64 Vulkan window execution.

---

## 5. Validation Matrix

| Check | Command / Tool | Latest Verified Result |
| --- | --- | --- |
| Portable 12-crate pure-Rust suite | `cargo test --manifest-path rust/Cargo.toml -p pear-image -p pear-dyld -p pear-libobjc -p pear-foundation -p pear-graphics -p pear-metal-vk -p pear-swift-rt -p pear-host -p pear-host-linux -p pear-runtime -p pear-core -p pear-macho --lib --bins --tests` | **68 / 68 tests passing** (`0` failures, `0` warnings) |
| Standalone CLI build | `cargo build --manifest-path rust/Cargo.toml --release -p pear-runtime --bin pearruntime` | **Builds cleanly** |
| Missing symbol & class audit | `pearruntime GenshinImpact.app --list-missing-summary` | `images=3`, `classes=1472`, `unrealized=0`, `categories=34`, `selectors=12556`, **`missing symbols (0)`** |
| End-to-end headless & network probe | `PEAR_NET_TRACE=1 PEAR_HEADLESS=1 PEAR_HEADLESS_FRAMES=450 PEAR_DUMP_FRAME=out.ppm pearruntime GenshinImpact.app` | Boots Unity `il2cpp`, completes `query_region_list` HTTPS `200 OK` (`3020 B`) → `OnConnectGlobalDispatch`, dumps `1280×720` PPM UI (`1,254` distinct colors) |
| Native macOS `PearHost` probes | `tools/host-abi-census.py`, `tools/host-behavior-probe.py`, `tools/host-event-probe.py`, `tools/host-image-probe.py` | Verified across `build/host/` (see [`docs/runs/baseline-2026-11-21.md`](runs/baseline-2026-11-21.md)) |
| 32-bit Legacy path | `build-all.sh` + `PearLegacyEngine` | Miku Flick boots & runs |
