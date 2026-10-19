#!/usr/bin/env bash
# run-probex.sh <side> <mode...> -- the F18X tester probe card (cards/f18x.img) on kernel8-<side>-plain; modes on|strict|off|verify
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?head|wt}"; shift
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; verify) A="serial=on memprot=verify";; esac
  LSDIRS="/ /G1 /G2 /G2/SUB /H2 /I1 /K0 /K1 /K2 /L1 /L2 /M1 /N1 /N2 /O1 /O2 /O2/T /O2/T/IN /AUTO" FSCK=1 CARDLS=1 bash "$HERE/regress/run.sh" "$S/bin/kernel8-$SIDE-plain.img" "$S/cards/f18x.img" "f18x-$SIDE-$m" "$A" 'desktop up' 90
  L="$HERE/../serial/f18x-$SIDE-$m.log"
  echo "  $(grep -a 'F18X RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[F18X\] FAIL' "$L")  desktop: $(grep -ac 'desktop up' "$L")  fsck: $(grep -E 'rc=|files|clusters|ERR|Lost|lost' "$HERE/../serial/f18x-$SIDE-$m-fsck.txt" | tr '\n' ' ')"
done
