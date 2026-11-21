# PearRuntime — handoff for the next agent

Last updated 2026-11-21. Read this first, then `docs/independence-plan.md`
(the full plan: inventory, research, host boundary, milestones M1–M10,
validation matrix, baseline). This file says what is done, what is half-done,
and exactly where to pick up.

## Goal and ground rules

- **Goal:** PearRuntime becomes an independent iOS compatibility runtime:
  32- and 64-bit iOS apps and games on macOS, later Linux and Windows,
  without Mac Catalyst, without `/System/iOSSupport`, eventually without
  dyld/libobjc/Apple frameworks. 32-bit is handled by PearLegacyEngine
  (touchHLE fork, separate repo); this work is the 64-bit path.
- **Rust first.** No game-specific hacks. Never claim independence without
  evidence (a test, a probe, a comparison).
- **No external crates.** The user chose to write our own (image decoders,
  loader, ObjC runtime). The workspace has zero crates.io dependencies; keep
  it that way unless the user says otherwise.
- **Git:** branch `pearhost-rust-completion` in both repos. Never commit to
  `main`, **never push** without asking. Commit the runtime submodule
  first (`runtime/`), then the parent (`git add runtime`). Commit messages
  end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **The user's screen:** never capture the whole screen; game windows only
  (`tools/host-winid.swift`).
- The user writes Turkish; replies so far were in English.

## Clean-room rules (binding)

`docs/independent-runtime.md` §2–3 (Turkish) governs all code:
- **Allowed sources:** public SDK headers and `.tbd` stubs (interface only,
  never copied into the repo), Apple's public documentation, Apple's open
  source (`apple-oss-distributions/*`: objc4, dyld, xnu, CF…; APSL 2.0 — if
  code is ever copied it lives under `third_party/<project>` with its
  license), the analysed apps' own binaries (imports, selectors, observed
  calls), black-box observation, and public specs.
- **Forbidden:** disassembling/decompiling Apple binaries (iOSSupport, the
  dyld shared cache), class-dump output, leaked source, notes derived from
  them.
- **Every file's header names its source** ("Clean-room: from …"). The
  crates added in this work carry such lines (ABI layouts from objc4/dyld
  open source and public headers, formats from their public specs).

Scope note: `independent-runtime.md` (2026-11-14) declared Linux/Vulkan
out of scope; the user later asked for full independence (macOS, Linux,
Windows), which `independence-plan.md` follows. The previous handoff is
kept as `docs/HANDOFF-2026-11-14.md` (PearHost history: GTA VC DE reaching
gameplay, Minecraft world loading, input/keyboard notes, missing-call
report format); its ObjC `host/` layout no longer exists (ported to Rust).

## Repository map (runtime/)

| Path | What |
|---|---|
| `rust/crates/pear-host` | Host OS boundary: `HostPlatform` trait (windows, screen, display link, pasteboard, pickers, share, alerts, run loop, `surface()` → `NativeSurface`), `GuestEvents` sink (input, lifecycle), `NullHost`. No Apple deps; tested on Linux. |
| `rust/crates/pear-host-macos` | The only AppKit user: `MacHost`, `PearBackend*` C exports, `PearHostView`, delegates. |
| `rust/crates/pear-uikit` | PearRuntime's UIKit (built into `build/host/UIKit.framework`). `events.rs` (sink), `image_decode.rs` (UIImage via pear-image), `graphics.rs` (UIFont on CoreText, UILabel, UIImage). |
| `rust/crates/pear-gles`, `pear-opengles` | OpenGLES.framework on PearANGLE; forwarders are aarch64 asm (Mach-O and ELF relocations). |
| `rust/crates/pear-image` | Own decoders: inflate, PNG (+CgBI, Adam7, gAMA), JPEG (baseline/progressive, fancy upsampling). |
| `rust/crates/pear-macho` | Mach-O parser: segments/sections, dylibs, fixups (dyld-info opcodes + chained), exports trie, initializers. |
| `rust/crates/pear-dyld` | Own loader: maps an image below 2^47, applies fixups via a `Resolver`, TLV (`tlv_get_addr` thunk), initializers, exports. Fixtures in `fixtures/`. |
| `rust/crates/pear-libobjc` | Own Objective-C runtime (Apple 64-bit ABI). Fixture in `fixtures/fixture.m`. |
| `host/build-host.sh` | Builds `build/host/*.framework` (UIKit, OpenGLES, MetalKit, stubs). `host/UIFoundation.tbd`: UIKit re-exports UIFoundation (not AppKit). |
| `tools/` | Probes and comparisons, listed below. |
| `docs/independence-plan.md` | The plan; every finished step is recorded under its milestone with its evidence. |

