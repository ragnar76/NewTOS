#!/usr/bin/env bash
# mrun.sh <kernel8.img> <card.img> <tag> STEP...  -- headless QEMU raspi4b (APPEND as run-rpi4.sh), serial to a file, marker-keyed
# screendumps via mdrive.py (STEP syntax there). KEEP=1 keeps the card copy; TMO=<s> per-step marker timeout. One QEMU at a time.
set -uo pipefail
K="$1"; SD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"; SER="$HERE/../serial"; mkdir -p "$OUT" "$SER" /tmp/m5j/a15/s
LOG="$SER/$TAG-serial.log"
SDIR="$(mktemp -d /tmp/m5j/a15/s/q.XXXXXX)"
CARD="$(dirname "$SD")/run-$TAG.img"; cp "$SD" "$CARD"; rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" \
  -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/mdrive.py" "$SDIR/qmp.sock" "$OUT" "$TAG" "$LOG" --timeout "${TMO:-150}" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -rf "$SDIR"; [ "${KEEP:-}" = 1 ] || rm -f "$CARD"
echo "run $TAG: rc=$RC; battery: $(pmset -g batt | sed -n 1p | cut -c1-40)"
