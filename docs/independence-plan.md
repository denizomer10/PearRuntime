# PearRuntime independence plan

> Picking this work up? Start with `docs/HANDOFF.md` (state, open work, how to verify).

Goal: PearRuntime as an independent, Rust-first iOS compatibility runtime that
runs 32-bit and 64-bit ARM iOS apps and games on macOS, Linux and Windows,
with no `/System/iOSSupport`, no Mac Catalyst and no macOS framework behind the
compatibility layer. This file is the inventory, the boundary and the plan; it
is updated as milestones land. Nothing here is "independent" until a check in
§6 shows it.

Status date: 2026-11-21 (branch `pearhost-rust-completion`).

## 1. Where the runtime is today

Three execution paths exist:

| Path | What runs the app | iOS API provided by | Status |
|---|---|---|---|
| **PearHost** (64-bit) | macOS dyld loads the retargeted Mach-O natively on Apple silicon | PearRuntime's own UIKit / OpenGLES / MetalKit + generated stub and overlay frameworks (Rust); Foundation, CoreFoundation, libobjc, CoreGraphics, Metal… are **macOS's** | Default launch path for 64-bit apps in the launcher since 2026-11-20 |
| **Mac Catalyst** (64-bit, legacy) | macOS dyld, Catalyst platform | Apple's UIKit from `/System/iOSSupport` + `libPearRuntime.dylib` (pear-catalyst) patching it | Being retired: no longer used by the launcher for 64-bit apps; code still present |
| **PearLegacyEngine** (32-bit) | touchHLE fork: own Mach-O loader, Dynarmic A32 JIT, own Objective-C runtime and framework emulation, SDL2, OpenAL Soft, ANGLE | Its own Rust implementation | Works (Miku Flick); separate repository, prebuilt binary in the shipped app |

The 32-bit engine is already structurally independent (own loader, own ObjC
runtime, own frameworks, SDL host layer); the 64-bit PearHost path is not yet:
it owns UIKit but borrows Apple's ObjC runtime, Foundation, CoreFoundation,
dyld and graphics stack.

## 2. Dependency inventory

### 2.1 Non-Rust sources

| Item | Language | Role | Class (§3) |
|---|---|---|---|
| `compat/swift/PearSwiftUIKit.swift`, `PearUIKitNumeric.swift` | Swift | UIKit Swift overlay ABI (`libswiftUIKit.dylib`): IndexPath/numeric conformances iOS Swift apps import | External/ABI — Swift metadata cannot be emitted by hand from Rust safely |
| `compat/swift/MarketplaceKit.swift` | Swift | MarketplaceKit stub (Catalyst path) | Catalyst-only, retire with it |
| `third_party/angle` (PearANGLE fork) | C++ | GLES → Metal | External component |
| PearLegacyEngine (separate repo) | Rust + C/C++ (SDL2, OpenAL Soft, Dynarmic) | 32-bit engine | External component |
| `tools/*.py`, `*.sh` | Python/Bash | build, census, preparation tooling | Build-time only |
| PearSilicon launcher (`Sources/`) | Swift/SwiftUI | installer, preparer, launcher UI | Host application, not runtime |
| `scripts/*-smoke*.mm` (PearSilicon) | ObjC++ | test fixtures standing in for iOS app code | Tests only |

The runtime crates themselves have no Objective-C, C or assembly source files;
Objective-C ABI data and `@try` frames are emitted from Rust (`pear_objc::abi`).

### 2.2 Apple frameworks and system libraries, per crate

