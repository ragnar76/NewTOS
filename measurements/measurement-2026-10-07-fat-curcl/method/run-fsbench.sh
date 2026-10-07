#!/usr/bin/env bash
# run-fsbench.sh <scratch> <kernel8.img> <case> [append]
# FSBENCH from C:\AUTO\ on an APFS clone of <scratch>/base-fsb.img
# (prep-base.sh), headless QEMU raspi4b, screendump per page
# (qmp_on_marker.py, copied from measurement-2026-10-07-fsbench), then:
# C:\FSBENCH.LOG extracted, fsck_msdos -n on C: and D:, D:\FSBENCH\ checked
# against SHA256SUMS, run clone's sha256 recorded, clone deleted.
# Adapted from measurement-2026-10-07-fsbench/method/run-fsbench-qemu.sh.
# QEMU_EXTRA (env, optional): extra QEMU arguments (trace events, see
# boot-smoke.sh).
set -uo pipefail
S="$1"; K="$2"; CASE="$3"; APP="${4:-serial=on memprot=on}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMEOUT="${TIMEOUT:-1500}"
mkdir -p "$S/serial" "$S/shots" "$S/out" "$S/cards"
CARD="$S/cards/$CASE.img"; rm -f "$CARD"; cp -c "$S/base-fsb.img" "$CARD"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$CARD")
OFF2=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1D6:0x1DA])[0]*512)" "$CARD")
SD="$(mktemp -d "$S/qmp.XXXXXX")"
LOG="$S/serial/$CASE.log"; rm -f "$LOG"
# The QMP socket is addressed relative to $SD (cwd): the absolute scratch
# path exceeds the 104-byte limit of a UNIX socket path on macOS.
cd "$SD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off ${QEMU_EXTRA:-} &
QPID=$!
python3 "$HERE/qmp_on_marker.py" qmp.sock "$LOG" "$S/shots" "(no key wait" 4 "$TIMEOUT" "$CASE-page"
cd "$S"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
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
