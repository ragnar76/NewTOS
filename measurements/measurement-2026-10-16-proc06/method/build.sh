#!/usr/bin/env bash
# build.sh <id>... -- PROC06 scratch kernels (rpi4 release, QEMU raspi4b only), each from an rsync copy of the shared tree (make-scratch.sh)
# in /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06/tree-<id>; injector instrumentation / red revert only in that copy.
#   kv      tools/inject-arm.py desktop=views.script (tag u7v)            -- the 8 views (as t16-desk60 kv), with the PROC06 fix
#   kc      tools/inject-arm.py launch:CTRLCPAR=ctrlc.script (tag p06c)   -- Ctrl-C child case, with the PROC06 fix
#   kc-red  the same, and in the copy the fix line of finish_terminate put back to the pre-PROC06 `rc as i64 as u64` -- red control
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06
INJ="$WS/tools/inject-arm.py"
export SOURCE_DATE_EPOCH=1791979200
mkdir -p "$W" "$M/bin"
FIX='            frame.gpr[0] = tos_api::bdos::process::pterm::exit_code_for_caller(rc);'
OLD='            frame.gpr[0] = rc as i64 as u64;'
for id in "$@"; do
  T="$W/tree-$id"; LOG="$W/build-$id.log"
  bash "$HERE/make-scratch.sh" "$T"
  case $id in
    kv)     python3 "$INJ" "$T" --tag u7v desktop="$HERE/views.script";;
    kc)     python3 "$INJ" "$T" --tag p06c launch:CTRLCPAR="$HERE/ctrlc.script";;
    kc-red) python3 "$INJ" "$T" --tag p06c launch:CTRLCPAR="$HERE/ctrlc.script"
            python3 -I -c 'import sys; p,a,b=sys.argv[1:]; s=open(p).read(); assert s.count(a)==1; open(p,"w").write(s.replace(a,b))' \
              "$T/kernel/src/arch/aarch64/exceptions.rs" "$FIX" "$OLD"
            echo "kc-red: fix reverted: $(grep -c 'frame.gpr\[0\] = rc as i64 as u64;' "$T/kernel/src/arch/aarch64/exceptions.rs") line";;
    *) echo "unknown kernel id $id"; exit 2;;
  esac
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$LOG" 2>&1 || { tail -30 "$LOG"; exit 1; }
  echo "$id: $(grep -E 'generated [0-9]+ warning' "$LOG" | tr '\n' ' ')"
  cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
done
