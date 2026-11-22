#!/bin/bash
# Rebuilds the loader test fixtures (checked in, so tests need no SDK):
# fixture-dyldinfo.dylib (iOS 12: LC_DYLD_INFO_ONLY opcodes) and
# fixture-chained.dylib (iOS 15: LC_DYLD_CHAINED_FIXUPS), arm64.
set -euo pipefail
cd "$(dirname "$0")"
SDK="$(xcrun --sdk iphoneos --show-sdk-path)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
for target in 12.0:dyldinfo 15.0:chained; do
  min="${target%%:*}"; name="${target##*:}"
  cc=(xcrun --sdk iphoneos clang -target "arm64-apple-ios$min" -isysroot "$SDK" -O1)
  "${cc[@]}" -dynamiclib host.c -install_name @rpath/libpearhost.dylib -o "$tmp/libpearhost.dylib"
  "${cc[@]}" -dynamiclib fixture.c -install_name @rpath/fixture.dylib -L"$tmp" -lpearhost -o "fixture-$name.dylib"
done
ls -l fixture-*.dylib
