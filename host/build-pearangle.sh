#!/bin/bash
# Builds PearANGLE (third_party/angle, branch apple-silicon) for PearHost:
# macOS platform, Apple silicon (arm64), Metal only. The other ANGLE back ends
# and platforms are removed from that branch. Output: build/host/PearANGLE/{libEGL,libGLESv2}.dylib, which
# host/build-host.sh then uses for OpenGLES.framework.
#
#   host/build-pearangle.sh --sync   # first time: fetch depot_tools and deps
#   host/build-pearangle.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/third_party/angle"
TOOLS="${PEAR_DEPOT_TOOLS:-$ROOT/build/angle-tools/depot_tools}"
OUT_NAME=PearHostMetal
[[ -f "$SRC/DEPS" ]] || { echo "run: git submodule update --init --depth 1 third_party/angle" >&2; exit 2; }
if [[ "${1:-}" == --sync ]]; then
  [[ -f "$TOOLS/gclient.py" ]] || { mkdir -p "$(dirname "$TOOLS")"; git clone --depth 1 https://chromium.googlesource.com/chromium/tools/depot_tools.git "$TOOLS"; }
  [[ -f "$SRC/.gclient" ]] || cat > "$SRC/.gclient" <<'GC'
solutions = [{ "name": ".", "url": None, "deps_file": "DEPS", "managed": False, "custom_vars": {} }]
GC
  (cd "$SRC" && PATH="$TOOLS:$PATH" "$TOOLS/gclient" sync --nohooks --shallow --no-history)
  (cd "$SRC" && PATH="$TOOLS:$PATH" DEPOT_TOOLS_UPDATE=0 "$TOOLS/gclient" runhooks)
fi
mkdir -p "$SRC/out/$OUT_NAME"
cp "$ROOT/host/pearangle-macos-args.gn" "$SRC/out/$OUT_NAME/args.gn"
(cd "$SRC" && PATH="$TOOLS:$PATH" DEPOT_TOOLS_UPDATE=0 ./buildtools/mac/gn gen "out/$OUT_NAME")
NINJA="$(command -v ninja || echo "$TOOLS/ninja")"
"$NINJA" -C "$SRC/out/$OUT_NAME" libEGL libGLESv2
DEST="${PEAR_BUILD_DIR:-$ROOT/build}/host/PearANGLE-src"
mkdir -p "$DEST"
cp "$SRC/out/$OUT_NAME/libEGL.dylib" "$SRC/out/$OUT_NAME/libGLESv2.dylib" "$DEST/"
# ANGLE's standalone build names its dylibs "./lib*.dylib"; apps load them by @rpath.
for lib in libEGL libGLESv2; do
  install_name_tool -id "@rpath/$lib.dylib" "$DEST/$lib.dylib"
  codesign -f -s - "$DEST/$lib.dylib" 2>/dev/null
done
cp "$SRC/LICENSE" "$DEST/LICENSE.ANGLE"
echo ">> $DEST (PearANGLE $(git -C "$SRC" rev-parse --short HEAD), macOS arm64, Metal)"
