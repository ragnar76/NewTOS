#!/usr/bin/env bash
# run-probe.sh <side> <mode...> -- the P4R probe card on kernel8-<side>-plain, one QEMU per mode (on | strict | off),
# 60 s max, ends on "RD RESULT" (then "[desk] desktop up" follows); serial -> ../serial/rd-<side>-<mode>.log
# (regress/run.sh: per-run APFS clone).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?head|wt}"; shift
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; verify) A="serial=on memprot=verify";; esac
  bash "$HERE/regress/run.sh" "$S/bin/kernel8-$SIDE-plain.img" "$S/cards/rd.img" "rd-$SIDE-$m" "$A" 'desktop up' 60
  L="$HERE/../serial/rd-$SIDE-$m.log"
  echo "  $(grep -a 'RD RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[RD\] FAIL' "$L")  desktop: $(grep -ac 'desktop up' "$L")"
done
