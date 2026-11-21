#!/bin/bash
# Rebuilds fixture-objc.dylib (arm64 iOS 15, ARC, libobjc only (auto-linked);
# checked in, so tests need no SDK).
set -euo pipefail
cd "$(dirname "$0")"
xcrun --sdk iphoneos clang -target arm64-apple-ios15.0 -isysroot "$(xcrun --sdk iphoneos --show-sdk-path)" \
  -O1 -fobjc-arc -dynamiclib fixture.m -install_name @rpath/fixture-objc.dylib -o fixture-objc.dylib
ls -l fixture-objc.dylib
