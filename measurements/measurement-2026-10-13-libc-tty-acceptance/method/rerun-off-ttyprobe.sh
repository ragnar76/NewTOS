#!/usr/bin/env bash
# rerun-off-ttyprobe.sh <cards-root> -- the off-mode TTYPROBE run of tty-series.sh again
# (first pass: boot stalled before the AES, no program started; drive.py took 880 s for a
# 200 s timeout -> host stall). Same kernel, same card.
set -uo pipefail
C="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
M=off; . "$HERE/mode.sh"; export APPEND="$A4"
bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-keys-tty.img" "$C/off/ttyprobe.img" "tty-off-rpi4-ttyprobe-rerun" \
  --end 'TTYPROBE: (PASS|FAIL)' --end-delay 2 --timeout 200
