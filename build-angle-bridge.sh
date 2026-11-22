#!/bin/bash
# libPearOpenGLES.dylib: the source-built Catalyst GLES/EAGL ABI bridge
# (rust/crates/pear-opengles), re-exporting libPearRuntime. No system OpenGLES
# dependency: every GLES entry point resolves to ANGLE through the runtime.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
runtime="${1:-$root/build/libPearRuntime.dylib}"
out="$(cd "$(dirname "$runtime")" && pwd)"
sdk="${PEAR_MACOS_SDK:-$(xcrun --sdk macosx --show-sdk-path)}"
target_dir="$out/rust-catalyst"
IPHONEOS_DEPLOYMENT_TARGET=15.0 RUSTC_BOOTSTRAP=1 \
  cargo build --manifest-path "$root/rust/Cargo.toml" --release -p pear-opengles \
  --target aarch64-apple-ios-macabi -Zbuild-std=std,panic_abort --target-dir "$target_dir"
xcrun clang -target arm64-apple-ios15.0-macabi -isysroot "$sdk" \
  -iframework "$sdk/System/iOSSupport/System/Library/Frameworks" -dynamiclib -Wl,-dead_strip \
  -Wl,-force_load,"$target_dir/aarch64-apple-ios-macabi/release/libpear_opengles.a" \
  -framework Foundation -Wl,-reexport_library,"$runtime" -Wl,-rpath,@loader_path \
  -install_name @rpath/libPearOpenGLES.dylib -o "$out/libPearOpenGLES.dylib"
file "$out/libPearOpenGLES.dylib"
