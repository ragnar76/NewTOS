#!/usr/bin/env bash
# run-fsb.sh <scratch> <kernel8.img> <base.img> <FSBENCH.NCP> <case>
# One FSBENCH run (TASK/FS08 part 4) from C:\AUTO\ on an APFS clone (cp -c)
# of <base.img> (tools/make-sd-image.sh 256 MiB with SD_FSBENCH_D: C: + D:),
# headless QEMU raspi4b, -append "serial=on memprot=on", QEMU trace of the SD
# commands (sdcard_normal_command / sdcard_read_block / sdcard_write_block) to
# <scratch>/trace/<case>.trace, screendump per page (qmp_on_marker.py, copied
# from measurement-2026-10-07-fsbench), quit. Then: C:\FSBENCH.LOG extracted,
# trace counted (trace-counts.py, copied from measurement-2026-10-07-fat-curcl),
# fsck_msdos -n on C: and D:, D:\FSBENCH\ against SHA256SUMS, the clone's
# sha256 recorded, clone + trace deleted. Adapted from
# measurement-2026-10-07-fat-curcl/method/run-fsbench.sh (that one ran the
# FSBENCH baked into its base card; this one copies the given NCP in).
# One QEMU at a time; only our own pid is killed (the host has no `timeout`).
set -uo pipefail
S="$1"; K="$2"; BASE="$3"; NCP="$4"; CASE="$5"; APP="${APPEND:-serial=on memprot=on}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMEOUT="${TIMEOUT:-1500}"
mkdir -p "$S/serial" "$S/shots" "$S/out" "$S/cards" "$S/trace"
CARD="$S/cards/$CASE.img"; rm -f "$CARD"; cp -c "$BASE" "$CARD"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$CARD")
OFF2=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1D6:0x1DA])[0]*512)" "$CARD")
mmd -i "$CARD@@$OFF" ::AUTO 2>/dev/null || true
mcopy -o -i "$CARD@@$OFF" "$NCP" ::AUTO/FSBENCH.NCP
printf 'skip FSBENCH key waits\r\n' > "$S/FSBNOKEY.FLG"
mcopy -o -i "$CARD@@$OFF" "$S/FSBNOKEY.FLG" ::FSBNOKEY.FLG
SD="$(mktemp -d "$S/qmp.XXXXXX")"
LOG="$S/serial/$CASE.log"; rm -f "$LOG"; TR="$S/trace/$CASE.trace"; rm -f "$TR"
cd "$SD"
T0=$(date +%s)
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off \
  -trace sdcard_normal_command -trace sdcard_read_block -trace sdcard_write_block -D "$TR" &
QPID=$!
python3 "$HERE/qmp_on_marker.py" qmp.sock "$LOG" "$S/shots" "(no key wait" 4 "$TIMEOUT" "$CASE-page"
cd "$S"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
echo "wall clock $(( $(date +%s) - T0 )) s"
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
mcopy -o -i "$CARD@@$OFF" ::FSBENCH.LOG "$S/out/$CASE-FSBENCH.LOG" 2>/dev/null && echo "FSBENCH.LOG extracted"
python3 "$HERE/trace-counts.py" "$TR" > "$S/out/$CASE-trace-counts.txt"; cat "$S/out/$CASE-trace-counts.txt"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
for p in s1 s2; do echo "== fsck_msdos -n ${p}"; fsck_msdos -n "${DEV}${p}" 2>&1; done > "$S/out/$CASE-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
grep -E "^==|Warning|differ|orphan|lost|Cross|corrupt|BAD" "$S/out/$CASE-fsck.txt"
rm -rf "$S/out/$CASE-D"; mkdir -p "$S/out/$CASE-D"
mcopy -i "$CARD@@$OFF2" "::FSBENCH/*.DAT" "$S/out/$CASE-D/" && ( cd "$S/out/$CASE-D" && shasum -a 256 -c "$S/files/SHA256SUMS" ) > "$S/out/$CASE-Dcheck.txt" 2>&1
grep -c ": OK" "$S/out/$CASE-Dcheck.txt" | sed 's/^/D: files OK: /'; rm -rf "$S/out/$CASE-D"
shasum -a 256 "$CARD" > "$S/out/$CASE.card-sha256"; rm -f "$CARD" "$TR"
grep -cE "\[EXC\]|PANIC|Memory violation" "$LOG" | sed 's/^/exceptions in serial: /'
echo "run $CASE done"