| Crate (lines) | Path | Links / messages |
|---|---|---|
| pear-objc (842) | all | **libobjc**, Foundation (every `msg*` call is Apple's ObjC runtime) |
| pear-core (497) | PearHost | libobjc (forwarding / missing-call reporter) |
| pear-uikit (13 170) | PearHost | **AppKit** (NSApplication, NSWindow, NSView, NSScreen, NSAlert, NSMenu, NSPasteboard, NSWorkspace, NSFont…: ~33 call sites, nearly all in `app.rs` behind `PearBackend*`), **QuartzCore** (CALayer tree, CADisplayLink, CATransaction), **CoreGraphics/CoreText/ImageIO** (~150 calls in `graphics.rs`: UIImage, UIFont, drawing), **Foundation** (NSArray, NSBundle, NSNotificationCenter, NSData, NSURL… used as the guest's own Foundation) |
| pear-gles (819) | PearHost | ANGLE (libEGL/libGLESv2), CALayer as the EGL native window |
| pear-metalkit (729) | PearHost | **Metal**, CAMetalLayer (MTKView) |
| pear-keychain (841) | both | **Security**, CoreFoundation (SecKey crypto) |
| pear-host-frameworks (generated) | PearHost | re-exports AVKit, GameController, StoreKit, MediaPlayer, SafariServices, Social, GLKit from macOS (overlays) |
| pear-catalyst (19 681) | Catalyst | Apple UIKit (iOSSupport), AppKit, Metal, GameController, AVFoundation… |
| pear-opengles (188) | both | ANGLE (forwarders) |
| pear-macho, pear-nib, pear-cxxcompat, pear-empty-framework, pear-swift-support | — | none (portable) |

### 2.3 Dependencies that are not visible as links

These matter most for Linux/Windows and are easy to miss:

1. **dyld** loads every 64-bit app image, resolves its imports against the
   PearHost frameworks and macOS libraries, runs initializers, and applies
   interposing. Linux/Windows need PearRuntime's own Mach-O loader and binder
   (pear-macho is the parser; the 32-bit engine has a loader).
2. **libobjc** (Apple's Objective-C runtime) runs all guest Objective-C:
   class realization, message dispatch, ARC, exceptions, associated objects.
3. **Foundation + CoreFoundation** (macOS's) *are* the guest's Foundation:
   NSString, collections, run loops, NSFileManager, NSUserDefaults,
   NSURLSession… iOS apps call them directly. This is guest-facing API served
   by the host OS, the largest single macOS dependency.
4. **libSystem / Darwin syscalls** (pthreads, mmap, files, sockets): native
   on macOS, need a translation layer elsewhere.
5. **CPU**: arm64 code runs natively on Apple silicon; Linux arm64 can run it
   natively once loaded; x86-64 hosts need A64 translation (Dynarmic supports
   A64 as well as A32).
6. **The macOS overlays re-export system frameworks** (AVKit, GameController,
   StoreKit, …): on macOS the iOS API surface they share is Apple's.

### 2.4 iOSSupport / Mac Catalyst

- `pear-catalyst`, `build.sh`'s `libPearRuntime.dylib`, `libPearOpenGLES`
  (pear-opengles `catalyst` feature), the Swift overlay build (macabi), and the
  launcher's Catalyst preparation (`PackageInstaller`) exist only for the
  Catalyst path.
- PearHost never loads `/System/iOSSupport` (the preparers rewrite every such
  path; verified with `DYLD_PRINT_LIBRARIES` for GTA VC earlier). Note: the
  installer still converts newly installed apps for Catalyst first; PearHost
  then re-prepares them. M2 below removes that step.

## 2.5 Research: what exists to build on (2026-11-21)

Checked with the GitHub API (licence, last push) and web search. "Use" means
code may enter PearRuntime under its licence; "reference" means study only.

| Component | Candidate | Licence | State | Fit |
|---|---|---|---|---|
| Kernel semantics (syscalls, Mach traps, errno, struct layouts, signals) | apple-oss-distributions/**xnu** | APSL 2.0 | active (2026-06) | Reference for Darwin ABI facts our libSystem layer must reproduce on Linux (stat/dirent/sockaddr layouts, errno and signal numbers, `mach_absolute_time`, Mach ports apps touch) |
| Loader / binder | apple-oss-distributions/**dyld** | APSL 2.0 | active | Reference for chained fixups, binding, initializers, TLV; our loader builds on pear-macho |
| Objective-C runtime, **Apple ABI** | apple-oss-distributions/**objc4** | APSL 2.0 | active | The definitive Apple-ABI runtime (class_ro_t, `__objc_classlist`, `objc_msgSend`, ARC, tagged pointers). Port or reference; iOS binaries need this ABI |
| Objective-C runtime | gnustep/**libobjc2** | MIT | active | **Does not implement Apple's ABI** (GNUstep v2 ABI): cannot run iOS binaries as is. Useful for EH/blocks/ARC algorithms only |
| Objective-C runtime in Rust | touchHLE / **PearLegacyEngine** (`objc` module) | MPL-2.0 | active | Reads Apple-ABI metadata from Mach-O in Rust (32-bit). Closest to a Rust-first Apple-ABI runtime; extend to 64-bit layouts |
| CoreFoundation | apple-oss-distributions/**CF** (CF-Lite) | APSL 2.0 | 2021 | Portable C CF; also inside swiftlang/**swift-corelibs-foundation** (Apache-2.0, active) which builds CF on Linux/Windows |
| Foundation | swift-corelibs-foundation | Apache-2.0 | active | Swift-native classes, **not** Objective-C: iOS code expecting ObjC `NSString` etc. cannot use it directly. Reference for semantics |
| Foundation (ObjC) | darlinghq/darling-foundation (from Apportable) | LGPL-2.1 | active | ObjC Foundation on CF; LGPL (dynamic linking only) |
| Foundation (ObjC) | gnustep/libs-base | GPL-2.0 (LGPL lib) | active | GNUstep ABI, GPL family |
| libdispatch / blocks | swift-corelibs-libdispatch, apple **libdispatch**, **libclosure** | Apache-2.0 / APSL | active | GCD on Linux exists (Apache-2.0): reusable |
| libc / pthreads / malloc | apple **Libc**, **libpthread**, **libmalloc** | APSL | active | Reference for Darwin-specific behaviour; on Linux the shim maps to glibc/musl |
| Whole Darwin userland on Linux | darlinghq/**darling** | GPL-3.0 | very active; **initial arm64 support** (arm64 macOS games load and open windows on arm64 Linux); experimenting with Dynarmic for arm64-on-x86-64 | Proof that native arm64 Darwin binaries can run on arm64 Linux. GPL-3.0: reference only unless PearRuntime adopts GPL |
| Metal on Vulkan | darlinghq/**indium** (with **Iridium**: AIR → SPIR-V) | **0BSD** | active (2026-11) | Use: no Foundation/ObjC dependency by design; an ABI-compatible Metal (MTLDevice…) layer on top is ours to write. Also `metal2vulkan` (Rust crate, AIR → Vulkan SPIR-V) |
| GLES | google/**angle** (PearANGLE fork) | BSD | active | Re-enable the Vulkan backend in the fork for Linux/Windows |
| iOS on Windows | microsoft/**WinObjC** (archived 2022), ipasimulator/**ipasim** (2023) | MIT | inactive | Reference: UIKit/Foundation on Windows (WinObjC), emulated iOS with translated system calls (ipasim) |
| 32-bit iOS | touchHLE / PearLegacyEngine | MPL-2.0 (engine binary GPL-3.0 via linked libs) | active | Already independent; integrate |
| CPU translation (non-arm64 hosts) | Dynarmic (azahar-emu fork, 0BSD; A32 + A64) | 0BSD | active | arm64 guests on x86-64 Linux/Windows; A32 already used by the 32-bit engine. FEX/box64 go the other way (x86 → arm) |

Darwin-on-Linux ABI facts that bite (from xnu/dyld and Darling's experience),
all of which the libSystem layer has to handle:

- **Variadic calls**: Darwin arm64 passes variadic arguments on the stack;
  Linux AAPCS64 passes them in registers. Every variadic libc entry an app
  calls (`printf`, `open`, `fcntl`, `ioctl`…) needs a Darwin-convention shim.
- **`long double`** is 64-bit on Darwin arm64, 128-bit on Linux aarch64.
- **TLS / thread pointer**: Darwin reads thread-specific data via
  `TPIDRRO_EL0`, Linux uses `TPIDR_EL0`; Darwin also reserves `x18`.
- **Struct layouts and constants** differ: `stat`, `dirent`, `sockaddr`
  (`sa_len`), `errno` values, signal numbers, `fcntl`/`open` flags, `O_*`.
- **Raw syscalls** (`svc #0x80`, number in `x16`) from apps or bundled
  libraries need trapping (Linux syscall user dispatch, as Darling uses).
- **Mach**: `mach_absolute_time`, `mach_task_self`, ports used by run loops
  and dispatch; our CF/run loop must not depend on real Mach IPC.
- **Page size**: iOS uses 16 KiB pages; Linux arm64 is often 4 KiB (mapping
  works; `vm_page_size` and allocator assumptions must agree).

### What is realistically reachable

- **32-bit iOS on macOS/Linux/Windows**: PearLegacyEngine already has the
  architecture (own loader, ObjC runtime, frameworks, Dynarmic, SDL); the work
  is integration and sharing PearANGLE and `pear-host`.
- **64-bit iOS on arm64 Linux, natively**: reachable; Darling shows Darwin arm64
  binaries running on arm64 Linux. Needs our loader, Darwin-ABI libSystem
  layer, an Apple-ABI ObjC runtime, CF + an Objective-C Foundation, our UIKit,
  ANGLE-Vulkan and an Indium-based Metal. Foundation and Metal are the largest
  pieces.
- **64-bit iOS on x86-64 Linux/Windows**: needs arm64 translation as well
  (Dynarmic A64 or whole-process translation); slowest and last.
- **Swift apps**: the Swift runtime is open source but must be the Darwin-ABI
  build (Objective-C interop on); Darling builds it that way.

## 3. Classification

**Portable compatibility logic (Rust, keep growing):** UIKit object model and
behaviour (pear-uikit except the AppKit backend and CG drawing), nib loading
(pear-nib), Mach-O parsing (pear-macho), the keychain store's item model,
EAGL state machine and GLES ABI (pear-gles, pear-opengles), the missing-call
reporter (pear-core), Objective-C ABI emission (pear_objc::abi), generated
stub/overlay frameworks.

**Replaceable host services (behind a backend):** windows, layers and
presentation surfaces (AppKit/CALayer), display link / vsync (CADisplayLink),
run loop integration (NSApplication), input (NSEvent → touches, keys,
gamepads via GameController), alerts, URL opening, pasteboard, screen info,
text rendering and image decoding (CoreText/ImageIO/CoreGraphics), crypto
(Security), audio.

**External components needing their own migration:** the Objective-C runtime
(libobjc), Foundation/CoreFoundation, dyld/Mach-O binding, Darwin syscall
surface, CPU translation for non-arm64 hosts (Dynarmic, PearLegacyJIT), PearANGLE
graphics backends (Metal today; Vulkan/D3D needed), PearLegacyEngine (already
independent; integrate rather than rewrite), Swift runtime for Swift apps.

## 4. Rust boundary for platform-neutral services

**Implemented (M3, step 1): crate `pear-host`** (no Apple dependency; builds
and passes its tests on Linux aarch64). It holds platform-neutral types
(`Size`, `Rect`, `ScreenInfo`, opaque `WindowHandle`, `LayerRef`,
`GuestObject`, `GuestSelector`), the `HostPlatform` trait, the process
registry (`install`, `host()`) and `NullHost` for tests and headless runs:

```rust
pub trait HostPlatform: Send + Sync {
    fn initialize(&self);
    fn screen(&self) -> ScreenInfo;
    fn create_window(&self, owner: GuestObject, frame: Rect) -> WindowHandle;
    fn set_root_layer(&self, window: WindowHandle, layer: LayerRef);
    fn show_window(&self, window: WindowHandle, key: bool);
    fn display_link(&self, target: GuestObject, selector: GuestSelector) -> GuestObject;
    fn open_url(&self, url: &str) -> bool;
    fn set_idle_timer_disabled(&self, disabled: bool);
    fn present_alert(&self, title: &str, message: &str, buttons: &[String],
                     reply: Box<dyn FnOnce(isize) + Send>);
    fn run(&self, did_finish_launching: Box<dyn FnOnce()>);
}
```

**Implemented (M3, step 2): crate `pear-host-macos`**, the only PearHost
crate that names AppKit for windows, the run loop and input: `MacHost`, the
`PearBackend*` functions (still exported from UIKit.framework for the C
ABI), display-link compatibility, IOKit power assertions, and the AppKit side
of input (PearHostView, window/application delegates, Mac key codes → HID
usages) delivering to UIKit's `PearUIKitDeliver*` / lifecycle entry points.
pear-uikit depends on it only on macOS and calls `pear_host_macos::install()`
from its image initializer; everything else goes through `pear_host::host()`.

*M3 step 3 (2026-11-21):* the pasteboard (`PasteboardItem` Text/Url/Data,
`pasteboard_read/_types/_data/_write/_change_count`), document picker
(`DocumentRequest`, `pick_documents`), share sheet (`share`) and screen
reader state (`screen_reader_running`) are trait methods; NullHost keeps
in-memory pasteboards, so pasteboard behaviour exists on any OS. The only
AppKit left in pear-uikit is text: `NSFont`/`NSFontManager` in graphics.rs
(UIFont, text drawing), which M4 replaces with `HostText` or pure Rust.
*M4 step 1:* NSFont/NSFontManager are gone; UIFont is CoreText only.

*M3 step 4 (2026-11-21):* the event sink. `pear_host::GuestEvents`
(pointer with `TouchPhase`, `KeyEvent` with HID usage and UIKeyModifierFlags
bits, text, `EditCommand`, `text_input_active`, window resized / became key,
`Lifecycle`) is what a host backend calls; `install_events`/`events()` hold
the sink. pear-uikit installs `UIKitEvents` (forwarding to its
`PearUIKitDeliver*`/lifecycle functions) before the macOS host, and
pear-host-macos no longer declares any `PearUIKit*` symbol: it depends on
pear-host and pear-objc only. `tools/host-event-probe.py` sends synthetic
NSEvents to the host view of an off-screen window and checks that a guest
UIView gets touchesBegan/Moved/Ended at the right points and
pressesBegan/Ended with HID usage 0x04.

*M3 step 5 (2026-11-21):* surfaces. `HostPlatform::surface(LayerRef)`
returns a `SurfaceInfo` (a `NativeSurface` and its pixel size) for a guest
layer; `NativeSurface` is `MetalLayer` / `Wayland` / `Xlib` / `Win32`, with
`egl_window()` giving the `EGLNativeWindowType` per platform. UIKit exports
it to other images as `PearHostSurfaceForLayer`; pear-gles' EAGL
`renderbufferStorage:fromDrawable:` now creates its EGL window surface and
sizes the drawable renderbuffer from it, instead of checking for a CALayer
and reading its bounds. MetalKit is unchanged: `CAMetalLayer` is part of the
guest API MTKView exposes, and moves with QuartzCore/Metal (M5+), not with
the host boundary.

Next steps for the boundary, in order:

1. ~~An event sink trait for input/lifecycle~~ (done, step 4).
2. ~~Surfaces for graphics backends~~ (done, step 5).

Separate traits follow the same pattern for services with their own
lifecycles: `HostText` (font lookup, shaping, rasterising glyph runs),
`HostImage` (decode PNG/JPEG/HEIC to RGBA), `HostAudio`, `HostCrypto`
(the keychain's key generation/signing), `HostFiles` (container roots).
Where a pure-Rust implementation exists (image decoding, crypto) the portable
default needs no host at all.

Graphics: PearANGLE keeps ANGLE's backend model; the boundary is
`NativeSurface` → EGL native window. Metal stays the macOS backend; ANGLE's
upstream Vulkan backend is the Linux/Windows target (re-enabled in the fork).

## 5. Prioritized migration plan

Each milestone is small, testable alone, and keeps working paths working.

1. **M1 — PearHost runs the regression set** (in progress). Make the 64-bit
   test games start and render under PearHost on macOS. Done so far: stub and
   overlay frameworks, full GLES ABI, GLES 1 contexts and drawable
   presentation, launcher preparation; see §7.
2. **M2 — Retire the Catalyst conversion at install.** Install 64-bit apps
   straight into PearHost form (from the IPA's iOS binaries); remove the
   Catalyst preparation once every regression game is at least as good under
   PearHost as under Catalyst.
3. **M3 — `pear-host` trait + `pear-host-macos`.** *Step 1 done
   (2026-11-21):* `pear-host` crate, pear-uikit calls the trait, MacHost
   over the AppKit backend; ABI census and behaviour probe identical, Burger
   Shop 2 and GTA VC render as before, pear-host tests pass on Linux.
   *Step 2 done (2026-11-21):* the AppKit backend and input in
   `pear-host-macos`; UIKit no longer links IOKit or creates NSWindows;
   census and probe identical, a scripted tap takes Burger Shop 2 from its
   menu into gameplay. *Step 3 done:* pasteboard, document picker, share
   sheet, screen reader behind the trait (only text/fonts remain, M4).
   *Step 4 done:* the `GuestEvents` sink; the AppKit backend talks to the
   guest only through it (event probe passes, census/probe identical,
   Burger Shop 2 and GTA VC render, pear-host tests pass on Linux). Next:
   surfaces. *Step 5 done:* `NativeSurface`/`SurfaceInfo` and
   `HostPlatform::surface`; pear-gles asks the host for its EGL window
   (probes identical, census adds only `PearHostSurfaceForLayer`, Burger
   Shop 2 and GTA VC render; GTA LCS now opens no window, also with the
   step 3 build, so not caused by M3 steps 4–5). Next: M4 text.
4. **M4 — Text and images off CoreGraphics/CoreText/ImageIO.** Pure-Rust
   decoding and a glyph rasteriser behind `HostImage`/`HostText`;
   compare rendered UILabel/UIImage output against today's pixels.
   Note: CoreGraphics, CoreText and ImageIO are *guest* APIs (iOS apps call
   CGContext*/CTFont* directly, UIFont is a CTFont, UIImage.CGImage is a
   CGImageRef), so off macOS they become PearRuntime frameworks of their
   own, with `HostText`/`HostImage` below them only for what is genuinely
   the host's (system fonts, codecs). *Step 1 done (2026-11-21):* UIFont
   is made with CoreText alone (UI font types, weight traits, name lookup
   that returns nil like NSFont, family members via font descriptors);
   pear-uikit names no AppKit class. 41 CoreText-vs-NSFont cases agree;
   the behaviour probe gained 16 font and UILabel-pixel facts, identical
   before and after (51 facts). *Step 2 done (2026-11-21):* UIKit.framework
   no longer re-exports AppKit. `tools/host-reexport-census.py` read every
   image of the 22 prepared games: 77 symbols bound "from UIKit" that UIKit
   does not define; 27 come from UIFoundation (text attributes,
   NSParagraphStyle, NSShadow, TextKit classes), 1 from AppKit
   (`NSDirectionalEdgeInsetsZero`, now defined in UIKit), 49 from nowhere
   (all weak imports). UIKit re-exports UIFoundation through
   `host/UIFoundation.tbd` (the SDK's tbd only allows Apple clients), as
   iOS's UIKit does; AppKit stays linked for pear-host-macos only. The stub
   generator now skips UIFoundation names only, which adds empty
   NSCollectionLayout*/NSDiffableDataSourceSnapshot classes (AppKit has
   same-named classes in the process; no prepared game binds them). Probes
   identical (51 facts); all 22 games launch as before (Burger Shop 2 and
   GTA VC render; Gutterball calls 25 missing APIs instead of 31).
   *Step 3 done (2026-11-21):* `pear-image`, PearRuntime's own decoders
   in portable Rust with no dependencies (inflate, PNG with every colour
   type/depth, Adam7, palettes, tRNS, gAMA, Apple's CgBI; JPEG baseline
   and progressive, fancy chroma upsampling, Adobe CMYK). UIImage decodes
   with it and presents the CGImage with ImageIO's layout for the same file
   (gray stays gray, palettes stay indexed, CgBI premultiplied); files with
   ICC profiles, PNG gamma, 16-bit samples or CMYK still use ImageIO on
   macOS (`PEAR_IMAGE_DECODER=imageio` forces it for all). Evidence on the
   2,613 PNG/JPEG files of the 22 games: every PNG (289 CgBI) identical to
   ImageIO in layout, bytes and drawn pixels; the 48 JPEGs that differ
   from ImageIO match Chromium/libjpeg-turbo within 3 levels (ImageIO's
   decoder is the outlier, up to 82 on edges); identical pixel hashes on
   macOS and arm64 Linux; 1.5× ImageIO's decode time (faster IDCT and
   unfiltering are follow-ups). Not covered: ICC colour management.
5. **M5 — Graphics backend selection in PearANGLE.** Build the fork with the
   Vulkan backend for Linux; `pear-gles` takes `NativeSurface`. Run
   the pear-gles GL smoke (Rust example) on Linux in a container.
6. **M6 — Own Mach-O loader for 64-bit.** Load and bind an iOS image with
   pear-macho (segments, chained fixups, initializers) without dyld; start with
   a tiny test binary, then a real app on macOS behind a flag.
   *Step 1 done (2026-11-21):* `pear-macho` parses load commands, segments
   and sections, dylibs, exports tries, initializers and fixups from both
   formats (dyld-info rebase/bind/lazy/weak opcodes: 14 of the 22 game
   executables; chained fixups `PTR_64`/`PTR_64_OFFSET`, import formats
   1–3: 7). Its fixups equal `dyld_info -fixups` for all 228 game Mach-Os
   dyld_info can read (11.96 M fixups; weak-coalescing entries matched by
   address, since dyld_info mislabels them; it crashes on 4 others). New
   crate `pear-dyld` (no dependencies): maps an image, applies rebases and
   binds through a caller's `Resolver` (lazy pointers bound eagerly, missing
   weak imports NULL, missing strong ones refused or trapped), sets
   protections, flushes the instruction cache, runs `__mod_init_func` and
   `__init_offsets` initializers, resolves exports, and implements Darwin
   thread-local variables (own `tlv_get_addr` thunk preserving all registers
   but x0/x16/x17, per-thread blocks from the template). Evidence: iOS
   fixture dylibs built by Apple's clang (`pear-dyld/fixtures`, both fixup
   formats) load and run — initializers, binds, rebases, imported data,
   weak imports, TLV across threads — on macOS and on arm64 Linux (6.18,
   4 KiB pages); all 264 Mach-O images of the 22 games map and fix up on
   both, with identical results. Next: running real app code needs its
   Objective-C metadata registered with a runtime (M7) and its imports
   resolved against PearRuntime's frameworks; re-exports and arm64e are
   not handled yet.
7. **M7 — Objective-C runtime.** Decided (2026-11-21): our own, in Rust,
   implementing Apple's 64-bit ABI (libobjc2 does not implement it, see
   §2.5; porting objc4 would bring a large Darwin-bound C++ codebase); the
   32-bit engine's runtime is the reference for semantics.
   *Step 1 done (2026-11-21):* crate `pear-libobjc` (no dependencies
   besides pear-dyld): selector uniquing (`__objc_selrefs`), class and
   metaclass realization from `class_ro_t` with pointer and relative method
   lists and non-fragile ivar sliding, categories, protocol conformance,
   `objc_msgSend`/`objc_msgSendSuper2` in assembly with `+initialize`,
   `+load` (classes then categories), NSObject's core methods, retain counts
   in a side table, `.cxx_destruct`, zeroing weak references, autorelease
   pools and the ARC entry points clang emits. It is not the host's libobjc:
   pear-dyld binds an image's libobjc imports to it (`pear_libobjc::symbol`).
   Evidence: an iOS fixture built by Apple's clang with ARC
   (`pear-libobjc/fixtures/fixture.m`: classes, super calls, a category, a
   protocol, introspection, strong/weak ARC, autorelease pools, +load) passes
   all 8 checks on macOS and on arm64 Linux; the metadata of all 264 game
   images registers on macOS (577,863 selector refs, 65,067 classes
   realized, 10,732 waiting on UIKit/Foundation classes not loaded). Found
   on the way: objc4 masks class data pointers to 47 bits (Darwin's user
   space); Linux arm64 maps at the top of 48 bits, so pear-libobjc keeps 48
   bits and pear-dyld now places images below 2^47 as Darwin does. Next: a
   per-class message cache in assembly (every send takes the Rust lookup
   today), forwarding and exceptions, associated objects, blocks,
   properties/reflection API, tagged pointers and Swift classes, then
   Foundation (M8) on top.
   *Step 2 in progress:* census-driven libobjc surface (119 of 158 imported
   symbols missing after step 1); method entries, forwarding,
   `objc_msgSendSuper`, `_objc_msgForward` and the `objc_retain_xN`
   variants are in; the remaining list and order are in `docs/HANDOFF.md`.
