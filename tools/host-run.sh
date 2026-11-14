#!/bin/bash
# Runs a PearHost-prepared app with its own container and collects a run folder:
#   tools/host-run.sh <prepared.app> <run-dir> [seconds]
# run-dir/stdout.log, pear.log, missing.jsonl (one line per first missing call,
# summary lines at exit), exit.txt. The process is stopped after [seconds] (default 45).
set -uo pipefail
APP="$(cd "${1:?prepared .app}" && pwd)"
RUN="${2:?run dir}"
SECS="${3:-45}"
mkdir -p "$RUN"
RUN="$(cd "$RUN" && pwd)"
CONTAINER="$(dirname "$APP")/container-$(basename "$APP" .app)"
# PEAR_HOST_SEED=<container> clones an existing app container (APFS clone: no
# extra space, the original is never written) the first time.
if [[ -n "${PEAR_HOST_SEED:-}" && ! -d "$CONTAINER" ]]; then
  cp -cR "$PEAR_HOST_SEED" "$CONTAINER"
fi
mkdir -p "$CONTAINER/Documents" "$CONTAINER/Library/Caches" "$CONTAINER/Library/Preferences" "$CONTAINER/tmp"
EXE="$(plutil -extract CFBundleExecutable raw "$APP/Info.plist")"
rm -f "$RUN"/*.log "$RUN"/missing.jsonl "$RUN"/exit.txt
CFFIXED_USER_HOME="$CONTAINER" HOME="$CONTAINER" TMPDIR="$CONTAINER/tmp/" \
PEAR_MISSING_REPORT="$RUN/missing.jsonl" PEAR_LOG_FILE="$RUN/pear.log" LLVM_PROFILE_FILE="$RUN/default.profraw" \
  "$APP/$EXE" > "$RUN/stdout.log" 2>&1 &
PID=$!
for ((i = 0; i < SECS; i++)); do
  kill -0 $PID 2>/dev/null || break
  sleep 1
done
if kill -0 $PID 2>/dev/null; then
  echo "still running after ${SECS}s; sending SIGTERM" > "$RUN/exit.txt"
  kill -TERM $PID; sleep 3; kill -KILL $PID 2>/dev/null
else
  wait $PID; echo "exited with status $? after ${i}s" > "$RUN/exit.txt"
fi
cat "$RUN/exit.txt"
