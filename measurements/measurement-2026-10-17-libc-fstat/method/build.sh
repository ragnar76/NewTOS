#!/usr/bin/env bash
# (LIBC-FSTAT: copied from measurement-2026-10-16-proc06/method/build.sh, only the kv kernel kept; scratch .../tmp/lf.)
# build.sh kv -- the views kernel (rpi4 release, QEMU raspi4b only) from an rsync copy of the shared tree (make-scratch.sh) in
# /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf/tree-kv; injector instrumentation only in that copy:
#   kv      tools/inject-arm.py desktop=views.script (tag u7v)            -- the 8 views (as proc06 kv)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf
INJ="$WS/tools/inject-arm.py"
export SOURCE_DATE_EPOCH=1791979200
mkdir -p "$W" "$M/bin"
for id in "$@"; do
  T="$W/tree-$id"; LOG="$W/build-$id.log"
  bash "$HERE/make-scratch.sh" "$T"
  case $id in
    kv)     python3 "$INJ" "$T" --tag u7v desktop="$HERE/views.script";;
    *) echo "unknown kernel id $id"; exit 2;;
  esac
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$LOG" 2>&1 || { tail -30 "$LOG"; exit 1; }
  echo "$id: $(grep -E 'generated [0-9]+ warning' "$LOG" | tr '\n' ' ')"
  cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
done
