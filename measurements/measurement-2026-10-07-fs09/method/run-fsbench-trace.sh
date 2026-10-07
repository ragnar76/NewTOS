#!/usr/bin/env bash
# run-fsbench-trace.sh <scratch> <kernel8.img> <case> [append]
# FS09: FSBENCH from C:\AUTO\ on an APFS clone of <scratch>/base-fsb.img
# (prep-base.sh), headless QEMU raspi4b. Adapted from run-fsbench.sh
# (measurement-2026-10-07-fat-curcl): the serial port goes to QEMU's stdout
# (-serial stdio) and the SD trace (sdcard_normal_command / read_block /
# write_block) to its stderr, BOTH into one file -- the two writers are the
# same vCPU thread, so the trace lines sit between the FSBENCH progress lines
# of the phase that caused them (phase-counts.py splits them per phase).
# Then as before: C:\FSBENCH.LOG extracted, fsck_msdos -n C: D:, D:\FSBENCH\
# against SHA256SUMS, clone hashed and deleted. The merged log is counted
# into <scratch>/out/<case>-phases.txt and deleted (size). One QEMU at a time.
set -uo pipefail
S="$1"; K="$2"; CASE="$3"; APP="${4:-serial=on memprot=on}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMEOUT="${TIMEOUT:-1500}"
mkdir -p "$S/serial" "$S/shots" "$S/out" "$S/cards"
CARD="$S/cards/$CASE.img"; rm -f "$CARD"; cp -c "$S/base-fsb.img" "$CARD"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$CARD")
OFF2=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1D6:0x1DA])[0]*512)" "$CARD")
SD="$(mktemp -d "$S/qmp.XXXXXX")"
LOG="$S/serial/$CASE.merged"; rm -f "$LOG"
cd "$SD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial stdio -qmp unix:qmp.sock,server=on,wait=off \
  -trace sdcard_normal_command -trace sdcard_read_block -trace sdcard_write_block \
  < /dev/null > "$LOG" 2>&1 &
QPID=$!
python3 "$HERE/qmp_on_marker.py" qmp.sock "$LOG" "$S/shots" "(no key wait" 4 "$TIMEOUT" "$CASE-page"
cd "$S"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
grep -av '^sdcard_' "$LOG" > "$S/serial/$CASE.log"
python3 "$HERE/phase-counts.py" "$LOG" > "$S/out/$CASE-phases.txt"; cat "$S/out/$CASE-phases.txt"
echo "merged log $(wc -c < "$LOG" | tr -d ' ') bytes (deleted)"; rm -f "$LOG"
mcopy -o -i "$CARD@@$OFF" ::FSBENCH.LOG "$S/out/$CASE-FSBENCH.LOG" 2>/dev/null && echo "FSBENCH.LOG extracted"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
for p in s1 s2; do echo "== fsck_msdos -n ${p}"; fsck_msdos -n "${DEV}${p}" 2>&1; done > "$S/out/$CASE-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
cat "$S/out/$CASE-fsck.txt"
rm -rf "$S/out/$CASE-D"; mkdir -p "$S/out/$CASE-D"
mcopy -i "$CARD@@$OFF2" "::FSBENCH/*.DAT" "$S/out/$CASE-D/" && ( cd "$S/out/$CASE-D" && shasum -a 256 -c "$S/files/SHA256SUMS" ) > "$S/out/$CASE-Dcheck.txt" 2>&1
cat "$S/out/$CASE-Dcheck.txt"; rm -rf "$S/out/$CASE-D"
shasum -a 256 "$CARD" > "$S/out/$CASE.card-sha256"; rm -f "$CARD"
echo "run $CASE done"
