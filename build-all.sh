#!/bin/bash
# Unified PearRuntime build:
#   1. PearANGLE (Metal-only arm64, if --angle or --sync-angle is passed)
#   2. PearHost (Rust pear-core + pear-macho, Swift UIKit / MetalKit / OpenGLES / AppKitBackend)
#   3. PearLegacyEngine + PearLegacyJIT (32-bit ARMv7 -> arm64 Rust + Dynarmic runtime)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="${PEAR_BUILD_DIR:-$ROOT/build}/host"

BUILD_ANGLE=0
SYNC_ANGLE=0
BUILD_LEGACY=1

for arg in "$@"; do
  case "$arg" in
    --angle) BUILD_ANGLE=1 ;;
    --sync-angle) BUILD_ANGLE=1; SYNC_ANGLE=1 ;;
    --no-legacy) BUILD_LEGACY=0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

if [[ "$BUILD_ANGLE" == 1 ]]; then
  if [[ "$SYNC_ANGLE" == 1 ]]; then
    bash "$ROOT/host/build-pearangle.sh" --sync
  else
    bash "$ROOT/host/build-pearangle.sh"
  fi
fi

echo ">> Building PearHost (Rust + Swift)"
bash "$ROOT/host/build-host.sh"

if [[ "$BUILD_LEGACY" == 1 ]] && [[ -f "$ROOT/third_party/legacy-engine/Cargo.toml" ]]; then
  echo ">> Building PearLegacyEngine + PearLegacyJIT"
  JIT_DIR="$ROOT/third_party/legacy-jit"
  (
    cd "$ROOT/third_party/legacy-engine"
    PEAR_DYNARMIC_ROOT="$JIT_DIR" cargo build --release
  )
  mkdir -p "$OUT_DIR/PearLegacyEngine"
  cp "$ROOT/third_party/legacy-engine/target/release/touchHLE" "$OUT_DIR/PearLegacyEngine/"
  echo ">> $OUT_DIR/PearLegacyEngine/touchHLE"
fi
