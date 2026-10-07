#!/usr/bin/env bash
# run-boot.sh <scratch> <kernel> <case> <desk|api> [append]
# One headless QEMU raspi4b boot on an APFS clone of <scratch>/base.img:
#   desk = plain desktop, one screendump 3 s after "[desk] desktop up";
#   api  = #Z C:\APITEST.NCP written into the clone (mtools), screendump
#          after "TOTAL".
# Serial log -> <scratch>/serial/<case>.log, dump -> <scratch>/shots/,
# fsck_msdos -n on the clone, its sha256 recorded, the clone deleted.
# One QEMU at a time; only our own pid is killed. QMP socket relative to a
# private dir (macOS 104-byte UNIX socket limit).
set -uo pipefail
S="$1"; K="$2"; CASE="$3"; MODE="$4"; APP="${5:-serial=on memprot=on}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$S/serial" "$S/shots" "$S/out" "$S/cards"
CARD="$S/cards/$CASE.img"; rm -f "$CARD"; cp -c "$S/base.img" "$CARD"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$CARD")
if [ "$MODE" = api ]; then
  sed 's|^#Z .*|#Z 01 C:\\APITEST.NCP@|' "$HERE/newdesk-template.inf" > "$S/out/$CASE-NEWDESK.INF"
  mcopy -o -i "$CARD@@$OFF" "$S/out/$CASE-NEWDESK.INF" ::NEWDESK.INF
  MARK="TOTAL"; export MARK_DELAY=1
else
  MARK="[desk] desktop up"; export MARK_DELAY=3
fi
SD="$(mktemp -d "$S/q.XXXXXX")"
LOG="$S/serial/$CASE.log"; rm -f "$LOG"
cd "$SD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
python3 "$HERE/qmp_on_marker.py" qmp.sock "$LOG" "$S/shots" "$MARK" 1 "${TIMEOUT:-240}" "$CASE"
cd "$S"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
{ echo "== fsck_msdos -n s1"; fsck_msdos -n "${DEV}s1" 2>&1; } > "$S/out/$CASE-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
shasum -a 256 "$CARD" > "$S/out/$CASE.card-sha256"; rm -f "$CARD"
echo "run $CASE done"