PearSilicon (parent) launches 64-bit apps through PearHost
(`PearHostPreparer.swift`, `AppLauncher.swift` — note the transitional
`if true` there, M2 work).

## Status by milestone

| Milestone | State |
|---|---|
| M1 inventory/plan/baseline | Done (`docs/independence-plan.md`, `docs/runs/baseline-2026-11-21.md`). |
| M2 retire Catalyst | **Done:** `AppLauncher` drops `if true` & Catalyst launch paths; `PackageInstaller` installs 64-bit apps straight into PearHost form; `AppDetailView` drops the Catalyst/PearHost toggle. |
| M3 host trait | **Done** (steps 1–5): trait, AppKit moved to pear-host-macos, pasteboard/picker/share/a11y, `GuestEvents` sink, `NativeSurface`. |
| M4 text/images | Steps 1–3 done: UIFont on CoreText only (no AppKit in pear-uikit); UIKit re-exports UIFoundation instead of AppKit; own image decoders behind UIImage. **Open:** ICC colour management (ImageIO fallback for ICC/gAMA/16-bit/CMYK on macOS), decoder speed (1.5× ImageIO), own CoreGraphics/CoreText for non-macOS (big). |
| M5 GLES on Vulkan | Not started (PearANGLE Linux/Vulkan build). Only prerequisite done: forwarders build on arm64 Linux. |
| M6 own Mach-O loader | Step 1 done. **Open:** re-exported symbols, arm64e, thread-local *initializer* sections (refused), resolving a real app against PearRuntime frameworks. |
| M7 own ObjC runtime | **Done (steps 1–3):** 129/158 census symbols + 23 register variants; per-class lock-free ARM64 assembly cache, tagged pointers, Swift class flags (`bits & 3`), DWARF LSDA `___objc_personality_v0`, blocks, properties, `@synchronized`, associated objects, reflection, runtime classes; `fixture.m`, `fixture2.m`, and LSDA unit test pass on macOS & Linux. |
| M8 Foundation/CF | Not started. |
| M9 Linux host backend, M10 | Not started. |

## In progress: M7 step 2 (uncommitted work becomes committed with this file)

Census-driven: `docs/census/*.json` lists every libobjc symbol the 23 apps
import. Step 1 provided 39 of 158. The script used (rerun it after each
change; it reads `pub fn symbol` in `pear-libobjc/src/lib.rs`):

```bash
cd runtime && python3 - <<'EOF'
import json, glob, re, collections
have=set(re.findall(r'"(_[A-Za-z_$0-9]+)"', open('rust/crates/pear-libobjc/src/lib.rs').read().split('pub fn symbol')[1]))
apps=collections.defaultdict(set)
for f in glob.glob('docs/census/*.json'):
    d=json.load(open(f)); lib=d.get('libraries',{}).get('libobjc')
    if not lib: continue
    app=f.split('/')[-1].rsplit('-',1)[0]
    for s in lib.get('symbols',[]): apps[s].add(app)
    for c in lib.get('classes',[]): apps['_OBJC_CLASS_$_'+c].add(app)
for n,k in sorted(((len(v),k) for k,v in apps.items() if k not in have), reverse=True): print(n,k)
EOF
```

(Register variants `_objc_retain_xN`/`_objc_release_xN` are matched in
`message::register_variant`, so the script still lists them; they are done.)

**Done in step 2 so far (builds; all tests pass on macOS and Linux):**
- Methods are stable `MethodEntry { name, types, imp: AtomicUsize }`
  (what a `Method` handle will point to); classes record `method_list`
  (definition order), `ivars` (`IvarEntry`), `properties`, `image` name,
  `building` flag. `runtime::lookup_method`, `own_method_entry`,
  `register_dynamic`, `publish`, `all_classes`, `image_names` exist.
