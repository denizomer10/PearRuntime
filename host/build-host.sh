#!/bin/bash
# Builds the PearHost layer: our own UIKit (and empty stand-ins for linked
# iOS-only frameworks an app does not actually call) for the *macOS* platform.
# Nothing here links /System/iOSSupport.
#
#   host/build-host.sh                 # output in build/host
#   PEAR_BUILD_DIR=/tmp/out host/build-host.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${PEAR_BUILD_DIR:-$ROOT/build}/host"
SDK="$(xcrun --sdk macosx --show-sdk-path)"
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION" 2>/dev/null || echo dev)"
TARGET="arm64-apple-macos26.0"
# Everything is compiled by cargo; clang only drives the linker.
LD=(xcrun --sdk macosx clang -target "$TARGET" -isysroot "$SDK")

mkdir -p "$OUT_DIR/UIKit.framework"

# Regenerate stubs from every census report (not the *-selectors.json side
# files) into pear-uikit, before the crates build.
CENSUS=()
for f in "$ROOT"/docs/census/*.json; do
  [[ "$f" == *-selectors.json ]] || CENSUS+=("$f")
done
python3 -I "$ROOT/tools/gen-uikit-stubs.py" "${CENSUS[@]}" > "$OUT_DIR/stubs.log"
sed 's/^/   /' "$OUT_DIR/stubs.log"

# Build the Rust workspace crates (pear-core, pear-gles, pear-metalkit, pear-uikit, ...).
# Each framework is one Rust static library: its Objective-C ABI (class
# symbols and metadata, constants, stubs), its behavior and its image
# initializer. It is force-loaded (nothing else references the initializer).
PEAR_RUNTIME_VERSION="$VERSION" MACOSX_DEPLOYMENT_TARGET=26.0 cargo build --manifest-path "$ROOT/rust/Cargo.toml" --release --target aarch64-apple-darwin --target-dir "$OUT_DIR/rust-target"
RUST_LIBS="$OUT_DIR/rust-target/aarch64-apple-darwin/release"
RUST_GLES_LIB="$RUST_LIBS/libpear_gles.a"
RUST_GLKIT_LIB="$RUST_LIBS/libpear_glkit.a"
RUST_MTK_LIB="$RUST_LIBS/libpear_metalkit.a"
RUST_UIKIT_LIB="$RUST_LIBS/libpear_uikit.a"

rm -rf "$OUT_DIR/obj" # objects of earlier builds (Objective-C sources)

# AppKit is linked for the macOS host backend (pear-host-macos) but not
# re-exported: apps see UIKit's own symbols plus UIFoundation's text system,
# as on iOS (host/UIFoundation.tbd, tools/host-reexport-census.py).
"${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_UIKIT_LIB" -o "$OUT_DIR/UIKit.framework/UIKit" \
  "$ROOT/host/guest-dlopen.c" "$ROOT/host/cloudkit-unavailable.m" "$ROOT/host/client-credential.m" \
  -fobjc-arc -Wno-deprecated-declarations \
  -install_name @rpath/UIKit.framework/UIKit -compatibility_version 1.0.0 -current_version 6209.0.0 \
  -framework Foundation -framework AppKit -Wl,-reexport_library,"$ROOT/host/UIFoundation.tbd" -framework QuartzCore -framework Metal -framework CoreGraphics -framework CoreText -framework ImageIO -framework IOKit -framework Security -lc++
echo ">> $OUT_DIR/UIKit.framework/UIKit"

# Stub and overlay frameworks generated from the census: iOS-only frameworks
# apps link (MessageUI, iAd, ...) and macOS frameworks missing iOS-only API
# (AVKit's AVPlayerViewController, ...); an overlay re-exports the macOS
# framework of the same name. tools/gen-host-frameworks.py lists them.
python3 -I "$ROOT/tools/gen-host-frameworks.py" "${CENSUS[@]}" > "$OUT_DIR/host-frameworks.log"
sed 's/^/   /' "$OUT_DIR/host-frameworks.log"
while read -r fw kind; do
  [[ -z "$fw" || "$fw" == \#* ]] && continue
  PEAR_HOST_FRAMEWORK="$fw" MACOSX_DEPLOYMENT_TARGET=26.0 cargo build --quiet --manifest-path "$ROOT/rust/Cargo.toml" \
    --release --target aarch64-apple-darwin --target-dir "$OUT_DIR/rust-target" -p pear-host-frameworks
  REEXPORT=()
  [[ "$kind" == overlay ]] && REEXPORT=(-Wl,-reexport_framework,"$fw")
  rm -rf "$OUT_DIR/$fw.framework"
  mkdir -p "$OUT_DIR/$fw.framework"
  "${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_LIBS/libpear_host_framework.a" -Wl,-dead_strip \
    "$OUT_DIR/UIKit.framework/UIKit" -framework Foundation ${REEXPORT[@]+"${REEXPORT[@]}"} \
    -o "$OUT_DIR/$fw.framework/$fw" -install_name "@rpath/$fw.framework/$fw" \
    -compatibility_version 1.0.0 -current_version 1.0.0
  echo ">> $OUT_DIR/$fw.framework/$fw ($kind)"
done < "$ROOT/rust/crates/pear-host-frameworks/frameworks.txt"

# MobileCoreServices is CoreServices on macOS. A framework of that name that
# re-exports it keeps an app that links both from naming CoreServices twice.
rm -rf "$OUT_DIR/MobileCoreServices.framework"
mkdir -p "$OUT_DIR/MobileCoreServices.framework"
"${LD[@]}" -dynamiclib -Wl,-u,_pear_empty_framework "$RUST_LIBS/libpear_empty_framework.a" -Wl,-dead_strip \
  -Wl,-exported_symbol,_pear_empty_framework -Wl,-reexport_framework,CoreServices \
  -o "$OUT_DIR/MobileCoreServices.framework/MobileCoreServices" \
  -install_name @rpath/MobileCoreServices.framework/MobileCoreServices -compatibility_version 1.0.0 -current_version 1.0.0
echo ">> $OUT_DIR/MobileCoreServices.framework/MobileCoreServices (overlay of CoreServices)"

# C++ runtime compatibility (pear-cxxcompat, as for Mac Catalyst): re-exports
# the system libc++ and supplies the empty tag object some binaries expect.
MACOSX_DEPLOYMENT_TARGET=26.0 cargo build --quiet --manifest-path "$ROOT/rust/Cargo.toml" --release \
  --target aarch64-apple-darwin --target-dir "$OUT_DIR/rust-target" -p pear-cxxcompat
"${LD[@]}" -dynamiclib -Wl,-dead_strip -Wl,-reexport_library,"$SDK/usr/lib/libc++.1.tbd" \
  -Wl,-force_load,"$RUST_LIBS/libpear_cxxcompat.a" -o "$OUT_DIR/libcxxcompat.dylib" \
  -install_name @rpath/libcxxcompat.dylib -compatibility_version 1.0.0 -current_version 1.0.0
echo ">> $OUT_DIR/libcxxcompat.dylib"

# The UIKit Swift overlay (Swift, compat/swift + pear-swift-support).
# The preparers retarget it to macOS like the app's own images.
IOS_FRAMEWORKS="$SDK/System/iOSSupport/System/Library/Frameworks"
CATALYST_OVERLAY="${PEAR_BUILD_DIR:-$ROOT/build}/libswiftUIKit.dylib"
if [[ -f "$CATALYST_OVERLAY" ]]; then
  cp "$CATALYST_OVERLAY" "$OUT_DIR/libswiftUIKit.dylib"
  echo ">> $OUT_DIR/libswiftUIKit.dylib (staged)"
elif [[ -d "$IOS_FRAMEWORKS" ]]; then
  SWIFT_TARGET_DIR="${PEAR_BUILD_DIR:-$ROOT/build}/rust-catalyst"
  CLANG_TARGET="arm64-apple-ios15.0-macabi"
  SWIFTC="$(xcrun --find swiftc)"
  IPHONEOS_DEPLOYMENT_TARGET=15.0 RUSTC_BOOTSTRAP=1 \
    cargo build --quiet --manifest-path "$ROOT/rust/Cargo.toml" --release -p pear-swift-support \
    --target aarch64-apple-ios-macabi -Zbuild-std=std,panic_abort --target-dir "$SWIFT_TARGET_DIR"
  SWIFT_SUPPORT_LIB="$SWIFT_TARGET_DIR/aarch64-apple-ios-macabi/release/libpear_swift_support.a"
  "$SWIFTC" -target "$CLANG_TARGET" -sdk "$SDK" -F "$IOS_FRAMEWORKS" \
    -parse-as-library -emit-object -module-name PearSwiftUIKit \
    -o "$OUT_DIR/PearSwiftUIKit.o" "$ROOT/compat/swift/PearSwiftUIKit.swift"
  "$SWIFTC" -target "$CLANG_TARGET" -sdk "$SDK" -F "$IOS_FRAMEWORKS" \
    -parse-as-library -emit-object -module-name UIKit -module-alias AppleUIKit=UIKit \
    -o "$OUT_DIR/PearUIKitNumeric.o" "$ROOT/compat/swift/PearUIKitNumeric.swift"
  "$SWIFTC" -target "$CLANG_TARGET" -sdk "$SDK" -F "$IOS_FRAMEWORKS" \
    -parse-as-library -emit-library -Xlinker -install_name -Xlinker @rpath/libswiftUIKit.dylib \
    -Xlinker -undefined -Xlinker dynamic_lookup \
    -o "$OUT_DIR/libswiftUIKit.dylib" "$OUT_DIR/PearSwiftUIKit.o" "$OUT_DIR/PearUIKitNumeric.o" \
    -Xlinker -u -Xlinker '__swift_FORCE_LOAD_$_swiftUIKit' "$SWIFT_SUPPORT_LIB"
  rm -f "$OUT_DIR/PearSwiftUIKit.o" "$OUT_DIR/PearUIKitNumeric.o"
  echo ">> $OUT_DIR/libswiftUIKit.dylib (built from compat/swift)"
elif [[ ! -f "$OUT_DIR/libswiftUIKit.dylib" ]]; then
  echo "!! no libswiftUIKit.dylib and no iOSSupport SDK: apps linking the UIKit Swift overlay will not start" >&2
fi
if [[ -f "$OUT_DIR/libswiftUIKit.dylib" ]] && vtool -show-build "$OUT_DIR/libswiftUIKit.dylib" 2>/dev/null | grep -q "platform MACCATALYST"; then
  vtool -set-build-version macos 26.0 26.0 -replace -output "$OUT_DIR/libswiftUIKit.dylib.tmp" "$OUT_DIR/libswiftUIKit.dylib"
  mv "$OUT_DIR/libswiftUIKit.dylib.tmp" "$OUT_DIR/libswiftUIKit.dylib"
  echo "   retargeted $OUT_DIR/libswiftUIKit.dylib to macOS 26.0"
fi

# Stands in for the Catalyst runtime (libPearRuntime.dylib) that a bundle
# prepared for Mac Catalyst links; PearHost needs nothing from it.
"${LD[@]}" -dynamiclib -Wl,-u,_pear_empty_framework "$RUST_LIBS/libpear_empty_framework.a" -Wl,-dead_strip \
  -Wl,-exported_symbol,_pear_empty_framework -o "$OUT_DIR/libPearHostPlaceholder.dylib" \
  -install_name @rpath/libPearHostPlaceholder.dylib -compatibility_version 1.0.0 -current_version 1.0.0
echo ">> $OUT_DIR/libPearHostPlaceholder.dylib"

# OpenGLES.framework: macOS has none for macOS-platform processes. pear-gles
# is EAGL and every GLES prototype (forwarders resolved on first call): ANGLE's
# entry point (PearANGLE, our Metal-only fork, loaded from ../PearANGLE), the
# same function under its core/EXT/OES/ANGLE name, or an adapter for iOS-only
# extensions; an entry point with none of these logs and aborts when called.
ANGLE_DIR="${PEAR_HOST_ANGLE_DIR:-$OUT_DIR/PearANGLE-src}"
mkdir -p "$OUT_DIR/OpenGLES.framework"
GLES_LINK=()
if [[ -f "$ANGLE_DIR/libGLESv2.dylib" ]]; then
  rm -rf "$OUT_DIR/PearANGLE"; mkdir -p "$OUT_DIR/PearANGLE"
  # Copy libEGL/libGLESv2 and every @rpath library they pull in (component builds).
  cp "$ANGLE_DIR/LICENSE.ANGLE" "$OUT_DIR/PearANGLE/" 2>/dev/null || true
  todo=(libGLESv2.dylib libEGL.dylib)
  while ((${#todo[@]})); do
    lib="${todo[0]}"; todo=("${todo[@]:1}")
    [[ -f "$OUT_DIR/PearANGLE/$lib" ]] && continue
    cp "$ANGLE_DIR/$lib" "$OUT_DIR/PearANGLE/"
    while IFS= read -r dep; do todo+=("${dep#@rpath/}"); done < <(otool -L "$ANGLE_DIR/$lib" | awk '/@rpath\// {print $1}' | grep -v "@rpath/$lib\$")
  done
  cp "$ROOT/third_party/angle/LICENSE" "$OUT_DIR/PearANGLE/LICENSE.ANGLE"
  # ANGLE's LICENSE covers ANGLE only; the libraries also contain abseil, zlib,
  # xxHash and others whose licenses travel with them (as in build.sh).
  if [[ -f "$ANGLE_DIR/THIRD_PARTY_LICENSES.txt" ]]; then
    cp "$ANGLE_DIR/THIRD_PARTY_LICENSES.txt" "$OUT_DIR/PearANGLE/THIRD_PARTY_LICENSES.txt"
  else
    ANGLE_LICENSE_ARGS=(--angle-root "$ROOT/third_party/angle" --out "$OUT_DIR/PearANGLE/THIRD_PARTY_LICENSES.txt")
    ANGLE_NINJA="$ROOT/third_party/angle/out/PearHostMetal"
    if [[ -f "$ANGLE_NINJA/build.ninja" ]]; then
      ANGLE_LICENSE_ARGS+=(--ninja-dir "$ANGLE_NINJA" --target libEGL.dylib --target libGLESv2.dylib)
    else
      for component in abseil-cpp astc-encoder perfetto volk xxhash zlib; do
        ANGLE_LICENSE_ARGS+=(--component "$component")
      done
    fi
    python3 "$ROOT/collect-angle-licenses.py" "${ANGLE_LICENSE_ARGS[@]}"
  fi
  echo "   ANGLE: $(ls "$OUT_DIR/PearANGLE" | grep -c dylib) libraries"
  GLES_LINK=(-Wl,-rpath,@loader_path/../PearANGLE)
else
  echo "!! no PearANGLE build at $ANGLE_DIR; OpenGLES.framework has no GL implementation (run host/build-pearangle.sh --sync for GLES)" >&2
fi
"${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_GLES_LIB" \
  "$OUT_DIR/UIKit.framework/UIKit" ${GLES_LINK[@]+"${GLES_LINK[@]}"} \
  -o "$OUT_DIR/OpenGLES.framework/OpenGLES" -install_name @rpath/OpenGLES.framework/OpenGLES \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework QuartzCore
echo ">> $OUT_DIR/OpenGLES.framework/OpenGLES"

# GLKit.framework: GLKView and GLKViewController on PearANGLE (Rust pear-glkit),
# re-exporting macOS GLKit for GLKBaseEffect / GLKMath / GLKTextureLoader.
mkdir -p "$OUT_DIR/GLKit.framework"
"${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_GLKIT_LIB" \
  "$OUT_DIR/UIKit.framework/UIKit" "$OUT_DIR/OpenGLES.framework/OpenGLES" \
  -Wl,-reexport_framework,GLKit \
  -o "$OUT_DIR/GLKit.framework/GLKit" -install_name @rpath/GLKit.framework/GLKit \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework QuartzCore
echo ">> $OUT_DIR/GLKit.framework/GLKit"

# MetalKit.framework: iOS apps expect MTKView to be a UIView (Rust pear-metalkit).
mkdir -p "$OUT_DIR/MetalKit.framework"
"${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_MTK_LIB" \
  "$OUT_DIR/UIKit.framework/UIKit" \
  -o "$OUT_DIR/MetalKit.framework/MetalKit" -install_name @rpath/MetalKit.framework/MetalKit \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework Metal -framework QuartzCore
echo ">> $OUT_DIR/MetalKit.framework/MetalKit"

if [[ -f "$RUST_LIBS/libpear_controller.dylib" ]]; then
  cp -f "$RUST_LIBS/libpear_controller.dylib" "$OUT_DIR/libpear_controller.dylib"
  install_name_tool -id @rpath/libpear_controller.dylib "$OUT_DIR/libpear_controller.dylib" 2>/dev/null || true
  echo ">> $OUT_DIR/libpear_controller.dylib"
fi

# Pre-sign host outputs so runtime preparation can clone them directly without
# per-framework codesign subprocesses.
for fw in "$OUT_DIR"/*.framework; do
  name="$(basename "$fw" .framework)"
  [[ -f "$fw/$name" ]] && codesign --force --sign - "$fw/$name" 2>/dev/null || true
done
for lib in "$OUT_DIR"/*.dylib "$OUT_DIR"/PearANGLE/*.dylib; do
  [[ -f "$lib" ]] && codesign --force --sign - "$lib" 2>/dev/null || true
done
