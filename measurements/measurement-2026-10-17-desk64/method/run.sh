#!/usr/bin/env bash
# run.sh <kernel-id> <tag> STEP... -- one headless QEMU raspi4b run (-append serial=on) on an APFS clone (cp -c) of the
# base card, serial to ../serial/<tag>-serial.log, marker-keyed QMP screendumps via mdrive.py (STEP = 'REGEX@N=DELAY:name.png').
# The clone's NEWDESK.INF is copied to ../serial/<tag>-NEWDESK-after.INF, then the clone is deleted. One QEMU at a time.
set -uo pipefail
id="${1:?}"; TAG="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/d64
K="$HERE/../bin/kernel8-$id.img"; OUT="$HERE/../screendumps"; LOG="$HERE/../serial/$TAG-serial.log"
CARD="$W/run-$TAG.img"; cp -c "$W/${BASE:-base}.img" "$CARD"   # BASE=<name>: another base card (unused here); rm -f "$LOG"
cd "$W"; SDIR="$(mktemp -d q.XXXXXX)"   # relative: the QMP socket path must stay < 104 bytes
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" \
  -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/mdrive.py" "$SDIR/qmp.sock" "$OUT" "$TAG" "$LOG" --timeout "${TMO:-150}" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
# DESK64: the card's NEWDESK.INF after the run (the immediate save), read with mtools at the MBR partition offset.
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[454:458])[0]*512)" "$CARD")
mtype -i "$CARD@@$OFF" ::NEWDESK.INF > "$HERE/../serial/$TAG-NEWDESK-after.INF" 2>&1
rm -rf "$SDIR"; rm -f "$CARD"
echo "run $TAG: rc=$RC"
