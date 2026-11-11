#!/bin/bash
# Builds PearRuntime (libPearRuntime.dylib) for the Catalyst target (arm64).
#
# The dylib must match the host process, which is an iOS binary, so it is
# compiled for the Catalyst target. The iOS frameworks ship inside the macOS
# SDK's iOSSupport directory, so this works with the Command Line Tools alone
# as well as with a full toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="${PEAR_BUILD_DIR:-$ROOT/build}"
case "$OUT_DIR" in /*) ;; *) OUT_DIR="$ROOT/$OUT_DIR" ;; esac
OUT="$OUT_DIR/libPearRuntime.dylib"
SWIFT_OVERLAY="$OUT_DIR/libswiftUIKit.dylib"
CXX_COMPAT="$OUT_DIR/libcxxcompat.dylib"
DEVICE_PROFILE="$OUT_DIR/PearDeviceProfile.o"

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
OBJ_DIR="$OUT_DIR/PearRuntimeObjects"
rm -rf "$OBJ_DIR"
mkdir -p "$OBJ_DIR"
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION" 2>/dev/null || echo dev)"

# Apple silicon tuning. `apple-m1` is the baseline every Apple silicon Mac
# implements (M1 through current M-series and A-series Macs), so the code is
# scheduled for Apple's cores without emitting instructions a newer chip
# alone has. PEAR_RUNTIME_CPU overrides it for experiments.
CPU="${PEAR_RUNTIME_CPU:-apple-m1}"
OPT="${PEAR_RUNTIME_OPT:--O2}"
TARGET="arm64-apple-ios15.0-macabi"
COMMON=(-target "$TARGET" -isysroot "$SDK" -iframework "$IOS_FRAMEWORKS"
        -I"$ROOT/include" -std=c++17 -fobjc-arc "$OPT" -mcpu="$CPU"
        -fno-omit-frame-pointer -DPEAR_RUNTIME_VERSION="\"$VERSION\"")

# Module layout: modules/<module>/*.mm. Every file is compiled with its module
# name so its initializer can be switched off at launch (PEAR_RUNTIME_DISABLE).
MODULES=(core input graphics device storage services)

# Emit a compilation database so editors/clangd resolve UIKit for these files.
{
  echo "["
  first=1
  for module in "${MODULES[@]}"; do
    for source in "$ROOT/modules/$module"/*.mm; do
      [[ "$(basename "$source")" == DeviceProfile.mm ]] && continue
      [[ $first -eq 1 ]] || echo ","
      first=0
      printf '  {"directory": "%s", "file": "%s", "command": "%s %s -DPEAR_RUNTIME_MODULE=\\"%s\\" -include PearRuntime.h -c %s"}' \
        "$ROOT" "$source" "$CXX" "${COMMON[*]//\"/\\\"}" "$module" "$source"
    done
  done
  echo
  echo "]"
} > "$ROOT/compile_commands.json"

# Do not force-include PearRuntime.h into the early device snapshot. Compiling
# it as plain C++ also keeps ObjC/Foundation initialization out of this object.
xcrun --sdk macosx clang++ \
  -x c++ -target "$TARGET" -isysroot "$SDK" "$OPT" -mcpu="$CPU" \
  -I"$ROOT/include" -std=c++17 -c "$ROOT/modules/device/DeviceProfile.mm" \
  -o "$DEVICE_PROFILE"

echo ">> Compiling PearRuntime $VERSION modules (${MODULES[*]}) for $CPU"
JOBS="$(sysctl -n hw.perflevel0.physicalcpu 2>/dev/null || sysctl -n hw.ncpu)"
pids=()
objects=()
for module in "${MODULES[@]}"; do
  for source in "$ROOT/modules/$module"/*.mm; do
    [[ "$(basename "$source")" == DeviceProfile.mm ]] && continue
    object="$OBJ_DIR/$module-$(basename "${source%.mm}").o"
    objects+=("$object")
    "$CXX" "${COMMON[@]}" -DPEAR_RUNTIME_MODULE="\"$module\"" -include PearRuntime.h \
      -c "$source" -o "$object" &
    pids+=($!)
    if (( ${#pids[@]} >= JOBS )); then
      wait "${pids[0]}"
      pids=("${pids[@]:1}")
    fi
  done
done
for pid in "${pids[@]}"; do wait "$pid"; done

echo ">> Linking $OUT (Mac Catalyst)"
"$CXX" -target "$TARGET" -isysroot "$SDK" -iframework "$IOS_FRAMEWORKS" \
  -dynamiclib \
  -Wl,-undefined,dynamic_lookup \
  -Wl,-dead_strip \
  -fobjc-arc \
  -framework Foundation \
  -framework UIKit \
  -framework GameController \
  -framework QuartzCore \
  -framework Metal \
  -framework CoreGraphics \
  -framework Security \
  -framework AppKit \
  -framework AudioToolbox \
  -install_name @rpath/libPearRuntime.dylib \
  -current_version "${VERSION%%-*}" \
  -o "$OUT" \
  "${objects[@]}" "$DEVICE_PROFILE"

echo ">> Done: $OUT"
file "$OUT"

# A few Catalyst binaries carry a weak C++ placeholder that the host runtime
# does not export. This small library re-exports the system C++ runtime and
# supplies the missing empty tag object.
echo ">> Building $CXX_COMPAT (C++ runtime compatibility)"
xcrun --sdk macosx clang++ \
  -target arm64-apple-ios15.0-macabi \
  -isysroot "$SDK" \
  -dynamiclib \
  -Wl,-reexport_library,"$SDK/usr/lib/libc++.1.tbd" \
  -install_name @rpath/libcxxcompat.dylib \
  -o "$CXX_COMPAT" \
  "$ROOT/compat/cxxcompat.cpp"
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
