#!/usr/bin/env bash
# run-probe.sh <side> <mode...> -- the P4F probe card on kernel8-<side>-plain, one QEMU per mode (on | strict | off),
# 60 s max, ends on "PF RESULT"; serial -> ../serial/pf-<side>-<mode>.log (regress/run.sh: per-run APFS clone).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?head|wt}"; shift
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$S/bin/kernel8-$SIDE-plain.img" "$S/cards/pf.img" "pf-$SIDE-$m" "$A" 'PF RESULT' 60
  L="$HERE/../serial/pf-$SIDE-$m.log"
  echo "  $(grep -a 'PF RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[PF\] FAIL' "$L")"
done
