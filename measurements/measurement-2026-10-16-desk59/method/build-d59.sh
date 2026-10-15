#!/usr/bin/env bash
# build-d59.sh <id>... -- DESK59 scratch kernels (rpi4 release, QEMU raspi4b only), each from an rsync copy of the shared tree
# (make-scratch.sh) in /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/d59/tree-<id>; instrumentation (corrected injector copy inject-arm-u935.py) only in that copy.
#   kt   --features selftest-desk + launch:NEWDESK=st-launch.script (tag kt)  -- desktop self-tests incl. the new [desk-act] check
#   kv   injector desktop=views.script (tag u7v)                              -- the 8 views (same tag/script as the acceptance kv)
# SUFFIX=<s> names the output bin/kernel8-<id><s>.img (e.g. -red for the build before the fix).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; W=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/d59
export SOURCE_DATE_EPOCH=1791979200
for id in "$@"; do
  T="$W/tree-$id${SUFFIX:-}"; LOG="$W/build-$id${SUFFIX:-}.log"
  bash "$HERE/make-scratch.sh" "$T"
  FEAT=()
  case $id in
    kt) python3 "$HERE/inject-arm-u935.py" "$T" --tag kt launch:NEWDESK="$HERE/st-launch.script"; FEAT=(--features selftest-desk);;
    kv) python3 "$HERE/inject-arm-u935.py" "$T" --tag u7v desktop="$HERE/views.script";;
    *) echo "unknown kernel id $id"; exit 2;;
  esac
  (cd "$T" && cargo xtask build --board rpi4 --release ${FEAT[@]+"${FEAT[@]}"}) > "$LOG" 2>&1 || { tail -30 "$LOG"; exit 1; }
  echo "$id${SUFFIX:-}: $(grep -E 'generated [0-9]+ warning' "$LOG" | tr '\n' ' ')"
  cp "$T/kernel8.img" "$M/bin/kernel8-$id${SUFFIX:-}.img"
  [ $id = kt ] && cp "$T/target/programs/rpi4/NEWDESK.NAP" "$M/bin/NEWDESK-kt${SUFFIX:-}-selftest.NAP" || true
done
