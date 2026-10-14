#!/usr/bin/env bash
# seq-run.sh <workdir> — DESK43 U5a, G2 prerequisite: one QEMU raspi4b run of
# the call-sequence oracle.
#   1. scratch copy of the shared tree (make-scratch.sh), key injector applied
#      to the copy only (inj-desk-seq.py: Alt-C 1 s after "desktop up")
#   2. release rpi4 kernel with `--features svc-seq-trace`
#   3. a private copy of the MEM05 plain card (no #Z, the card of
#      acc-on-rpi4-desktop-t25.png), checked to hold no SVCSEQ.LOG before
#   4. headless QEMU raspi4b, `-append serial=on`, screendumps at 20 s / 45 s,
#      quit
#   5. C:\SVCSEQ.LOG pulled off the card (read-only mount), tools/svc-seq.py
# Kernel binary -> ../bin/ (gitignored, sha256 in RUNINFO), serial -> ../serial,
# screendumps -> ../screendumps, log + sequences -> ../trace.
set -euo pipefail
W="${1:?workdir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
WS="$(cd "$M/../.." && pwd)"
CARD_SRC="${CARD_SRC:-/tmp/m5j/acc/cards/plain.img}"
mkdir -p "$W/s"

sh "$HERE/make-scratch.sh" "$W/tree"
python3 "$HERE/inj-desk-seq.py" "$W/tree"
(cd "$W/tree" && cargo xtask build --board rpi4 --release --features svc-seq-trace) > "$W/build-seq.log" 2>&1 \
  || { tail -30 "$W/build-seq.log"; exit 1; }
grep -E "generated [0-9]+ warning|^error" "$W/build-seq.log" || true
cp "$W/tree/kernel8.img" "$M/bin/kernel8-seq.img"

mount_ro() { # mount_ro <img> -> sets DEV, MNT
  DEV=$(hdiutil attach -readonly -nomount -imagekey diskimage-class=CRawDiskImage "$1" | head -1 | awk '{print $1}')
  MNT="$W/mnt"; mkdir -p "$MNT"
  diskutil mount readOnly -mountPoint "$MNT" "${DEV}s1" > /dev/null
}
umount_ro() { diskutil unmount "$MNT" > /dev/null 2>&1 || true; hdiutil detach "$DEV" > /dev/null 2>&1 || true; }

CARD="$W/seq-card.img"
cp "$CARD_SRC" "$CARD"
mount_ro "$CARD"
if [ -e "$MNT/SVCSEQ.LOG" ]; then umount_ro; echo "card already holds SVCSEQ.LOG"; exit 1; fi
ls "$MNT" > "$M/trace/card-before.txt"
umount_ro

LOG="$M/serial/seq-rpi4-serial.log"
rm -f "$LOG" "$W/s/qmp.sock"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -kernel "$M/bin/kernel8-seq.img" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:"$W/s/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/qmp_timed.py" "$W/s/qmp.sock" "$M/screendumps" 20:seq-rpi4-t20.png 45:seq-rpi4-t45.png
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null || true; wait "$QPID" 2>/dev/null || true

mount_ro "$CARD"
ls "$MNT" > "$M/trace/card-after.txt"
cp "$MNT/SVCSEQ.LOG" "$M/trace/SVCSEQ.LOG" || { umount_ro; echo "no SVCSEQ.LOG on the card"; exit 1; }
umount_ro
python3 "$WS/tools/svc-seq.py" "$M/trace/SVCSEQ.LOG" > "$M/trace/sequence-all.txt"
python3 "$WS/tools/svc-seq.py" --caller K "$M/trace/SVCSEQ.LOG" > "$M/trace/sequence-desktop-K.txt"
python3 "$WS/tools/svc-seq.py" --caller K --no-vdi --no-caller "$M/trace/SVCSEQ.LOG" > "$M/trace/sequence-desktop-K-novdi-shape.txt"
rm -f "$CARD"
echo "seq run done: $(wc -l < "$M/trace/sequence-all.txt") calls, $(grep -c '^K ' "$M/trace/sequence-desktop-K.txt") from the EL1 desktop"