8. **M8 — Foundation/CoreFoundation for the guest.** Rust implementations of
   the classes apps actually use, census-driven (the census tools already
   record every imported class and selector).
9. **M9 — Linux host backend** (Wayland/X11 windowing, input, audio) and an
   end-to-end run of one 64-bit and one 32-bit game in a Linux arm64 VM.
10. **M10 — x86-64 hosts / Windows**: A64 translation (Dynarmic), Win32 backend.
11. **32-bit track (parallel):** bring PearLegacyEngine into this workspace
    (it is Rust already), share PearANGLE and `pear-host` with it, then retire
    its SDL dependency per platform as `pear-host` backends mature.

## 6. Validation matrix

| Check | How | Today |
|---|---|---|
| Workspace build | `cargo build` (host), `host/build-host.sh`, `build.sh` (Catalyst while it exists) | passes on macOS |
| Linux portability | `container run … rust` builds of each crate | pear-macho, pear-nib, pear-objc, pear-core, pear-host, pear-opengles, pear-gles build on arm64 Linux (the GLES forwarders use ELF `:lo12:` relocations there, Mach-O `@PAGE` on Apple; x86-64 needs its own trampolines); tests of pear-host/pear-macho/pear-nib pass; tests of pear-macho/pear-nib pass; pear-uikit/pear-keychain/pear-metalkit link Apple frameworks |
| ObjC ABI parity | `tools/host-abi-census.py --compare` | identical to the last ObjC build and across M3 |
| Host behaviour | `tools/host-behavior-probe.py --compare` (UIKit accessors, exception boundaries, constants, EAGL GLES 1/2/3) | identical across M3 (35 facts) and M4 step 1 (51 facts: + UIFont constructors/metrics, a UILabel's pixels) |
| Mach-O loading | `pear-macho --example fixups-dump` vs `dyld_info -fixups`; `pear-dyld` tests (fixtures run); `pear-dyld --example load-check` over every game image | 228/228 identical fixups; fixtures run on macOS and Linux; 264/264 images load on both (M6 step 1) |
| Objective-C runtime | `pear-libobjc` test (iOS ARC fixture, 8 checks); `pear-libobjc --example objc-register` over every game image | fixture passes on macOS and Linux; 264/264 images register on macOS (M7 step 1) |
| Image decoding | `tools/host-image-probe.py` (UIImage vs ImageIO: layout, bytes, drawn pixels); `pear-image --example imageio-compare`, `decode-hash` (macOS vs Linux) | 2,565/2,613 identical, rest JPEG-only and within libjpeg-turbo's output (M4 step 3) |
| Host input path | `tools/host-event-probe.py` (synthetic NSEvents → host view → `events()` → guest UIView) | touches and presses delivered (M3 step 4) |
| GL | `pear-gles --example pearangle-smoke`; EAGL GLES1/2/3 context test | pass on macOS |
| Game regression | §7 | see below |
| 32-bit | Miku Flick via PearLegacyEngine | runs (log 2026-11-19) |

## 7. Baseline (2026-11-21)

See `docs/runs/baseline-2026-11-21.md` for the per-game table (PearHost on
macOS, 20-second runs, window-only capture, rendered = window not black).