- Pointer-format method lists now unique their selector names (bug fix:
  older binaries' lookups would have missed).
- `message.rs`: lookups return `Dispatch { imp, receiver }`; forwarding via
  `+resolveInstanceMethod:`/`+resolveClassMethod:` then
  `forwardingTargetForSelector:`; `objc_msgSendSuper`, `_objc_msgForward`;
  `objc_retain_xN`/`objc_release_xN` (x1–x15, x19–x28).

**To do in step 2, in this order (each with a fixture check):**
1. **Custom retain/release:** `objc_retain`/`objc_release`/`objc_autorelease`
   bypass messages today; classes that override `-retain`/`-release`
   (global blocks, singletons) must get the message. Compute "has custom RR"
   per class (lookup of retain/release ≠ NSObject's) and send when true.
2. **Blocks (libclosure, in a new `blocks.rs`):** `_Block_copy`,
   `_Block_release`, `_Block_object_assign`, `_Block_object_dispose`,
   byref copy/release, `_NSConcreteStackBlock/MallocBlock/GlobalBlock` as
   real class objects (`__NSStackBlock__` etc. under `NSBlock` → NSObject,
   with retain/release/copy methods), `objc_retainBlock`. These are
   *libSystem* imports: resolvers must map them to pear-libobjc too.
   `imp_implementationWithBlock` (+`imp_getBlock`/`imp_removeBlock`):
   trampoline per block on a fresh RX page:
   `mov x1, x0 (0xAA0003E1); ldr x0, +12 (0x58000060); ldr x16, [x0, #16] (0xF9400810); br x16 (0xD61F0200); .quad block`.
3. **Properties / sync / associated objects (`props.rs`):**
   `objc_getProperty`, `objc_setProperty` and its `_atomic`, `_nonatomic`,
   `_atomic_copy`, `_nonatomic_copy` variants (striped locks), `objc_copyStruct`,
   `objc_copyCppObjectAtomic`; `objc_sync_enter/exit` (recursive lock per
   object); `objc_enumerationMutation` (abort with message);
   `objc_set/get/removeAssociatedObjects` (policies 0, 1, 3, 01401, 01403;
   released on dealloc — hook into `object::dispose`). NSObject also needs
   `copy`/`mutableCopy` (→ `copyWithZone:`), `methodForSelector:`,
   `doesNotRecognizeSelector:`, `forwardingTargetForSelector:` (nil),
   `+resolveInstanceMethod:` (NO).
4. **Reflection & runtime classes (`reflection.rs`):** `class_getName`,
   `class_getInstanceMethod/ClassMethod`, `class_copyMethodList`,
   `method_getName/getImplementation/setImplementation/getTypeEncoding/
   getNumberOfArguments/copyArgumentType/copyReturnType/getArgumentType/
   getReturnType/getDescription`, `method_exchangeImplementations`,
   `class_getMethodImplementation`, `class_replaceMethod`,
   `class_getInstanceVariable`, `ivar_getName/Offset/TypeEncoding`,
   `class_copyIvarList`, `class_getInstanceSize`, `class_isMetaClass`,
   `class_conformsToProtocol`, `class_addProtocol`, `class_copyProtocolList`,
   `class_copyPropertyList`, `class_getProperty`, `property_getName/
   getAttributes/copyAttributeList/copyAttributeValue`, `class_getImageName`,
   `objc_copyClassNamesForImage`, `objc_copyImageNames`,
   `objc_copyClassList`, `objc_getClassList`, `objc_getMetaClass`,
   `objc_getRequiredClass`, `objc_getProtocol` (needs a protocol registry
   filled from `__objc_protolist` in `register_image`), `protocol_getName`,
   `protocol_getMethodDescription` (returns a 16-byte struct in x0/x1),
   `protocol_copyMethodDescriptionList`, `protocol_conformsToProtocol`,
   `sel_isEqual`, `object_getClassName`, `object_isClass`,
   `object_getIvar/setIvar`, `object_getIndexedIvars`, `object_setClass`,
   `class_createInstance`, `objc_constructInstance`,
   `objc_destructInstance`, `objc_allocWithZone`, `_objc_rootAutorelease`,
   `objc_allocateClassPair`/`class_addIvar`/`objc_registerClassPair`
   (use `runtime::register_dynamic` + `publish`; such classes have no
   `class_ro_t`, `bits = 0` — code that calls `runtime::ro()` must check for
   null, as `call_load_methods` already does). Type encodings need a small
   parser (qualifiers `rnNoORV`, `{}`, `()`, `[]`, `^`, `b<n>`, `@"Class"`,
   `@?`, offset digits).
5. **Exceptions:** for now `objc_exception_throw`/`rethrow`,
   `objc_begin_catch/end_catch`, `objc_terminate` log and abort;
   `_OBJC_EHTYPE_id`, `_objc_ehtype_vtable`, `__objc_empty_vtable` as
   statics; `objc_setExceptionPreprocessor` stores and returns. Real
   exceptions are a separate step: a real `__objc_personality_v0` (LSDA
   parsing) plus an unwinder that knows the loaded images' unwind info
   (`__unwind_info`/`__eh_frame` of pear-dyld images are invisible to
   libgcc/libunwind on Linux).
6. Extend the fixture: a second ObjC fixture (`fixtures/fixture2.m`,
   rebuild with `build-fixtures.sh`) covering properties (atomic/nonatomic,
   copy), `@synchronized`, associated objects, blocks (stack→heap, `__block`
   captures, block properties), `imp_implementationWithBlock` +
   `class_addMethod`, swizzling, reflection counts/offsets, a class made at
   run time. Run on macOS **and** Linux.
7. Then step 3: per-class message cache in assembly (every send currently
   takes the Rust lookup with a global `RwLock<HashMap>` — correct but slow),
   real exceptions, tagged pointers, Swift classes (`bits & 3`).

## How to build and verify

```bash
# Rust workspace (macOS)
cd runtime/rust && cargo build && cargo test -p pear-macho -p pear-dyld -p pear-libobjc -p pear-image -p pear-host

# Same on arm64 Linux (Apple `container`; if it says "XPC connection error":
#   container system start --enable-kernel-install)
container run --rm -v "$PWD":/src -w /src rust:1-bookworm bash -c \
  'CARGO_TARGET_DIR=/tmp/t cargo test -q -p pear-macho -p pear-dyld -p pear-libobjc -p pear-image -p pear-host'

# PearHost frameworks + probes (macOS)
cd runtime && bash host/build-host.sh
python3 tools/host-behavior-probe.py build/host --compare <earlier output>   # 51 facts (fonts, label pixels, EAGL, pasteboard…)
python3 tools/host-event-probe.py build/host                                  # AppKit input → events() → UIView
python3 tools/host-abi-census.py build/host --compare <earlier json>           # ObjC/C ABI parity
find <app> -name '*.png' -o -name '*.jpg' | python3 tools/host-image-probe.py build/host   # UIImage vs ImageIO

# Games (needs the external disk)
tools/host-baseline.sh 25 burgershop     # one game; omit the filter for all 22
```

Comparison tools for the loader/runtime/decoders (need the external disk):
`tools/macho-fixups-compare.py` (pear-macho vs `dyld_info -fixups`),
`pear-dyld --example load-check`, `pear-libobjc --example objc-register`,
`pear-image --example imageio-compare | bench | decode-hash`.
Write long outputs to files: zsh's multios makes `2>&1 >/dev/null | …`
pipe stdout too.

## Test data

External disk `/Volumes/Untitled/PearSilicon`: `ios-apps/` (22 decrypted
apps), `Games/` (their data containers), `Work/host-apps/` (prepared copies
and run logs). It **disconnects at times** — check `ls /Volumes` first; the
last M7 check (objc-register on Linux) was skipped because of that.

Known baseline (2026-11-21, unchanged through all steps): Burger Shop 2 and
GTA VC render; GTA LCS stays alive with no window (since before M3 step 4;
also with older builds — probably game state); Gutterball alive, window
detection varies; 14 others reach launch and crash or miss symbols
(YouTube: Swift UIKit symbol; Angry Birds Transformers:
`UISceneWillDeactivateNotification`); Miku Flick is 32-bit (legacy engine).

## Lessons that cost time

- Linux arm64 user space is 48-bit and maps high; Darwin code assumes
  < 2^47 (objc4 masks). pear-dyld places images low; keep it.
- `dyld_info` mislabels weak-coalescing binds and crashes on 4 big game
  binaries; compare weak entries by address only.
- clang folds constant `__attribute__((constructor))` bodies away: fixture
  initializers must call an import.
- macOS lets unhardened processes turn anonymous RW pages RX (no MAP_JIT
  needed); hardened apps would need the dyld approach (map signed file).
- ImageIO's JPEG decoder deviates from libjpeg-turbo by up to ~80 levels on
  edges; pear-image matches libjpeg-turbo (checked against Chromium).
- The in-app browser pane can serve as an independent reference decoder
  (canvas `getImageData`).
