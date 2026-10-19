#!/usr/bin/env bash
# series-api.sh -- APITEST (#Z autorun, built against the SDK01 A libc.a by build-probes.sh) on kernel8-plain, modes
# on / strict / off, one QEMU at a time, 240 s max, ends on "TOTAL "; serial -> ../serial/api-<mode>.log.
# (APITEST part of measurement-2026-10-18-proc04-fdelete/method/regress/series.sh.)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/run.sh" "$S/bin/kernel8-plain.img" "$S/cards/api.img" "api-$m" "$A" 'TOTAL ' 240
  echo "  $m: $(grep -a 'TOTAL ' "$HERE/../serial/api-$m.log" | tail -1 | tr -d '\r')"
done
