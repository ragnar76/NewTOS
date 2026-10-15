#!/usr/bin/env bash
# build.sh <id>... -- T16+DESK60 scratch kernels (rpi4 release, QEMU raspi4b only), each from an rsync copy of the shared tree (make-scratch.sh)
# in /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/t16d60/tree-<id>; injector instrumentation only in that copy.
# Injector: tools/inject-arm.py of the SHARED tree (T16-corrected); for kf-red the pre-T16 version (git show HEAD:tools/inject-arm.py,
# saved as method/inject-arm-pre-t16.py).
#   kv     tools/inject-arm.py desktop=views.script (tag u7v)                                -- the 8 views (as desk58 kv)
#   kf     tools/inject-arm.py desktop=f1.script launch:HELLOGEM=ret.script (tag u6a)        -- T16 proof: F1 -> HELLOGEM under the EL0 desktop
#   kf-red the same arms with method/inject-arm-pre-t16.py                                   -- T16 red control
#   kxtd   tools/inject-arm.py launch:DESKTOP=tdf1.script (tag kxtd)                          -- DESK60: #Z TeraDesk -> QED -> quit -> desktop
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/t16d60
INJ="$WS/tools/inject-arm.py"
export SOURCE_DATE_EPOCH=1791979200
mkdir -p "$W"
for id in "$@"; do
  T="$W/tree-$id"; LOG="$W/build-$id.log"
  bash "$HERE/make-scratch.sh" "$T"
  case $id in
    kv)     python3 "$INJ" "$T" --tag u7v desktop="$HERE/views.script";;
    kf)     python3 "$INJ" "$T" --tag u6a desktop="$HERE/f1.script" launch:HELLOGEM="$HERE/ret.script";;
    kf-red) python3 "$HERE/inject-arm-pre-t16.py" "$T" --tag u6a desktop="$HERE/f1.script" launch:HELLOGEM="$HERE/ret.script";;
    kxtd)   python3 "$INJ" "$T" --tag kxtd launch:DESKTOP="$HERE/tdf1.script";;
    *) echo "unknown kernel id $id"; exit 2;;
  esac
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$LOG" 2>&1 || { tail -30 "$LOG"; exit 1; }
  echo "$id: $(grep -E 'generated [0-9]+ warning' "$LOG" | tr '\n' ' ')"
  cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
done
