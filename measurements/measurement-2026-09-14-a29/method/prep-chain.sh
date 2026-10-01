#!/usr/bin/env bash
# prep-chain.sh OUT_IMG
# Card for the A29 machine run: a real EL0 GEM app that calls appl_init(),
# appl_exit() and Pterm()s without any input -- the "start a program and let it
# end" case, headless.  CHAINTST.NAP chains to itself once and then to
# HELLO.NCP (already on the base image); when the last one ends the AES shell
# loop starts the built-in desktop.
set -euo pipefail
OUT_IMG="$1"
BASE="/Volumes/Daten/Development/PiRusTOS/measurement-2026-09-13-regression/qemu-tmp/sd-base.img"
APP="/Volumes/Daten/Development/PiRusTOS/sdk/apps/chaintest/CHAINTST.NAP"

cp -f "$BASE" "$OUT_IMG"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT_IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"
cp -f "$APP" "$V/CHAINTST.NAP"
printf '#Z 01 C:\\CHAINTST.NAP@\r\n' > "$V/NEWDESK.INF"
sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "prepared $OUT_IMG autorun=C:\\CHAINTST.NAP"
