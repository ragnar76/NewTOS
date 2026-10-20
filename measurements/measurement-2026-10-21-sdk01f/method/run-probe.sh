#!/usr/bin/env bash
# run-probe.sh <v> <mode...> -- the STATT probe card st-<v> (v = new|old libc) on kernel8-plain, one QEMU per mode
# (on = memprot default, strict, off), ends on the probe's "ST RESULT" line (+1 s, then a screendump "end");
# serial -> ../serial/st-<v>-<mode>-serial.log, fsck_msdos -n + mtools listing of the clone (run.sh) before it is deleted.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; V=$1; shift
for m in "$@"; do
  case $m in on) X="";; strict) X="memprot=strict";; off) X="memprot=off";; esac
  APPEND_EXTRA="$X" LSDIRS="/ /TMP /SUB" TMO=120 bash "$HERE/run.sh" "$S/bin/kernel8-plain.img" "$S/cards/st-$V.img" "st-$V-$m" \
    'ST RESULT@1=1:end.png'
  L="$HERE/../serial/st-$V-$m-serial.log"
  echo "  $(grep -a 'ST RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[ST\] FAIL' "$L")  ok lines: $(grep -ac '\[ST\] ok' "$L")"
done
