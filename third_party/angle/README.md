# PearANGLE

**PearANGLE** is the [PearSilicon](https://github.com/denizomer10/PearSilicon)
fork of [ANGLE](https://chromium.googlesource.com/angle/angle), the OpenGL ES
implementation maintained by Google and the ANGLE contributors. It provides the
OpenGL ES 1.1, 2.0 and 3.x implementation, running on Metal, that iOS games use
when PearSilicon runs them on Apple silicon Macs.

PearANGLE targets **Apple silicon Macs only, with the Metal back end only**
(macOS and Mac Catalyst, arm64). Direct3D,
desktop OpenGL, Vulkan, WebGPU, null and OpenCL back ends, the HLSL, WGSL and
SPIR-V shader translators, Windows/Android/Linux/Fuchsia/Ozone platform code,
Perfetto tracing and the test-suite and trace dependencies are removed; `DEPS`
fetches only what the Metal build needs. `gni/angle.gni` asserts arm64 Apple
targets with Metal only. Everything that remains, and almost all of the code,
is ANGLE's work. The upstream project README is kept
as [README.ANGLE.md](README.ANGLE.md).

## Where it is used

| Consumer | Build | How it is loaded |
| --- | --- | --- |
| PearRuntime PearHost (`host/build-pearangle.sh`) | macOS arm64, Metal only | re-exported by PearHost's `OpenGLES.framework` for iOS apps that run without iOSSupport |
| [PearRuntime](https://github.com/denizomer10/PearRuntime) | Mac Catalyst, arm64 (`arm64-apple-ios15.0-macabi`), Metal only | `libEGL.dylib` and `libGLESv2.dylib` are copied into each prepared iOS app and loaded by PearRuntime's GLES/EAGL bridge |
| [PearLegacyEngine](https://github.com/denizomer10/PearLegacyEngine) | macOS arm64, Metal | `touchHLE_dylibs/angle/`, loaded by the 32-bit iOS engine |

PearRuntime pins this repository as its `third_party/angle` submodule, and
PearLegacyEngine as `vendor/angle`.

## Changes from upstream ANGLE

All changes are on `main`, on top of the upstream base commit
`686adbb2a39854ab687da009e534f6842a85f453`:

| Area | Change |
| --- | --- |
| EGL threads | Release the current EGL context when an Apple thread exits, so a later thread can bind it again (`EGL_BAD_ACCESS` otherwise). |
| Metal surfaces | Position the Metal child layer in the host layer's coordinates; place drawable content below the application's own sublayers. |
| Mac Catalyst | Build relocatable Catalyst dylibs (`@rpath` install names, iOS system paths). |
| Texture formats | Expose PVRTC and Apple GPU formats on Mac Catalyst, where the Apple GPU family supports them. |
| Shaders | Compile Metal shader libraries with at least MSL 3.0. |
| Contexts | Opt-in serialized multi-thread context bindings for games that share one context across threads ([doc/PearRuntimeThreadSharing.md](doc/PearRuntimeThreadSharing.md)). |

`git log 686adbb2a398..main` shows each patch with its rationale.

## Building

The supported way to build the fork is through PearSilicon, which pins the
Chromium toolchain from `DEPS`, applies the GN arguments below and tests the
result:

```sh
git clone --recurse-submodules https://github.com/denizomer10/PearSilicon.git
cd PearSilicon
bash scripts/build-angle-metal.sh --sync        # Mac Catalyst: runtime/third_party/angle/out/PearCatalyst
PEAR_ANGLE_TARGET=macos bash scripts/build-angle-metal.sh   # macOS: out/PearMetal
```

The Mac Catalyst build uses:

```gn
target_os = "ios"
target_environment = "catalyst"
target_cpu = "arm64"
is_component_build = false
angle_enable_metal = true
angle_enable_gl = false
angle_enable_vulkan = false
angle_enable_wgpu = false
```

Building it on its own follows upstream's instructions in
[doc/DevSetup.md](doc/DevSetup.md) (`gclient sync`, `gn gen`, `ninja`).

## License

ANGLE is licensed under the BSD 3-Clause license in [LICENSE](LICENSE),
copyright The ANGLE Project Authors. The changes in this fork are distributed
under the same license. Third-party code in `third_party/` and
`src/third_party/` keeps its own license; each directory's `README.chromium`
names it.

Binaries built from this fork contain some of that third-party code (abseil,
zlib, xxHash, astc-encoder and others). PearRuntime's `collect-angle-licenses.py`
reads the link inputs of a build and gathers their license texts into
`THIRD_PARTY_LICENSES.txt`, which ships next to the dylibs.

PearANGLE is not affiliated with or endorsed by Google or the ANGLE project.
