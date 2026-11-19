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
RUST_MTK_LIB="$RUST_LIBS/libpear_metalkit.a"
RUST_UIKIT_LIB="$RUST_LIBS/libpear_uikit.a"

rm -rf "$OUT_DIR/obj" # objects of earlier builds (Objective-C sources)

"${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_UIKIT_LIB" -o "$OUT_DIR/UIKit.framework/UIKit" \
  -install_name @rpath/UIKit.framework/UIKit -compatibility_version 1.0.0 -current_version 6209.0.0 \
  -framework Foundation -Wl,-reexport_framework,AppKit -framework QuartzCore -framework CoreGraphics -framework CoreText -framework ImageIO -framework IOKit -framework Security -lc++
echo ">> $OUT_DIR/UIKit.framework/UIKit"

# Empty images for iOS-only frameworks an app links but does not call. If an
# app later imports a symbol from one of them, dyld names it and the census
# picks it up; then it gets a real implementation.
for fw in iAd; do
  mkdir -p "$OUT_DIR/$fw.framework"
  "${LD[@]}" -dynamiclib -Wl,-u,_pear_empty_framework "$RUST_LIBS/libpear_empty_framework.a" -Wl,-dead_strip \
    -Wl,-exported_symbol,_pear_empty_framework -o "$OUT_DIR/$fw.framework/$fw" \
    -install_name "@rpath/$fw.framework/$fw" -compatibility_version 1.0.0 -current_version 1.0.0
  echo ">> $OUT_DIR/$fw.framework/$fw (empty)"
done

# OpenGLES.framework: macOS has none for macOS-platform processes. GL entry
# points are re-exported from PearANGLE (our fork, Metal only) when present;
# EAGL implementation is in Rust (pear-gles).
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
  echo "   ANGLE: $(ls "$OUT_DIR/PearANGLE" | grep -c dylib) libraries"
  GLES_LINK=(-L"$OUT_DIR/PearANGLE" -Wl,-reexport-lGLESv2 -Wl,-rpath,@loader_path/../PearANGLE)
else
  echo "!! no PearANGLE build at $ANGLE_DIR; building OpenGLES.framework without re-exported GL symbols (run host/build-pearangle.sh --sync for GLES)" >&2
fi
"${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_GLES_LIB" \
  "$OUT_DIR/UIKit.framework/UIKit" ${GLES_LINK[@]+"${GLES_LINK[@]}"} \
  -o "$OUT_DIR/OpenGLES.framework/OpenGLES" -install_name @rpath/OpenGLES.framework/OpenGLES \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework QuartzCore
echo ">> $OUT_DIR/OpenGLES.framework/OpenGLES"

# MetalKit.framework: iOS apps expect MTKView to be a UIView (Rust pear-metalkit).
mkdir -p "$OUT_DIR/MetalKit.framework"
"${LD[@]}" -dynamiclib -Wl,-force_load,"$RUST_MTK_LIB" \
  "$OUT_DIR/UIKit.framework/UIKit" \
  -o "$OUT_DIR/MetalKit.framework/MetalKit" -install_name @rpath/MetalKit.framework/MetalKit \
  -compatibility_version 1.0.0 -current_version 1.0.0 -framework Foundation -framework Metal -framework QuartzCore
echo ">> $OUT_DIR/MetalKit.framework/MetalKit"
