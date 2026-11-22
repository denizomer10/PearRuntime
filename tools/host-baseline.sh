#!/bin/bash
# Game baseline for PearHost: prepares each game on the external disk, runs
# it SECS seconds, captures only its window, and prints one line per game:
#   id | alive/dead seconds | launched=0/1 rendered=yes/black/no-window missing=N | first error
#
#   tools/host-baseline.sh [SECS] [FILTER]     e.g. tools/host-baseline.sh 25 burgershop
#
# Needs the games at /Volumes/Untitled/PearSilicon (ios-apps, Games, Work).
# The rendered column depends on window timing and varies between full runs;
# rerun a single game before calling a change a regression.
set -uo pipefail
SECS="${1:-20}"
FILTER="${2:-}"
S="${PEAR_BASELINE_OUT:-${TMPDIR:-/tmp}/pear-baseline}"
mkdir -p "$S"
# Window-id helper (captures only the game window, never the whole screen).
if [[ ! -x "$S/winid" ]]; then swiftc -O "$(dirname "$0")/host-winid.swift" -o "$S/winid"; fi
RT="$(cd "$(dirname "$0")/.." && pwd)"
SRC=/Volumes/Untitled/PearSilicon/ios-apps
GAMES=/Volumes/Untitled/PearSilicon/Games
W=/Volumes/Untitled/PearSilicon/Work/host-apps
mkdir -p "$W/runs" "$S/shots"
for app in "$SRC"/*.app; do
  id="$(basename "$app" .app)"
  [[ -n "$FILTER" && "$id" != *"$FILTER"* ]] && continue
  dst="$W/$id.app"
  if ! bash -x "$RT/tools/host-prepare.sh" "$app" "$dst" > "$W/runs/$id.prepare.log" 2>&1; then
    echo "$id | PREPARE FAILED"; continue
  fi
  container="$W/container-$id"
  if [[ ! -d "$container" && -d "$GAMES/$id" ]]; then cp -cR "$GAMES/$id" "$container" 2>/dev/null || mkdir -p "$container"; fi
  mkdir -p "$container/tmp" "$container/Documents" "$container/Library/Caches"
  exe="$dst/$(plutil -extract CFBundleExecutable raw "$dst/Info.plist")"
  run="$W/runs/$id"; mkdir -p "$run"; rm -f "$run"/*
  ( cd "$container" && HOME="$container" CFFIXED_USER_HOME="$container" TMPDIR="$container/tmp/" \
    PEAR_LOG_FILE="$run/pear.log" PEAR_MISSING_REPORT="$run/missing.jsonl" \
    "$exe" > "$run/stdout.log" 2>&1 ) &
  sleep 1
  pid="$(pgrep -nf "$exe")"
  t=1; while (( t < SECS )) && kill -0 "$pid" 2>/dev/null; do sleep 1; t=$((t+1)); done
  state=dead; rendered=-
  if kill -0 "$pid" 2>/dev/null; then
    state=alive
    wid="$("$S/winid" "$pid")"
    if [[ -n "$wid" ]] && screencapture -x -o -l "$wid" "$S/shots/$id.png" 2>/dev/null; then
      sips -s format bmp -z 32 32 "$S/shots/$id.png" --out "$S/shots/$id.bmp" >/dev/null 2>&1
      rendered="$(python3 -c "
import sys;d=open('$S/shots/$id.bmp','rb').read();o=int.from_bytes(d[10:14],'little');px=d[o:]
v=[x for x in px[::1]];m=sum(v)/max(1,len(v));print('yes' if m>12 else 'black')")"
    else rendered=no-window; fi
    kill "$pid"; sleep 1; kill -9 "$pid" 2>/dev/null
  fi
  wait 2>/dev/null
  missing=0; [[ -f "$run/missing.jsonl" ]] && missing="$(grep -c '"first"' "$run/missing.jsonl")"
  launched="$(grep -c "launch complete" "$run/pear.log" 2>/dev/null)"
  err="$(grep -o -m1 "Library not loaded: [^ ]*\|Symbol not found: [^ ]*\|Terminating app due to uncaught exception '[^']*'\|panicked at [^ ]*\|unavailable GLES entry: [^ ]*" "$run/stdout.log" "$run/pear.log" 2>/dev/null | head -1 | sed 's/^[^:]*://')"
  echo "$id | $state ${t}s | launched=${launched:-0} rendered=$rendered missing=$missing | $err"
done
