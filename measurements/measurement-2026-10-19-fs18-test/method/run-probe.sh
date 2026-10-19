#!/usr/bin/env bash
# run-probe.sh <side> <mode...> -- the F18 probe card on kernel8-<side>-plain, one QEMU per mode (on | strict | off),
# 90 s max, ends on "desktop up" (the probe's "F18 RESULT" comes before it); serial -> ../serial/f18-<side>-<mode>.log; the
# clone is fsck'ed (fsck_msdos -n, ../serial/f18-<side>-<mode>-fsck.txt) and listed (mtools) before it is deleted.
# (Adapted from measurement-2026-10-19-proc04-fdelete-test/method/run-probe.sh.)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?head|wt}"; shift
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; verify) A="serial=on memprot=verify";; esac
  LSDIRS="/ /R18A /R18O /R18B /R18C /R14 /R18D /R18E /AUTO" FSCK=1 CARDLS=1 bash "$HERE/regress/run.sh" "$S/bin/kernel8-$SIDE-plain.img" "$S/cards/f18.img" "f18-$SIDE-$m" "$A" 'desktop up' 90
  L="$HERE/../serial/f18-$SIDE-$m.log"
  echo "  $(grep -a 'F18 RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[F18\] FAIL' "$L")  desktop: $(grep -ac 'desktop up' "$L")  fsck: $(grep -E 'rc=|files|clusters|ERR|Lost|lost' "$HERE/../serial/f18-$SIDE-$m-fsck.txt" | tr '\n' ' ')"
done
