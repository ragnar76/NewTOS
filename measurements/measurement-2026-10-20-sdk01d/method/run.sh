#!/usr/bin/env bash
# run.sh <kernel8.img> <base card.img> <tag> STEP... -- one headless QEMU raspi4b run (-append "serial=on")
# on an APFS clone (cp -c) of the base card; serial -> ../serial/<tag>-serial.log; marker-keyed QMP
# screendumps (mdrive.py STEP = 'REGEX@N=DELAY:name.png' -> ../screendumps/<tag>-<name>); then
# fsck_msdos -n of the clone (../serial/<tag>-fsck.txt) and the clone is deleted.
# (after measurement-2026-10-20-sdk01a-test/method/run.sh)
set -uo pipefail
K="$1"; BASE="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
OUT="$HERE/../screendumps"; LOG="$HERE/../serial/$TAG-serial.log"; mkdir -p "$OUT" "$HERE/../serial"
mkdir -p "$S/img"; CARD="$S/img/run-$TAG.img"; cp -c "$BASE" "$CARD"; rm -f "$LOG"
cd "$S"; SDIR="$(mktemp -d q.XXXXXX)"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" \
  -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/mdrive.py" "$SDIR/qmp.sock" "$OUT" "$TAG" "$LOG" --timeout "${TMO:-150}" --quit-delay "${QDELAY:-0}" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
fsck_msdos -n "${DEV}s1" > "$HERE/../serial/$TAG-fsck.txt" 2>&1; echo "fsck rc=$?" >> "$HERE/../serial/$TAG-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1 || true
rm -rf "$SDIR"; rm -f "$CARD"
echo "run $TAG: rc=$RC $(tail -1 "$HERE/../serial/$TAG-fsck.txt") EXC/PANIC: $(grep -Ec '\[EXC\]|PANIC|panicked' "$LOG") lines: $(grep -c . "$LOG")"
