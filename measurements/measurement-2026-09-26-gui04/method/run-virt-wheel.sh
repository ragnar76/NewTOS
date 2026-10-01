#!/usr/bin/env bash
#
# run-virt-wheel.sh <kernel.img> <serial.log> <up> <down>
#
# Run D: QEMU `-M virt` with a REAL emulated USB mouse (`-device qemu-xhci
# -device usb-mouse`, the same line tools/xtask uses for `--board qemu-virt`),
# and turn its wheel through QMP. This is the only run in which the report
# really comes out of an xHCI interrupt transfer instead of being synthesised.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="${1:?}"; LOG="${2:?}"; UP="${3:-3}"; DN="${4:-4}"
SOCK="/private/tmp/claude-501/gui04-virt-$$.sock"
rm -f "$LOG" "$SOCK"
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M \
  -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
  -display none -serial file:"$LOG" -no-reboot -kernel "$K" \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_wheel.py" "$SOCK" "$LOG" "[desk] desktop up" 90 "$UP" "$DN"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SOCK"
echo "run virt-wheel: rc=$RC log=$LOG"
exit $RC
