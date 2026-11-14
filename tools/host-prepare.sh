#!/bin/bash
# Prepares a no-iOSSupport copy of an installed iOS app for PearHost.
#
#   tools/host-prepare.sh <source.app|source.ipa> <dest.app>
#
# The source can be the original .ipa (an iOS-platform, decrypted binary): it is
# unpacked next to the destination (<dest>.payload/) and nothing from the Mac
# Catalyst conversion is needed. A Catalyst-prepared .app works too.
#
# The copy shares large resources with the source through symlinks; only the
# Mach-O images and Info.plist are copied. For every Mach-O image:
#   * the build version is set to macOS, so dyld resolves /System/Library
#     paths against macOS itself and never against /System/iOSSupport
#   * UIKit and other iOS-only frameworks are redirected to PearHost images
#     in <dest.app>/Frameworks (built by host/build-host.sh)
#   * the Catalyst PearRuntime dylib is redirected to an empty placeholder
#   * the image is re-signed ad hoc
set -euo pipefail

SRC="${1:?source .app}"
DST="${2:?destination .app}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST="${PEAR_BUILD_DIR:-$ROOT/build}/host"
[[ -f "$HOST/UIKit.framework/UIKit" ]] || { echo "run host/build-host.sh first" >&2; exit 1; }

if [[ "$SRC" == *.ipa ]]; then
  PAYLOAD="${DST%.app}.payload"
  if [[ ! -d "$PAYLOAD/Payload" || "$SRC" -nt "$PAYLOAD/.unpacked" ]]; then
    rm -rf "$PAYLOAD"
    mkdir -p "$PAYLOAD"
    echo ">> unpacking $(basename "$SRC")"
    ditto -x -k "$SRC" "$PAYLOAD"
    touch "$PAYLOAD/.unpacked"
  fi
  SRC="$(ls -d "$PAYLOAD"/Payload/*.app | head -1)"
fi
SRC="$(cd "$SRC" && pwd)"  # resource symlinks below must be absolute
CRYPT="$(otool -l "$SRC/$(plutil -extract CFBundleExecutable raw "$SRC/Info.plist")" | awk '/cryptid/ {print $2; exit}')"
if [[ "${CRYPT:-0}" != 0 ]]; then
  echo "the app binary is FairPlay-encrypted (cryptid $CRYPT); a decrypted IPA is required" >&2
  exit 1
fi

# Frameworks that exist only in iOSSupport (no macOS framework of the same path).
REDIRECT=(UIKit iAd)
# iOS framework paths whose API macOS ships under another framework.
SYSTEM_MAP=("/System/Library/Frameworks/MobileCoreServices.framework/MobileCoreServices=/System/Library/Frameworks/CoreServices.framework/CoreServices")

rm -rf "$DST"
mkdir -p "$DST/Frameworks"
for item in "$SRC"/*; do
  name="$(basename "$item")"
  case "$name" in
    Frameworks|Info.plist|_CodeSignature) ;;
    *) ln -s "$item" "$DST/$name" ;;
  esac
done
cp "$SRC/Info.plist" "$DST/Info.plist"
EXE="$(plutil -extract CFBundleExecutable raw "$DST/Info.plist")"
rm "$DST/$EXE"
cp "$SRC/$EXE" "$DST/$EXE"
chmod +x "$DST/$EXE"  # zip archives may drop the execute bit

# App frameworks: copy Mach-O frameworks, skip the Catalyst runtime pieces.
for item in "$SRC"/Frameworks/*; do
  name="$(basename "$item")"
  case "$name" in
    libPearRuntime.dylib|libPearOpenGLES.dylib|libswiftUIKit.dylib|PearANGLE|MarketplaceKit.framework) ;;
    *) cp -R "$item" "$DST/Frameworks/" ;;
  esac
done
for fw in "${REDIRECT[@]}"; do
  cp -R "$HOST/$fw.framework" "$DST/Frameworks/"
done
echo 'void pear_catalyst_runtime_placeholder(void) {}' > "$DST/Frameworks/.placeholder.c"
xcrun --sdk macosx clang -target arm64-apple-macos14.0 -dynamiclib "$DST/Frameworks/.placeholder.c" \
  -install_name @rpath/libPearHostPlaceholder.dylib -o "$DST/Frameworks/libPearHostPlaceholder.dylib"
rm "$DST/Frameworks/.placeholder.c"

retarget() {
  local bin="$1"
  local sdk
  sdk="$(dyld_info -platform "$bin" | awk '/macCatalyst|iOS/ {print $3; exit}')"
  vtool -set-build-version macos 14.0 "${sdk:-26.0}" -replace -output "$bin.tmp" "$bin" 2>/dev/null
  mv "$bin.tmp" "$bin"
  local deps
  deps="$(otool -L "$bin" | tail -n +2 | sed -E 's/^[[:space:]]+//; s/ \(compatibility.*$//')"
  for fw in "${REDIRECT[@]}"; do
    if grep -qx "/System/Library/Frameworks/$fw.framework/$fw" <<<"$deps"; then
      install_name_tool -change "/System/Library/Frameworks/$fw.framework/$fw" "@rpath/$fw.framework/$fw" "$bin" 2>/dev/null
    fi
  done
  for pair in "${SYSTEM_MAP[@]}"; do
    if grep -qx "${pair%%=*}" <<<"$deps"; then
      install_name_tool -change "${pair%%=*}" "${pair#*=}" "$bin" 2>/dev/null
    fi
  done
  local pear
  pear="$(grep 'libPearRuntime.dylib' <<<"$deps" || true)"
  [[ -n "$pear" ]] && install_name_tool -change "$pear" "@rpath/libPearHostPlaceholder.dylib" "$bin" 2>/dev/null
  # Drop absolute rpaths into the source bundle; keep @executable_path ones.
  while IFS= read -r rp; do
    [[ "$rp" == /* ]] && install_name_tool -delete_rpath "$rp" "$bin" 2>/dev/null || true
  done < <(otool -l "$bin" | awk '/cmd LC_RPATH/{getline; getline; sub(/^ *path /, ""); sub(/ \(offset [0-9]+\)$/, ""); print}')
  # Swift runtime: iOS-built images find it through @rpath; on macOS it lives in /usr/lib/swift.
  if grep -q '^@rpath/libswift' <<<"$deps"; then
    install_name_tool -add_rpath /usr/lib/swift "$bin" 2>/dev/null || true
  fi
  codesign -f -s - "$bin" 2>/dev/null
  echo "   retargeted $(basename "$bin") ($(dyld_info -platform "$bin" | awk '/macOS/ {print $1, $2; exit}'))"
}

retarget "$DST/$EXE"
while IFS= read -r -d '' bin; do
  if file "$bin" | grep -q 'Mach-O'; then retarget "$bin"; fi
done < <(find "$DST/Frameworks" -type f ! -name "*.plist" ! -path "*/_CodeSignature/*" -print0)
for fw in "${REDIRECT[@]}"; do codesign -f -s - "$DST/Frameworks/$fw.framework/$fw" 2>/dev/null; done
codesign -f -s - "$DST/Frameworks/libPearHostPlaceholder.dylib" 2>/dev/null
echo ">> $DST"
