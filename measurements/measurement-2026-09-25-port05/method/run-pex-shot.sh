#!/usr/bin/env bash
#
# run-pex-shot.sh <kernel8.img> <card.img> <serial.log> <outdir> <timeout> SPEC...
#   SPEC = MARKER:PNGNAME[:SETTLE_S]
#
# Same headless raspi4b run as run-pex.sh, plus QMP screendumps triggered on
# serial markers (method/qmp_multi.py, copied unchanged from
# measurement-2026-09-25-port03/method/). A screendump is the only way to tell
# "the launched GEM application is up and idling in evnt_multi" (no further
# serial output, which is correct) from "the machine is stuck".
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="${1:?}"; IMG="${2:?}"; LOG="${3:?}"; OUT="${4:?}"; TMO="${5:?}"; shift 5
mkdir -p "$OUT"
# The scratchpad path is long; a UNIX socket path must stay under 104 bytes.
SOCK="${TMPDIR:-/tmp}/p05-$$.sock"
rm -f "$LOG" "$SOCK"
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -kernel "$K" \
  -drive file="$IMG",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_multi.py" "$SOCK" "$LOG" "$OUT" "$TMO" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SOCK"
echo "run: rc=$RC log=$LOG"
exit $RC
