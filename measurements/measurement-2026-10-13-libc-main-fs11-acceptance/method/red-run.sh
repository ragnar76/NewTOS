#!/usr/bin/env bash
# red-run.sh <cards-dir> -- the PPARENT probe card on the kernel BEFORE this
# change (measurement-2026-10-06-vforkt-fix/bin/kernel8-vf.img, read only),
# memprot=on: AUTO/#Z programs get p_parent 0 and the AUTO one p_env 0, so the
# probes must FAIL -- and, with the new crt0 (mintlib's p_parent test), the
# programs are taken for accessories. Shows that the probe sees the change
# and that new SDK binaries need a PPARENT kernel.
set -uo pipefail
C="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
K="$HERE/../../measurement-2026-10-06-vforkt-fix/bin/kernel8-vf.img"
shasum -a 256 "$K"
mkdir -p "$C"
[ -f "$C/pparent.img" ] || SEED_ACC=1 EXTRA="AUTO/PPAUTO.NCP=$B/PPAUTO.NCP ACC/PPACC.NAC=$B/PPACC.NAC" \
  bash "$HERE/seed-card.sh" "$C/pparent.img" 'C:\PPZ.NAP' > /dev/null
APPEND="serial=on" bash "$HERE/run-rpi4-drive.sh" "$K" "$C/pparent.img" "red-vf-on-rpi4-pparent" \
  --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 180
