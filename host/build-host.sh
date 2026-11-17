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
TARGET="arm64-apple-macos14.0"
CC=(xcrun --sdk macosx clang -target "$TARGET" -isysroot "$SDK" -O2 -mcpu=apple-m1 -fno-omit-frame-pointer
    -DPEAR_RUNTIME_VERSION="\"$VERSION\"")
SWIFTC=(xcrun --sdk macosx swiftc -target "$TARGET" -sdk "$SDK" -O -wmo -parse-as-library)
SWIFT_LINK=(-L"$SDK/usr/lib/swift" -Wl,-rpath,/usr/lib/swift)

mkdir -p "$OUT_DIR/UIKit.framework"

# Build the Rust core crate (pear-core: logging, missing-call tracking, JSONL).
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --manifest-path "$ROOT/rust/Cargo.toml" --release --target aarch64-apple-darwin --target-dir "$OUT_DIR/rust-target"
RUST_CORE_LIB="$OUT_DIR/rust-target/aarch64-apple-darwin/release/libpear_core.a"

# Regenerate stubs from every census report (not the *-selectors.json side files).
CENSUS=()
for f in "$ROOT"/docs/census/*.json; do
  [[ "$f" == *-selectors.json ]] || CENSUS+=("$f")
done
python3 -I "$ROOT/tools/gen-uikit-stubs.py" "${CENSUS[@]}" > "$OUT_DIR/stubs.log"
sed 's/^/   /' "$OUT_DIR/stubs.log"

OBJ="$OUT_DIR/obj"
rm -rf "$OBJ"
mkdir -p "$OBJ"

SOURCES=("$ROOT"/host/core/*.m "$ROOT"/host/core/*.s "$ROOT"/host/uikit/src/*.m
         "$ROOT"/host/uikit/generated/*.m "$ROOT"/host/uikit/generated/*.s)
# Platform-neutral Catalyst modules shared with PearHost (Foundation/Security only).
SHARED_MODULES=("$ROOT"/modules/storage/KeychainShim.mm)
objs=()
for src in "${SHARED_MODULES[@]}"; do
  o="$OBJ/shared-$(basename "$src").o"
  "${CC[@]}" -x objective-c++ -std=c++17 -fobjc-arc -Wno-deprecated-declarations -I"$ROOT/host/core" \
    -include "$ROOT/host/core/PearCatalystCompat.h" -c "$src" -o "$o"
  objs+=("$o")
done
for src in "${SOURCES[@]}"; do
  o="$OBJ/$(basename "$(dirname "$src")")-$(basename "$src").o"
  case "$src" in
    *.s) "${CC[@]}" -c "$src" -o "$o" ;;
    *)   "${CC[@]}" -fobjc-arc -Wall -Wno-unused-function -Wno-objc-missing-super-calls \
           -Wno-incomplete-implementation -Wno-objc-property-implementation -Wno-deprecated-declarations \
           -I"$ROOT/host/uikit/include" -I"$ROOT/host/core" -c "$src" -o "$o" ;;
  esac
  objs+=("$o")
done

SWIFT_SOURCES=("$ROOT"/host/uikit/src/*.swift "$ROOT"/host/uikit/backend/appkit/*.swift)
if ((${#SWIFT_SOURCES[@]})) && [[ -e "${SWIFT_SOURCES[0]}" ]]; then
  "${SWIFTC[@]}" -Xcc -DPEAR_SWIFT_HOST -module-name PearUIKitSwift -import-objc-header "$ROOT/host/uikit/src/PearUIKit-Bridging.h" \
    -emit-module -emit-module-path "$OBJ/PearUIKitSwift.swiftmodule" \
    -I"$ROOT/host/uikit/include" -I"$ROOT/host/uikit/src" -I"$ROOT/host/core" \
    -c "${SWIFT_SOURCES[@]}" -o "$OBJ/uikit-swift.o"
  objs+=("$OBJ/uikit-swift.o")
fi

"${CC[@]}" -dynamiclib "${objs[@]}" "$RUST_CORE_LIB" "${SWIFT_LINK[@]}" -o "$OUT_DIR/UIKit.framework/UIKit" \
  -install_name @rpath/UIKit.framework/UIKit -compatibility_version 1.0.0 -current_version 6209.0.0 \
  -framework Foundation -Wl,-reexport_framework,AppKit -framework QuartzCore -framework CoreGraphics -framework IOKit -framework Security -lc++
echo ">> $OUT_DIR/UIKit.framework/UIKit"

# Empty images for iOS-only frameworks an app links but does not call. If an
# app later imports a symbol from one of them, dyld names it and the census
# picks it up; then it gets a real implementation.
echo 'void pear_empty_framework(void) {}' > "$OBJ/empty.c"
for fw in iAd; do
  mkdir -p "$OUT_DIR/$fw.framework"
  "${CC[@]}" -dynamiclib "$OBJ/empty.c" -o "$OUT_DIR/$fw.framework/$fw" \
    -install_name "@rpath/$fw.framework/$fw" -compatibility_version 1.0.0 -current_version 1.0.0
  echo ">> $OUT_DIR/$fw.framework/$fw (empty)"
done

# OpenGLES.framework: macOS has none for macOS-platform processes. GL entry
# points are re-exported from PearANGLE (our fork, Metal only) when present;
# EAGL symbols are ours (host/opengles).
ANGLE_DIR="${PEAR_HOST_ANGLE_DIR:-$OUT_DIR/PearANGLE-src}"
mkdir -p "$OUT_DIR/OpenGLES.framework"
"${SWIFTC[@]}" -module-name PearOpenGLES -import-objc-header "$ROOT/host/core/PearHost.h" \
  -I"$ROOT/host/core" -c "$ROOT/host/opengles/EAGL.swift" -o "$OBJ/eagl-swift.o"
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
  echo "   ANGLE: $(ls "$OUT_DIR/PearANGLE" | wc -l | tr -d ' ') libraries"
  GLES_LINK=(-L"$OUT_DIR/PearANGLE" -Wl,-reexport-lGLESv2 -Wl,-rpath,@loader_path/../PearANGLE)
else
  echo "!! no PearANGLE build at $ANGLE_DIR; building OpenGLES.framework without re-exported GL symbols (run host/build-pearangle.sh --sync for GLES)" >&2
fi
"${CC[@]}" -fobjc-arc -dynamiclib "$ROOT/host/opengles/EAGLGlobals.m" "$OBJ/eagl-swift.o" \
  "$OUT_DIR/UIKit.framework/UIKit" "${SWIFT_LINK[@]}" ${GLES_LINK[@]+"${GLES_LINK[@]}"} \
  -o "$OUT_DIR/OpenGLES.framework/OpenGLES" -install_name @rpath/OpenGLES.framework/OpenGLES \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework QuartzCore
echo ">> $OUT_DIR/OpenGLES.framework/OpenGLES"

# MetalKit.framework: iOS apps expect MTKView to be a UIView (host/metalkit).
mkdir -p "$OUT_DIR/MetalKit.framework"
"${SWIFTC[@]}" -module-name PearMetalKit -I"$OBJ" \
  -c "$ROOT/host/metalkit/MTKView.swift" -o "$OBJ/mtkview-swift.o"
"${CC[@]}" -Wall -dynamiclib "$OBJ/mtkview-swift.o" "$ROOT/host/metalkit/MTKLoad.c" \
  "$OUT_DIR/UIKit.framework/UIKit" "${SWIFT_LINK[@]}" \
  -o "$OUT_DIR/MetalKit.framework/MetalKit" -install_name @rpath/MetalKit.framework/MetalKit \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework Metal -framework QuartzCore
echo ">> $OUT_DIR/MetalKit.framework/MetalKit"
