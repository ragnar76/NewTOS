#!/usr/bin/env bash
# DESK43 U8a copy of measurement-2026-10-14-desk43-u7/method/seq-run.sh (unchanged below this header; the injector it applies is
# this run's inj-desk-seq.py, armed at the kernel's desktop-up line). Source tree: rsync of the shared tree.
# seq-run.sh <workdir> <source-tree> <label> -- DESK43 U7 (copied from
# measurement-2026-10-14-desk43-u5b/method/seq-run.sh; only this header changed),
# guard G2 as a before/after oracle: one QEMU raspi4b run of the call-sequence
# tracer on a kernel built from <source-tree> (/tmp/m5j/u7/headtree = `git archive`
# of HEAD 1775339d, or /tmp/m5j/u7/newtree = the same plus the U7 change).
# Steps:
#   1. rsync <source-tree> -> <workdir>/tree-<label>, key injector applied to
#      that copy only (inj-desk-seq.py: Alt-C 1 s after "desktop up")
#   2. release rpi4 kernel with `--features svc-seq-trace`
#   3. a private copy of the MEM05 plain card, checked to hold no SVCSEQ.LOG
#   4. headless QEMU raspi4b, `-append serial=on`, screendumps 20 s / 45 s, quit
#   5. C:\SVCSEQ.LOG pulled off the card (read-only mount), tools/svc-seq.py
set -euo pipefail
W="${1:?workdir}"; SRC="${2:?source tree}"; L="${3:?label}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
WS="$(cd "$M/../.." && pwd)"
CARD_SRC="${CARD_SRC:-/tmp/m5j/u5b/cards/plain.img}"
TR="$M/trace"; mkdir -p "$W/s" "$TR"
export SOURCE_DATE_EPOCH=1791979200

rsync -a --delete --exclude target "$SRC/" "$W/tree-$L/"
python3 "$HERE/inj-desk-seq.py" "$W/tree-$L"
(cd "$W/tree-$L" && cargo xtask build --board rpi4 --release --features svc-seq-trace) > "$W/build-seq-$L.log" 2>&1 \
  || { tail -30 "$W/build-seq-$L.log"; exit 1; }
grep -E "generated [0-9]+ warning|^error" "$W/build-seq-$L.log" || true
cp "$W/tree-$L/kernel8.img" "$M/bin/kernel8-seq-$L.img"

mount_ro() { # mount_ro <img> -> sets DEV, MNT
  DEV=$(hdiutil attach -readonly -nomount -imagekey diskimage-class=CRawDiskImage "$1" | head -1 | awk '{print $1}')
  MNT="$W/mnt"; mkdir -p "$MNT"
  diskutil mount readOnly -mountPoint "$MNT" "${DEV}s1" > /dev/null
}
umount_ro() { diskutil unmount "$MNT" > /dev/null 2>&1 || true; hdiutil detach "$DEV" > /dev/null 2>&1 || true; }

CARD="$W/seq-card-$L.img"
cp "$CARD_SRC" "$CARD"
mount_ro "$CARD"
if [ -e "$MNT/SVCSEQ.LOG" ]; then umount_ro; echo "card already holds SVCSEQ.LOG"; exit 1; fi
umount_ro

LOG="$M/serial/seq-$L-rpi4-serial.log"
rm -f "$LOG" "$W/s/qmp.sock"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -kernel "$M/bin/kernel8-seq-$L.img" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:"$W/s/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/qmp_timed.py" "$W/s/qmp.sock" "$M/screendumps" 20:seq-$L-rpi4-t20.png 45:seq-$L-rpi4-t45.png
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null || true; wait "$QPID" 2>/dev/null || true

mount_ro "$CARD"
cp "$MNT/SVCSEQ.LOG" "$TR/SVCSEQ-$L.LOG" || { umount_ro; echo "no SVCSEQ.LOG on the card"; exit 1; }
umount_ro
python3 "$WS/tools/svc-seq.py" "$TR/SVCSEQ-$L.LOG" > "$TR/sequence-all-$L.txt"
python3 "$WS/tools/svc-seq.py" --caller K --no-results "$TR/SVCSEQ-$L.LOG" > "$TR/sequence-desktop-K-noresults-$L.txt"
python3 "$WS/tools/svc-seq.py" --caller K --no-vdi --no-caller "$TR/SVCSEQ-$L.LOG" > "$TR/sequence-desktop-K-novdi-shape-$L.txt"
rm -f "$CARD"
echo "seq run $L done: $(wc -l < "$TR/sequence-all-$L.txt") calls"
