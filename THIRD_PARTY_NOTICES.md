# PearRuntime — origins and licenses

PearRuntime is our project. Its own code (everything outside `third_party/`,
including the Rust crates under `rust/crates/`, the host frameworks, the
tools and PearDebug in `debug/`) is ours and licensed under the MIT License
([LICENSE](LICENSE)).

`third_party/` holds work by others that PearRuntime builds on. This file
says, for each piece, where it comes from, which license it is under and
where that license text is in this repository. When a binary built from it
is shipped, its license texts and notices travel with it (PearSilicon's
`scripts/stage-legacy-licenses.sh` and `runtime/collect-angle-licenses.py`;
`scripts/verify-licenses.sh` refuses a build in which one is missing).

## Projects we forked and continue here

| Component | Path | Based on | Upstream | License | License text |
|---|---|---|---|---|---|
| **PearLegacyEngine** — 32-bit iOS engine | `third_party/legacy-engine/` | touchHLE, by the touchHLE project contributors | <https://github.com/touchHLE/touchHLE> | Source files: **MPL-2.0** (files keep their headers). The built `touchHLE` executable: **GPL-3.0-or-later**, because it statically links OpenAL Soft (LGPL) | [licenses/legacy-engine-MPL-2.0.txt](licenses/legacy-engine-MPL-2.0.txt), `third_party/legacy-engine/LICENSE`, `third_party/legacy-engine/dev-scripts/gpl-3.0.txt` |
| **PearLegacyJIT** — ARMv7 → arm64 recompiler | `third_party/legacy-jit/` | Dynarmic, by merryhime and the Dynarmic contributors, through the touchHLE fork | <https://github.com/merryhime/dynarmic>, <https://github.com/touchHLE/dynarmic> (the engine pinned `e0f6bd9d0b48ed6e536672c0c9abbe87961be861`) | **0BSD** | [licenses/legacy-jit-0BSD.txt](licenses/legacy-jit-0BSD.txt), `third_party/legacy-jit/LICENSE.txt` |
| **PearANGLE** — OpenGL ES on Metal | `third_party/angle/` | ANGLE, by The ANGLE Project Authors (Google and others) | <https://chromium.googlesource.com/angle/angle> (base `686adbb2a39854ab687da009e534f6842a85f453`) | **BSD-3-Clause** | [licenses/angle-BSD-3-Clause.txt](licenses/angle-BSD-3-Clause.txt), `third_party/angle/LICENSE` |

PearLegacyEngine also credits [HyperHLE-Reborn](https://github.com/HyperHLE/HyperHLE-Reborn)
(see its README). If code from it is identified in the engine, its license
and copyright lines must be kept and listed here.

The development history of PearLegacyEngine and PearLegacyJIT before they
joined PearRuntime — every commit with its original ID and author — is kept
in this repository under the tag `archive/2026-11-22`; PearSilicon releases
up to v0.9.2 name commits from it as the engine's Corresponding Source.

## Libraries inside PearLegacyEngine

Copied into `third_party/legacy-engine/vendor/` at the upstream commits listed
in `vendor/VENDORED.txt`; each keeps its license file.

| Library | Upstream | License | License text |
|---|---|---|---|
| SDL 2 | <https://github.com/libsdl-org/SDL> | Zlib | `vendor/SDL/LICENSE.txt` |
| OpenAL Soft | <https://github.com/kcat/openal-soft> | LGPL-2.0-or-later | `vendor/openal-soft/COPYING` |
| stb (stb_image) | <https://github.com/nothings/stb> | MIT or public domain | `vendor/stb/LICENSE` |
| PVRTDecompress | Imagination Technologies | MIT | `vendor/PVRTDecompress/LICENSE.md` |
| About 90 Rust crates | crates.io (see `Cargo.lock`) | MIT, Apache-2.0, MPL-2.0, Zlib, BSD-3-Clause, Unicode | printed in full by `touchHLE --copyright` |

Loaded into the emulated iOS apps, not linked into the engine
(`third_party/legacy-engine/touchHLE_dylibs/`, origins and sources in its
`README.md`): libstdc++ and libgcc (GPL-2.0-or-later), zlib (Zlib), SQLite
(public domain), libxml2 (MIT) — texts in `COPYING.*` there.

Fonts (`third_party/legacy-engine/touchHLE_fonts/`): Liberation fonts and
Noto Sans CJK, SIL Open Font License 1.1 — `LICENSE.liberation`,
`LICENSE.noto` there.

## Libraries inside PearLegacyJIT

`third_party/legacy-jit/externals/`, each with its own license file:
fmt (MIT), mcl (MIT), oaknut (MIT), robin-map (MIT), biscuit (MIT),
xbyak (BSD-3-Clause), zycore and zydis (MIT), Catch2 (BSL-1.0, tests only);
Boost headers (BSL-1.0) come from the system and are not bundled.

## Libraries fetched when PearANGLE is built

`host/build-pearangle.sh --sync` fetches ANGLE's dependencies (abseil-cpp,
astc-encoder, zlib, xxHash, Chromium's build tools and others, pinned by
`third_party/angle/DEPS`) from Chromium's servers into git-ignored folders.
They are not stored in this repository. The licenses of the ones linked into
a build are collected from the build's own link inputs into
`THIRD_PARTY_LICENSES.txt` next to the shipped `libEGL`/`libGLESv2`.

## Used as reference only

Apple's open source (`apple-oss-distributions`: objc4, dyld, xnu, CF, Libc;
APSL 2.0) and the public Mach-O, Objective-C and Khronos specifications and
headers were read to implement compatible behaviour (pear-macho, pear-dyld,
pear-libobjc and others). No code from them is copied into PearRuntime; each
such file names its sources in its header ("Clean-room: …"). See
`docs/independent-runtime.md` for the rules.

The Rust workspace (`rust/`) has no crates.io dependencies.
