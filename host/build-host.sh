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

mkdir -p "$OUT_DIR/UIKit.framework"

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
         "$ROOT"/host/uikit/backend/appkit/*.m "$ROOT"/host/uikit/generated/*.m "$ROOT"/host/uikit/generated/*.s)
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

"${CC[@]}" -dynamiclib "${objs[@]}" -o "$OUT_DIR/UIKit.framework/UIKit" \
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
# points are re-exported from ANGLE; EAGL symbols are ours (host/opengles).
# PEAR_HOST_ANGLE_DIR must hold macOS-platform libEGL/libGLESv2 builds.
ANGLE_DIR="${PEAR_HOST_ANGLE_DIR:-$HOME/PearLegacyEngine/vendor/angle/out/MacMetal}"
if [[ -f "$ANGLE_DIR/libGLESv2.dylib" ]]; then
  rm -rf "$OUT_DIR/PearANGLE"; mkdir -p "$OUT_DIR/PearANGLE" "$OUT_DIR/OpenGLES.framework"
  # Copy libEGL/libGLESv2 and every @rpath library they pull in (component builds).
  todo=(libGLESv2.dylib libEGL.dylib)
  while ((${#todo[@]})); do
    lib="${todo[0]}"; todo=("${todo[@]:1}")
    [[ -f "$OUT_DIR/PearANGLE/$lib" ]] && continue
    cp "$ANGLE_DIR/$lib" "$OUT_DIR/PearANGLE/"
    while IFS= read -r dep; do todo+=("${dep#@rpath/}"); done < <(otool -L "$ANGLE_DIR/$lib" | awk '/@rpath\// {print $1}' | grep -v "@rpath/$lib\$")
  done
  echo "   ANGLE: $(ls "$OUT_DIR/PearANGLE" | wc -l | tr -d ' ') libraries"
  "${CC[@]}" -fobjc-arc -dynamiclib "$ROOT/host/opengles/EAGL.m" "$OUT_DIR/UIKit.framework/UIKit" \
    -o "$OUT_DIR/OpenGLES.framework/OpenGLES" -install_name @rpath/OpenGLES.framework/OpenGLES \
    -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation \
    -L"$OUT_DIR/PearANGLE" -Wl,-reexport-lGLESv2 -Wl,-rpath,@loader_path/../PearANGLE
  echo ">> $OUT_DIR/OpenGLES.framework/OpenGLES (GL from $ANGLE_DIR)"
else
  echo "!! no macOS ANGLE build at $ANGLE_DIR; OpenGLES.framework not built" >&2
fi

# MetalKit.framework: iOS apps expect MTKView to be a UIView (host/metalkit).
mkdir -p "$OUT_DIR/MetalKit.framework"
"${CC[@]}" -fobjc-arc -Wall -dynamiclib "$ROOT/host/metalkit/MTKView.m" "$OUT_DIR/UIKit.framework/UIKit" \
  -I"$ROOT/host/uikit/include" -I"$ROOT/host/core" \
  -o "$OUT_DIR/MetalKit.framework/MetalKit" -install_name @rpath/MetalKit.framework/MetalKit \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework Metal -framework QuartzCore
echo ">> $OUT_DIR/MetalKit.framework/MetalKit"
