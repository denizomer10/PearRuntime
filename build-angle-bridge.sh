#!/bin/bash
# Source-built Catalyst GLES/EAGL ABI bridge; no system OpenGLES dependency.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
runtime="${1:-$root/build/libPearRuntime.dylib}"
out="$(cd "$(dirname "$runtime")" && pwd)"
sdk="${PEAR_MACOS_SDK:-$(xcrun --sdk macosx --show-sdk-path)}"
python3 "$root/generate-angle-forwarders.py" "$out/PearGLESForwarders.mm"
xcrun clang++ -std=c++17 -fobjc-arc -O2 -target arm64-apple-ios15.0-macabi \
  -isysroot "$sdk" -iframework "$sdk/System/iOSSupport/System/Library/Frameworks" \
  -I"$root/include" -I"$root/third_party/angle/include" -dynamiclib \
  "$out/PearGLESForwarders.mm" "$root/bridge/EAGLExports.mm" \
  -framework Foundation -Wl,-reexport_library,"$runtime" -Wl,-rpath,@loader_path \
  -install_name @rpath/libPearOpenGLES.dylib -o "$out/libPearOpenGLES.dylib"
file "$out/libPearOpenGLES.dylib"
