#!/usr/bin/env bash
# run-rpi4.sh <kernel8.img> <card.img> <tag> <SECONDS:PNG>...
# Headless QEMU raspi4b, `-append "${APPEND:-serial=on}"` (SER01), the card as
# SD (used in place; the caller's private per-run copy), serial to
# serial/<tag>-serial.log, QMP screendumps at fixed offsets into screendumps/,
# then `quit` (qmp_timed.py; the host has no `timeout`). SYSINFO02 change
# against the SYSINFO01 copy: QMP socket in <card dir>/q.<tag> under a
# relative name, no second card copy.
set -uo pipefail
K="$1"; CARD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"; SER="$HERE/../serial"; mkdir -p "$OUT" "$SER"
LOG="$SER/$TAG-serial.log"
SDIR="$(dirname "$CARD")/q.$TAG"; rm -rf "$SDIR"; mkdir -p "$SDIR"
cd "$SDIR" || exit 1
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
specs=(); for s in "$@"; do specs+=("${s%%:*}:$TAG-${s#*:}"); done
python3 "$HERE/qmp_timed.py" qmp.sock "$OUT" "${specs[@]}"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd / && rm -rf "$SDIR"
echo "run $TAG: rc=$RC"
