# PearRuntime — Development Handoff

**Last updated:** 2026-11-27 (post-Phase 5.19; ready for **Phase 5.20**)

Start here for active engineering work. Architecture & historical context live in:
- [`docs/independence-plan.md`](independence-plan.md) (full dependency inventory, host boundary, M1–M10 + Phases 1–5.19)
- [`docs/linux-metal-swift-roadmap.md`](linux-metal-swift-roadmap.md) (5-layer ARM64 Metal + Swift roadmap, Turkish)
- [`docs/independent-runtime.md`](independent-runtime.md) (clean-room & licensing rules, Turkish)
- [`docs/HANDOFF-2026-11-14.md`](HANDOFF-2026-11-14.md) (archived pre-Porting `PearHost` history: GTA VC DE, Minecraft)

---

## 1. Non-Negotiable Ground Rules

- **Clean-room only:** Follow [`docs/independent-runtime.md`](independent-runtime.md) §2–3 strictly. Never disassemble `/System/iOSSupport` or `dyld_shared_cache`, never commit leaked or decompiled material, and cite public docs/specs or observed app behavior in module headers.
- **Rust first & zero `crates.io` deps in core runtime:** Keep the 12 pure-Rust runtime crates free of external `crates.io` dependencies. No game-specific hacks — fix underlying ABI, framework, or Metal/Foundation semantics cleanly and add targeted unit tests.
- **Never commit `rust/Cargo.lock`:** An untracked/dirty pre-existing `M rust/Cargo.lock` may exist in the working tree; stage ONLY modified source/doc files (`git add <paths>`).
- **Never delete `archive/2026-11-22`:** That tag preserves the pre-monorepo commit history of `PearLegacyEngine` and `PearLegacyJIT` referenced by older `PearSilicon` releases.
- **Never run `pearruntime` under `lldb`** and never capture the user's whole screen (window-only probes via [`tools/host-winid.swift`](../tools/host-winid.swift)).

---

## 2. Active Workspace Map (27 Crates)

| Layer | Crates (`rust/crates/`) | Notes |
| --- | --- | --- |
| **Loader & ABI** | `pear-macho`, `pear-dyld`, `pear-libobjc`, `pear-swift-rt`, `pear-runtime`, `pear-core` | Pure-Rust `arm64` Mach-O `@rpath` loader, Apple 64-bit ObjC ABI, Swift 5 interop, Darwin/C++ ABI, `pearruntime` CLI |
| **Frameworks & Graphics** | `pear-foundation`, `pear-graphics`, `pear-image`, `pear-metal-vk`, `pear-keychain` | Foundation + curl-backed `NSURLSession`/`NSURLConnection`, PNG (`CgBI`)/JPEG decoders, Metal → Vulkan + `MTLB`/AIR → SPIR-V + Khronos ASTC + scanline rasterizer, Keychain |
| **Host Platforms & Input** | `pear-host`, `pear-host-macos`, `pear-host-linux`, `pear-controller` | `HostPlatform` trait, AppKit backend (`MacHost`), Linux X11/Wayland backend, BLE/HID gamepads |
| **Native `PearHost` (`build/host/`)** | `pear-objc`, `pear-uikit`, `pear-metalkit`, `pear-gles`, `pear-glkit`, `pear-nib`, `pear-host-frameworks`, `pear-empty-framework`, `pear-cxxcompat`, `pear-swift-support` | Zero-ObjC macOS `PearHost` frameworks + Metal-only `PearANGLE` (`third_party/angle`); note `pear-catalyst` & `pear-opengles` were removed |
| **32-bit Legacy & JIT** | `pear-legacy-host`, `pear-jit` | `PearLegacyEngine` (`third_party/legacy-engine`) + Dynarmic JIT (`third_party/legacy-jit`) |

---

## 3. Verified State at End of Phase 5.19 (`8a85b96`)

1. **12-Crate Pure-Rust Test Suite:** **68 / 68 tests pass** (`0` failures, `0` new warnings).
2. **`GenshinImpact.app` (`v7.2.0`) Missing-Symbol Audit (`--list-missing-summary`):**
   - `images=3`, `classes=1472`, **`unrealized=0`**, `categories=34`, `selectors=12556`, **`missing symbols (0)`**.
3. **Headless Boot, Networking & Frame Capture (`PEAR_NET_TRACE=1`):**
   - Boots Unity `il2cpp` past `LogoPageContext` into `Home LoadScene` → `LoginMainPageContext OnAgreement/OnPreInit/InitServerDispatch` → `NetworkManager ConnectGlobalDispatchServer`.
   - `NSURLConnection` completes real HTTPS `GET https://dispatchosglobal.yuanshen.com/query_region_list?...` (`status=200`, `bytes=3020`) and triggers `NetworkManager OnConnectGlobalDispatch`.
   - Renders `1280×720` in-game UI dialog (`distinct_colors=1254`): displays `"Network error / Failed to connect to the server 4204 / Confirm"`.
