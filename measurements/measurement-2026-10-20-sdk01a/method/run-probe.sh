#!/usr/bin/env bash
# run-probe.sh <mode...> -- the SDKP probe card on kernel8-plain, one QEMU per mode (on | strict | off), 90 s max, ends on
# "desktop up" (the probe's "SP RESULT" comes before it, from C:\AUTO); serial -> ../serial/sp-<mode>.log; the clone is
# fsck'ed (fsck_msdos -n, ../serial/sp-<mode>-fsck.txt) and listed (mtools, ../serial/sp-<mode>-card.txt) before it is
# deleted (run.sh FSCK=1 CARDLS=1). Pattern of measurement-2026-10-18-proc04-fdelete/method/run-probe.sh.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  FSCK=1 CARDLS=1 bash "$HERE/run.sh" "$S/bin/kernel8-plain.img" "$S/cards/sp.img" "sp-$m" "$A" 'desktop up' 90
  L="$HERE/../serial/sp-$m.log"
  echo "  $(grep -a 'SP RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[SP\]   *FAIL\|  FAIL ' "$L")  desktop: $(grep -ac 'desktop up' "$L")  fsck: $(grep -E 'rc=|files|clusters|ERR|Lost|lost' "$HERE/../serial/sp-$m-fsck.txt" | tr '\n' ' ')"
done
