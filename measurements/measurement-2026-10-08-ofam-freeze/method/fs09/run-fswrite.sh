#!/usr/bin/env bash
# run-fswrite.sh <scratch> <kernel8.img> <case> [append]
# FSWRITE.NCP (build-fswrite.sh -> <scratch>/bin/FSWRITE.NCP) from C:\AUTO\
# on an APFS clone of <scratch>/base.img (prep-base.sh: C: NEWTOS + D:
# NEWTOS1 with FSBENCH files), headless QEMU raspi4b until "FSWRITE: PASS|
# FAIL" (or <TIMEOUT> s), then: C:\FSWRITE.LOG extracted, fswrite-check.py
# (byte model on the host), fsck_msdos -n on C: and D:, the clone's sha256
# recorded, the clone deleted. One QEMU at a time; only our own pid is killed.
set -uo pipefail
S="$1"; K="$2"; CASE="$3"; APP="${4:-serial=on memprot=on}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMEOUT="${TIMEOUT:-600}"
mkdir -p "$S/serial" "$S/out" "$S/cards"
CARD="$S/cards/$CASE.img"; rm -f "$CARD"; cp -c "$S/base.img" "$CARD"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$CARD")
mmd -i "$CARD@@$OFF" ::AUTO 2>/dev/null || true
mcopy -o -i "$CARD@@$OFF" "$S/bin/FSWRITE.NCP" ::AUTO/FSWRITE.NCP
LOG="$S/serial/$CASE.log"; rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" ${QEMU_EXTRA:-} &
QPID=$!
t=0; while [ $t -lt "$TIMEOUT" ]; do
  grep -qE 'FSWRITE: (PASS|FAIL)' "$LOG" 2>/dev/null && break
  kill -0 "$QPID" 2>/dev/null || break
  sleep 2; t=$((t+2))
done
sleep 3; kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
grep -E 'FSWRITE: |ERROR' "$LOG" | head -20
mcopy -o -i "$CARD@@$OFF" ::FSWRITE.LOG "$S/out/$CASE-FSWRITE.LOG" 2>/dev/null && echo "FSWRITE.LOG extracted"
python3 "$HERE/fswrite-check.py" "$CARD" > "$S/out/$CASE-hostcheck.txt" 2>&1; tail -1 "$S/out/$CASE-hostcheck.txt"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
for p in s1 s2; do echo "== fsck_msdos -n ${p}"; fsck_msdos -n "${DEV}${p}" 2>&1; done > "$S/out/$CASE-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
grep -E "^==|files|Free|orphan|lost|Cross|differ|corrupt" "$S/out/$CASE-fsck.txt"
shasum -a 256 "$CARD" > "$S/out/$CASE.card-sha256"; rm -f "$CARD"
echo "run $CASE done"
