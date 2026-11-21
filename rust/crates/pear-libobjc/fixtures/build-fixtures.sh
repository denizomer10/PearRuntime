#!/bin/bash
# Rebuilds fixture-objc.dylib and fixture2-objc.dylib (arm64 iOS 15, ARC,
# libobjc only (auto-linked); checked in, so tests need no SDK).
set -euo pipefail
cd "$(dirname "$0")"
SDK="$(xcrun --sdk iphoneos --show-sdk-path)"
xcrun --sdk iphoneos clang -target arm64-apple-ios15.0 -isysroot "$SDK" \
  -O1 -fobjc-arc -dynamiclib fixture.m -install_name @rpath/fixture-objc.dylib -o fixture-objc.dylib
xcrun --sdk iphoneos clang -target arm64-apple-ios15.0 -isysroot "$SDK" \
  -O1 -fobjc-arc -dynamiclib fixture2.m -install_name @rpath/fixture2-objc.dylib -o fixture2-objc.dylib
ls -l fixture-objc.dylib fixture2-objc.dylib
