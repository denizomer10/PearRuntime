#!/bin/bash
# Builds PearRuntime (libPearRuntime.dylib) for the Catalyst target (arm64).
#
# The dylib must match the host process, which is an iOS binary, so it is
# compiled for the Catalyst target. The iOS frameworks ship inside the macOS
# SDK's iOSSupport directory, so this works with the Command Line Tools alone
# as well as with a full toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
source "$ROOT/bridge/context-aliases.sh"
if [[ ! -f "$ROOT/third_party/angle/include/EGL/egl.h" ]]; then
  echo "ERROR: ANGLE headers are missing. Initialize the pinned fork:" >&2
  echo "  git -C '$ROOT' submodule update --init --depth 1 third_party/angle" >&2
  exit 1
fi
OUT_DIR="${PEAR_BUILD_DIR:-$ROOT/build}"
case "$OUT_DIR" in /*) ;; *) OUT_DIR="$ROOT/$OUT_DIR" ;; esac
OUT="$OUT_DIR/libPearRuntime.dylib"
SWIFT_OVERLAY="$OUT_DIR/libswiftUIKit.dylib"
CXX_COMPAT="$OUT_DIR/libcxxcompat.dylib"

SDK="${PEAR_MACOS_SDK:-$(xcrun --sdk macosx --show-sdk-path)}"
IOS_FRAMEWORKS="$SDK/System/iOSSupport/System/Library/Frameworks"
if [[ ! -d "$IOS_FRAMEWORKS" ]]; then
  echo "ERROR: the macOS SDK does not include the iOSSupport frameworks."
  echo "       SDK: $SDK"
  echo "       Install the Command Line Tools or a full toolchain with macOS SDK 26 or later."
  exit 1
fi

CXX="$(xcrun --find clang++)"
mkdir -p "$OUT_DIR"
rm -rf "$OUT_DIR/PearRuntimeObjects" # Objective-C++ objects of earlier builds
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION" 2>/dev/null || echo dev)"
TARGET="arm64-apple-ios15.0-macabi"

# Every runtime module (core input graphics graphics-gl graphics-metal device
# storage services performance) is Rust: rust/crates/pear-catalyst/src/<module>.
# Rust ships no prebuilt standard library for Mac Catalyst, so std is built
# from the toolchain's rust-src.
RUST_TARGET_DIR="$OUT_DIR/rust-catalyst"
echo ">> Building PearRuntime $VERSION (pear-catalyst, aarch64-apple-ios-macabi)"
PEAR_RUNTIME_VERSION="$VERSION" IPHONEOS_DEPLOYMENT_TARGET=15.0 RUSTC_BOOTSTRAP=1 \
  cargo build --manifest-path "$ROOT/rust/Cargo.toml" --release -p pear-catalyst \
  --target aarch64-apple-ios-macabi -Zbuild-std=std,panic_abort --target-dir "$RUST_TARGET_DIR"
RUST_LIB="$RUST_TARGET_DIR/aarch64-apple-ios-macabi/release/libpear_catalyst.a"

echo ">> Linking $OUT (Mac Catalyst)"
"$CXX" -target "$TARGET" -isysroot "$SDK" -iframework "$IOS_FRAMEWORKS" \
  -dynamiclib \
  -Wl,-undefined,dynamic_lookup \
  -Wl,-dead_strip \
  "${PEAR_EAGL_ALIAS_FLAGS[@]}" \
  -framework Foundation \
  -framework UIKit \
  -framework GameController \
  -framework QuartzCore \
  -framework Metal \
  -framework CoreGraphics \
  -framework Security \
  -framework AppKit \
  -framework AudioToolbox \
  -framework IOKit \
  -install_name @rpath/libPearRuntime.dylib \
  -current_version "${VERSION%%-*}" \
  -o "$OUT" \
  -Wl,-force_load,"$RUST_LIB"

# -undefined dynamic_lookup turns a misspelled or nonexistent import into a
# flat-namespace lookup that fails at load and makes dyld drop every interpose
# tuple. Only the CoreAudio HAL calls (no Catalyst stub) may resolve that way.
UNRESOLVED="$(dyld_info -imports "$OUT" | awk '/flat-namespace/ {print $1}' | grep -vE '^_AudioObject(Get|Set)PropertyData$' || true)"
if [[ -n "$UNRESOLVED" ]]; then
  echo "ERROR: unresolved flat-namespace imports in $OUT:" >&2
  echo "$UNRESOLVED" >&2
  exit 1
fi

echo ">> Done: $OUT"
file "$OUT"

# A few Catalyst binaries carry a weak C++ placeholder that the host runtime
# does not export. This small library re-exports the system C++ runtime and
# supplies the missing empty tag object.
echo ">> Building $CXX_COMPAT (C++ runtime compatibility)"
IPHONEOS_DEPLOYMENT_TARGET=15.0 RUSTC_BOOTSTRAP=1 \
  cargo build --manifest-path "$ROOT/rust/Cargo.toml" --release -p pear-cxxcompat \
  --target aarch64-apple-ios-macabi -Zbuild-std=std,panic_abort --target-dir "$RUST_TARGET_DIR"
"$CXX" -target "$TARGET" -isysroot "$SDK" \
  -dynamiclib -Wl,-dead_strip \
  -Wl,-reexport_library,"$SDK/usr/lib/libc++.1.tbd" \
  -Wl,-force_load,"$RUST_TARGET_DIR/aarch64-apple-ios-macabi/release/libpear_cxxcompat.a" \
  -install_name @rpath/libcxxcompat.dylib \
  -o "$CXX_COMPAT"
file "$CXX_COMPAT"

# Some Swift SDKs link against the UIKit Swift overlay, which the Mac Catalyst
# host runtime does not ship. Build the small source-built overlay used by the
# installer. It contains only public API bridges from compat/swift/.
echo ">> Building $SWIFT_OVERLAY (Swift UIKit compatibility)"
CLANG_TARGET="arm64-apple-ios15.0-macabi"
CLANG="$(xcrun --find clang)"
SWIFTC="$(xcrun --find swiftc)"
"$CLANG" -c -target "$CLANG_TARGET" -isysroot "$SDK" \
  "$ROOT/compat/swift/PearSwiftUIKitSupport.c" \
  -o "$OUT_DIR/PearSwiftUIKitSupport.o"
# The numeric raw-representable ABI must live in the `UIKit` module, not just
# in this dylib. Compile it as its own module and alias the system headers so
# the mangled protocol/conformance/witness symbols match the original overlay.
"$SWIFTC" -target "$CLANG_TARGET" -sdk "$SDK" -F "$IOS_FRAMEWORKS" \
  -parse-as-library -emit-object -module-name PearSwiftUIKit \
  -o "$OUT_DIR/PearSwiftUIKit.o" \
  "$ROOT/compat/swift/PearSwiftUIKit.swift"
"$SWIFTC" -target "$CLANG_TARGET" -sdk "$SDK" -F "$IOS_FRAMEWORKS" \
  -parse-as-library -emit-object -module-name UIKit \
  -module-alias AppleUIKit=UIKit \
  -o "$OUT_DIR/PearUIKitNumeric.o" \
  "$ROOT/compat/swift/PearUIKitNumeric.swift"
"$SWIFTC" -target "$CLANG_TARGET" -sdk "$SDK" -F "$IOS_FRAMEWORKS" \
  -parse-as-library -emit-library \
  -Xlinker -install_name -Xlinker @rpath/libswiftUIKit.dylib \
  -Xlinker -undefined -Xlinker dynamic_lookup \
  -o "$SWIFT_OVERLAY" \
  "$OUT_DIR/PearSwiftUIKit.o" \
  "$OUT_DIR/PearUIKitNumeric.o" \
  "$OUT_DIR/PearSwiftUIKitSupport.o"
file "$SWIFT_OVERLAY"

# Some store builds link MarketplaceKit, which the host ships as resources
# only. Without a binary the dynamic linker cannot resolve the weak Swift
# references and aborts while registering metadata. The installer copies this
# stub into apps that need it; `-enable-library-evolution` is required for the
# enum case constructors the already-compiled client binds to.
echo ">> Building MarketplaceKit.framework (distribution metadata stub)"
MARKETPLACE="$OUT_DIR/MarketplaceKit.framework"
rm -rf "$MARKETPLACE"
mkdir -p "$MARKETPLACE/Versions/A/Resources"
"$SWIFTC" -target "$CLANG_TARGET" -sdk "$SDK" -F "$IOS_FRAMEWORKS" \
  -emit-library -module-name MarketplaceKit -enable-library-evolution \
  -Xlinker -install_name -Xlinker @rpath/MarketplaceKit.framework/MarketplaceKit \
  -Xlinker -undefined -Xlinker dynamic_lookup \
  -o "$MARKETPLACE/Versions/A/MarketplaceKit" \
  "$ROOT/compat/swift/MarketplaceKit.swift"
ln -s A "$MARKETPLACE/Versions/Current"
ln -s Versions/Current/MarketplaceKit "$MARKETPLACE/MarketplaceKit"
ln -s Versions/Current/Resources "$MARKETPLACE/Resources"
cat > "$MARKETPLACE/Versions/A/Resources/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>MarketplaceKit</string>
  <key>CFBundleIdentifier</key><string>com.pearsilicon.marketplacekit</string>
  <key>CFBundlePackageType</key><string>FMWK</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
</dict>
</plist>
PLIST
file "$MARKETPLACE/MarketplaceKit"
bash "$ROOT/build-angle-bridge.sh" "$OUT"

# --- ANGLE (Mac Catalyst) ---------------------------------------------------
# Every compatibility app carries its own ANGLE next to libPearOpenGLES; the
# system OpenGLES framework is never used. Stage the source-built Catalyst
# dylibs (scripts/build-angle-metal.sh -> out/PearCatalyst) with ANGLE's
# license. A macOS build must not be passed off as Catalyst: the platform is
# checked, never patched.
ANGLE_SRC="${PEAR_ANGLE_CATALYST_DIR:-$ROOT/third_party/angle/out/PearCatalyst}"
ANGLE_STAGE="$OUT_DIR/PearANGLE"
echo ">> Staging ANGLE (Mac Catalyst) from $ANGLE_SRC"
for lib in libEGL.dylib libGLESv2.dylib; do
  if [[ ! -f "$ANGLE_SRC/$lib" ]]; then
    echo "ERROR: $ANGLE_SRC/$lib is missing. Build it with:" >&2
    echo "  bash scripts/build-angle-metal.sh   (from the PearSilicon checkout)" >&2
    exit 1
  fi
  if ! xcrun vtool -show-build "$ANGLE_SRC/$lib" 2>/dev/null | grep -q "platform MACCATALYST"; then
    echo "ERROR: $ANGLE_SRC/$lib is not a Mac Catalyst binary." >&2
    exit 1
  fi
done
rm -rf "$ANGLE_STAGE"
mkdir -p "$ANGLE_STAGE"
for lib in libEGL.dylib libGLESv2.dylib; do
  cp "$ANGLE_SRC/$lib" "$ANGLE_STAGE/$lib"
  xcrun install_name_tool -id "@rpath/PearANGLE/$lib" "$ANGLE_STAGE/$lib" 2>/dev/null
done
cp "$ROOT/third_party/angle/LICENSE" "$ANGLE_STAGE/LICENSE.ANGLE"
# ANGLE's LICENSE covers ANGLE only; the dylibs also contain abseil, zlib,
# xxHash, astc-encoder and others whose licenses must travel with them. Read
# the link inputs from the build when it is available, so a new dependency
# cannot slip in without its license.
ANGLE_LICENSE_ARGS=(--angle-root "$ROOT/third_party/angle" --out "$ANGLE_STAGE/THIRD_PARTY_LICENSES.txt")
if [[ -f "$ANGLE_SRC/build.ninja" ]]; then
  ANGLE_LICENSE_ARGS+=(--ninja-dir "$ANGLE_SRC" --target libEGL.dylib --target libGLESv2.dylib)
else
  for component in abseil-cpp astc-encoder perfetto volk xxhash zlib; do
    ANGLE_LICENSE_ARGS+=(--component "$component")
  done
fi
python3 "$ROOT/collect-angle-licenses.py" "${ANGLE_LICENSE_ARGS[@]}"
file "$ANGLE_STAGE"/*.dylib
