#!/usr/bin/env bash
# run-probe.sh <mode...> -- the LDP card on kernel8-plain, one QEMU per mode (on | strict | off), 90 s max, ends on "desktop up"
# (LDP RESULT comes before it, from C:\AUTO); serial -> ../serial/ldp-<mode>.log; the clone is fsck'ed (../serial/ldp-<mode>-fsck.txt)
# before it is deleted (run-probe-qemu.sh FSCK=1). Pattern of measurement-2026-10-21-sdk01e/method/run-probe.sh.
# Kernels: the release rpi4 kernels of measurement-2026-10-21-sdk01f (git archive 33ef861a; no kernel/tos-api/tos-types/tools change
# between 33ef861a and HEAD 04f81551, `git diff --stat` empty), cloned to $S/bin, sha256 in ../checks/kernels.sha256.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  FSCK=1 bash "$HERE/run-probe-qemu.sh" "$S/bin/kernel8-plain.img" "$S/cards/ldp.img" "ldp-$m" "$A" 'desktop up' 90
  L="$HERE/../serial/ldp-$m.log"
  echo "  $(grep -a 'LDP RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[LDP\] FAIL' "$L")  ok lines: $(grep -ac '\[LDP\] ok' "$L")  desktop: $(grep -ac 'desktop up' "$L")  fsck: $(grep -E 'rc=' "$HERE/../serial/ldp-$m-fsck.txt" | tr '\n' ' ')"
done