4. **Key Recent Milestones (Phases 5.15 – 5.19):**
   - **5.15 (`22ac940`):** Fixed ARC `+1` UAF in `-[PearMTLTexture newTextureViewWithPixelFormat:]` + view variants & `default_device_singleton()`.
   - **5.16 (`761b6c6`):** Live `Arc<RwLock<Vec<u8>>>` texture view aliasing, `DrawTransform::{Matrix, RawNdc, Reject}` clip guards, division-free monomorphized `rasterize_textured_span<POT, BLEND, MODULATE>`.
   - **5.17 (`d7bb7f7`):** Real `NSURLSession` / `NSURLSessionTask` / `NSHTTPURLResponse` / `NSError` HTTP/HTTPS stack in [`pear-foundation/src/url_session.rs`](../rust/crates/pear-foundation/src/url_session.rs).
   - **5.18 (`801a9bd`):** Real `NSURLConnection` (delegate / sync / async) + `PEAR_NET_TRACE=1` telemetry across `NSURLSession`, `NSURLConnection`, `SecureTransport`, `CFHTTPMessage`, and `Network.framework`.
   - **5.19 (`8a85b96`):** Fixed `store()` self-deadlock in `NSDictionary keyEnumerator` & `enum_all_objects` AB/BA lock ordering in [`pear-foundation/src/lib.rs`](../rust/crates/pear-foundation/src/lib.rs).

---

## 4. Exact Build, Test & Probe Commands

### Run the 12-Crate Pure-Rust Test Suite
**Do NOT use `cargo test --workspace`** (that pulls in macOS framework targets and fails in test harnesses). Always test the 12 portable crates explicitly:

```sh
cargo test --manifest-path rust/Cargo.toml \
  -p pear-image -p pear-dyld -p pear-libobjc -p pear-foundation \
  -p pear-graphics -p pear-metal-vk -p pear-swift-rt -p pear-host \
  -p pear-host-linux -p pear-runtime -p pear-core -p pear-macho \
  --lib --bins --tests
```

### Build & Run Standalone `pearruntime`
> **CLI argument rule:** Pass the `.app` bundle or Mach-O path **before** any `--` flag.

```sh
cargo build --manifest-path rust/Cargo.toml --release -p pear-runtime --bin pearruntime

# Missing symbol/class check
rust/target/release/pearruntime build/host-apps/GenshinImpact.payload/Payload/GenshinImpact.app --list-missing-summary

# Headless boot + network trace + PPM frame dump (use a bounded timeout when probing UI loops)
PEAR_NET_TRACE=1 PEAR_HEADLESS=1 PEAR_HEADLESS_FRAMES=450 PEAR_DUMP_FRAME=/tmp/genshin.ppm \
  rust/target/release/pearruntime build/host-apps/GenshinImpact.payload/Payload/GenshinImpact.app
```

### Unified `PearHost` + `PearANGLE` + `PearLegacyEngine` Build & Probes
```sh
bash build-all.sh [--sync-angle] [--no-legacy]
python3 tools/host-behavior-probe.py build/host --compare <prev.json>
python3 tools/host-event-probe.py build/host
python3 tools/host-abi-census.py build/host --compare <prev.json>
```

---

## 5. Next Priorities (Phase 5.20+)

1. **Unblock Post-Dispatch Error `4204` (`OnConnectGlobalDispatch` → `query_cur_region`):**
   - Inspect why Genshin Impact displays `4204` right after `query_region_list` succeeds (`200 OK`, `3020 B`).
   - Top suspect: [`rust/crates/pear-runtime/src/security.rs`](../rust/crates/pear-runtime/src/security.rs) (`sec_key_copy_passthrough` currently copies bytes verbatim instead of performing real RSA decrypt/encrypt via `SecKeyRawDecrypt` / `SecKeyCreateDecryptedData`), or subsequent HTTPS/JSON dispatch handling after `query_region_list`.
2. **Accelerate Software Rasterization & Advance Hardware Vulkan Rendering (`pear-metal-vk`):**
   - `sample` profiling shows `UnityGfxDeviceWorker` spends ~90% of CPU inside `rasterize_textured_span` (`objc_metal.rs`). Add a 1:1 opaque scanline fast-path (`du_fp == 65536 && dv_fp == 0 && !BLEND && !MODULATE`) and optimize clear/blit passes, alongside hardware Vulkan / Lavapipe shader execution.
3. **Linux ARM64 End-to-End Verification:**
   - Re-verify `pearruntime` windowed Vulkan/X11 execution on ARM64 Linux (`pear-host-linux` + Mesa `lavapipe` / GPU driver).

---

## 6. Hard-Won Technical Notes

- **47-bit User VA Space (`objc4` compatibility):** Linux `aarch64` maps near `2^48`, whereas Apple Objective-C class-pointer bitmasks strip bits `>= 47`. [`pear-dyld`](../rust/crates/pear-dyld) intentionally maps images below `2^47`; preserve this invariant.
- **No `sdiv` Inside Pixel Loops:** In [`pear-metal-vk/src/objc_metal.rs`](../rust/crates/pear-metal-vk/src/objc_metal.rs), monomorphized `#[inline(never)] rasterize_textured_span<POT, BLEND, MODULATE>` functions avoid LLVM emitting hardware `sdiv` instructions per pixel; inspect generated ARM64 (`otool -tV`) if modifying the rasterizer inner loop.
- **Lock Ordering Around `pear-foundation` Store:** Never allocate NSObjects (`nsstring_from_rust`, `alloc_with_backing`) while holding `store().lock()`; snapshot keys/values and drop the store mutex first (`Phase 5.19`).
- **Zsh `multios` Pitfall:** `cmd 2>&1 >/dev/null | grep ...` pipes stdout too under zsh; redirect explicitly to a log file when capturing traces.
