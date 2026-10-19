#!/usr/bin/env bash
# run-sp.sh <mode...> -- developer probe SDKP (sdk01a/method/sdkp.c, built against the new libc by build-probes.sh) on kernel8-plain, one QEMU per mode
# (on | strict | off), 90 s max, ends on "desktop up"; fsck + card listing of the clone. Same as sdk01a/method/run-probe.sh (paths retargeted).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  FSCK=1 CARDLS=1 bash "$HERE/run-plain.sh" "$S/bin/kernel8-plain.img" "$S/cards/sp.img" "sp-$m" "$A" 'desktop up' 90
  L="$HERE/../serial/sp-$m.log"
  echo "  $(grep -a 'SP RESULT' "$L" | tr -d '\r')  ok-lines: $(grep -ac '  ok ' "$L") FAIL lines: $(grep -ac '  FAIL ' "$L")  desktop: $(grep -ac 'desktop up' "$L")  fsck: $(tail -1 "$HERE/../serial/sp-$m-fsck.txt")"
done
