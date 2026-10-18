#!/usr/bin/env bash
# run-probe.sh <side> <mode...> -- the probe card on kernel8-<side>-inj (EmuCON keys armed on launch:P4Z2), one QEMU
# per mode (on | strict | off), 80 s each, serial -> ../serial/p4-<side>-<mode>.log (regress/run.sh: per-run APFS clone).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/../env.sh"; SIDE="${1:?head|wt}"; shift
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/../regress/run.sh" "$S/bin/kernel8-$SIDE-inj.img" "$S/cards/p4.img" "p4-$SIDE-$m" "$A" 'NEVER-ENDS' 80
  L="$HERE/../../serial/p4-$SIDE-$m.log"
  echo "  $(grep -a 'RESULT' "$L" | tr -d '\r' | tr '\n' ';')"
done
