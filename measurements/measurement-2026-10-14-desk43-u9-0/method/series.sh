#!/usr/bin/env bash
# (DESK43 U9-0 copy of measurements/measurement-2026-10-14-desk43-u8b/method/series.sh; only this header changed;
#  label u90 = this change, reference = the U8b run; cwd /tmp/m5j/u90)
# series.sh <label> <mode> -- behaviour runs, QEMU raspi4b, one kernel (bin/kernel8-<label>.img: u8b = this change;
# the reference is the U6b run)
# in one protection mode (mode.sh: on | off | strict; always serial=on):
# desktop (plain card, dump at 25 s), APITEST (25/40 s), TeraDesk (35 s),
# QED (30 s). Cards: private copies of the MEM05 acceptance cards
# (measurement-2026-10-13-mem05-acceptance, seed-card.sh), sha256 in RUNINFO.
# cwd must hold s/ (short QMP socket dir): run from /tmp/m5j/u8b.
set -uo pipefail
L="${1:?}"; m="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="$HERE/../bin/kernel8-$L.img"; C=/tmp/m5j/u5b/cards
M="$m"; . "$HERE/mode.sh"
T="$L-$m"
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
r4 "$K" "$C/plain.img" "$T-rpi4-desktop" 25:t25.png
r4 "$K" "$C/apitest.img" "$T-rpi4-apitest" 25:t25.png 40:t40.png
r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
r4 "$K" "$C/qed.img" "$T-rpi4-qed" 30:t30.png
