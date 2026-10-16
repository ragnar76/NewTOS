#!/usr/bin/env bash
# series.sh <label> <mode> [kernel] -- DESK43 U9-3/4/5 (copy of measurement-2026-10-14-desk43-u9-1-2/method/series.sh;
# changed: the desktop path is part of the run -- DESK=kernel appends `desk=kernel` to the boot arguments and the tag gets
# a `k` (acck-<mode>-...), default = the EL0 NEWDESK (acc-<mode>-...); modes as mode.sh incl. on-asid; cwd /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad).
# Runs, QEMU raspi4b, kernel bin/kernel8-<label>.img: desktop (plain card, dump at 25 s), APITEST (25/40 s), TeraDesk
# (35 s), QED (30 s). Cards: private copies of the MEM05 acceptance cards /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards.
set -uo pipefail
L="${1:?}"; m="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="$HERE/../bin/kernel8-$L.img"; C=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards
M="$m"; . "$HERE/mode.sh"
T="$L-$m"
if [ "${DESK:-}" = kernel ]; then export APPEND="$APPEND desk=kernel"; T="${L}k-$m"; fi
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
r4 "$K" "$C/plain.img" "$T-rpi4-desktop" 25:t25.png
r4 "$K" "$C/apitest.img" "$T-rpi4-apitest" 25:t25.png 40:t40.png
r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
r4 "$K" "$C/qed.img" "$T-rpi4-qed" 30:t30.png
