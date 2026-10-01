#!/usr/bin/env bash
# run.sh MACHINE KERNEL IMG OUTDIR TAG TIMEOUT_S SPEC...   (SPEC = MARKER:PNG[:SETTLE])
#   MACHINE = raspi4b  -> -M raspi4b, KERNEL = kernel8.img, IMG = SD image
#   MACHINE = virt     -> the `cargo xtask run` (qemu-virt) machine line:
#                         virt,gic-version=2 / cortex-a72 / 272M / ramfb /
#                         qemu-xhci + usb-kbd + usb-mouse, KERNEL = the flat
#                         kernel.img xtask's objcopy produces; IMG ignored ("-").
# Serial to a file, QMP screendumps via qmp_multi.py, which ends the VM with
# QMP `quit`. No `timeout` on macOS: qmp_multi.py bounds the run.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MACH="$1"; KERNEL="$2"; IMG="$3"; OUTDIR="$4"; TAG="$5"; TMO="$6"; shift 6
mkdir -p "$OUTDIR"
LOG="$OUTDIR/$TAG-serial.log"
SOCK="/private/tmp/claude-501/x03-$TAG-$$.sock"   # unix socket path must stay < 104 bytes
rm -f "$LOG" "$SOCK"
case "$MACH" in
  raspi4b)
    qemu-system-aarch64 -M raspi4b -no-reboot -display none \
      -serial file:"$LOG" -kernel "$KERNEL" \
      -drive file="$IMG",if=sd,format=raw \
      -qmp unix:"$SOCK",server,nowait & ;;
  virt)
    qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M \
      -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
      -display none -serial file:"$LOG" -no-reboot -kernel "$KERNEL" \
      -qmp unix:"$SOCK",server,nowait & ;;
  *) echo "unknown machine $MACH"; exit 2 ;;
esac
QPID=$!
python3 "$HERE/qmp_multi.py" "$SOCK" "$LOG" "$OUTDIR" "$TMO" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SOCK"
echo "run $TAG: rc=$RC log=$LOG"
exit $RC
