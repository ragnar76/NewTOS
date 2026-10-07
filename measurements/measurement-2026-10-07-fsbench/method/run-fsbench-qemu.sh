#!/usr/bin/env bash
# run-fsbench-qemu.sh <scratch> [case] -- one headless QEMU raspi4b run of
# FSBENCH.NCP from C:\AUTO\ on a scratch two-partition card (TASK/FS08).
#
#   1. gen-fsbench-files.py           -> <scratch>/files (10 files, manifest)
#   2. build-fsbench.sh               -> <scratch>/bin/FSBENCH.NCP
#   3. board-rpi4 release kernel of the UNCHANGED tree, own target dir
#                                     -> <scratch>/bin/kernel8.img
#   4. tools/make-sd-image.sh 256 MiB with SD_FSBENCH_D (C: + D:\FSBENCH\)
#      + mtools: C:\AUTO\FSBENCH.NCP, C:\FSBNOKEY.FLG (3 s pause per page
#      instead of the key wait)
#   5. qemu-system-aarch64 -M raspi4b -append "serial=on memprot=on",
#      screendump per page (qmp_on_marker.py), quit; extract C:\FSBENCH.LOG.
# One QEMU at a time; bounded by the QMP helper's timeout + kill -9 of OUR pid.
set -uo pipefail
S="${1:?scratch dir}"; CASE="${2:-run1}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
TIMEOUT="${TIMEOUT:-1500}"
mkdir -p "$S/bin" "$S/serial" "$S/shots" "$S/out" "$S/cards"

if [ ! -f "$S/files/MANIFEST.TXT" ]; then
  python3 "$WS/sdk/examples/fsbench/gen-fsbench-files.py" "$S/files" --header "$S/files/fsbench-manifest.h" >/dev/null
  cmp "$S/files/fsbench-manifest.h" "$WS/sdk/examples/fsbench/fsbench-manifest.h" \
    || { echo "manifest header differs from the tracked one" >&2; exit 1; }
fi
"$WS/sdk/examples/fsbench/build-fsbench.sh" "$S/bin/FSBENCH.NCP" || exit 1
if [ ! -f "$S/bin/kernel8.img" ]; then
  ( cd "$WS" && CARGO_TARGET_DIR="$S/target" cargo build --package kernel --no-default-features \
      --features board-rpi4 --release ) > "$S/kbuild.log" 2>&1 || { tail -20 "$S/kbuild.log"; exit 1; }
  OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
  "$OC" -O binary "$S/target/aarch64-unknown-none-softfloat/release/kernel" "$S/bin/kernel8.img"
fi
if [ "${FSB_NOD:-0}" = "1" ]; then
  # negative case: the default single-partition card, no D: at all
  BASE="$S/base-nod.img"
  [ -f "$BASE" ] || "$WS/tools/make-sd-image.sh" "$BASE" 256 > "$S/mkimg-nod.log" 2>&1 \
    || { tail -20 "$S/mkimg-nod.log"; exit 1; }
else
  BASE="$S/base.img"
  [ -f "$BASE" ] || SD_FSBENCH_D="$S/files" "$WS/tools/make-sd-image.sh" "$BASE" 256 > "$S/mkimg.log" 2>&1 \
    || { tail -20 "$S/mkimg.log"; exit 1; }
fi
CARD="$S/cards/$CASE.img"; cp "$BASE" "$CARD"
# negative case: FSB_DEL="name ..." removes these files from D:\FSBENCH\ on this card copy
if [ -n "${FSB_DEL:-}" ]; then
  OFF2=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1D6:0x1DA])[0]*512)" "$CARD")
  for n in $FSB_DEL; do mdel -i "$CARD@@$OFF2" "::FSBENCH/$n"; done
fi
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$CARD")
mmd -i "$CARD@@$OFF" ::AUTO 2>/dev/null || true
mcopy -o -i "$CARD@@$OFF" "$S/bin/FSBENCH.NCP" ::AUTO/FSBENCH.NCP
printf 'skip FSBENCH key waits\r\n' > "$S/FSBNOKEY.FLG"
mcopy -o -i "$CARD@@$OFF" "$S/FSBNOKEY.FLG" ::FSBNOKEY.FLG

SD="$(mktemp -d /private/tmp/claude-501/fsb.XXXXXX)"
LOG="$S/serial/$CASE.log"; rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on memprot=on" \
  -kernel "$S/bin/kernel8.img" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:"$SD/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/qmp_on_marker.py" "$SD/qmp.sock" "$LOG" "$S/shots" "(no key wait" 4 "$TIMEOUT" "$CASE-page"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
mcopy -o -i "$CARD@@$OFF" ::FSBENCH.LOG "$S/out/$CASE-FSBENCH.LOG" 2>/dev/null && echo "FSBENCH.LOG extracted"
# read-only fsck of both partitions (the bench must not have changed D:)
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" | head -1 | awk '{print $1}')
PARTS="s1 s2"; [ "${FSB_NOD:-0}" = "1" ] && PARTS="s1"
for p in $PARTS; do echo "== fsck_msdos -n ${p}"; fsck_msdos -n "${DEV}${p}" 2>&1; done > "$S/out/$CASE-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
cat "$S/out/$CASE-fsck.txt"
[ "${FSB_NOD:-0}" = "1" ] && { echo "run $CASE done"; exit 0; }
# D:\FSBENCH\ unchanged after the run: copy back, check against SHA256SUMS
OFF2=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1D6:0x1DA])[0]*512)" "$CARD")
rm -rf "$S/out/$CASE-D"; mkdir -p "$S/out/$CASE-D"
mcopy -i "$CARD@@$OFF2" "::FSBENCH/*.DAT" "$S/out/$CASE-D/" && ( cd "$S/out/$CASE-D" && shasum -a 256 -c "$S/files/SHA256SUMS" ) > "$S/out/$CASE-Dcheck.txt" 2>&1
cat "$S/out/$CASE-Dcheck.txt"
echo "run $CASE done"
