#!/usr/bin/env bash
# boot-smoke.sh <scratch> <kernel8.img> <base.img> <label> <secs> [append]
# Headless QEMU raspi4b boot on an APFS clone of <base.img>; serial log to
# <scratch>/serial/<label>.log; QEMU (our own pid only) killed after <secs>.
# The run clone is fsck'd read-only (both partitions) and then deleted; its
# sha256 is recorded in <scratch>/serial/<label>.card-sha256.
# QEMU_EXTRA (env, optional): extra QEMU arguments, e.g. trace events
#   QEMU_EXTRA="-trace sdcard_normal_command -trace sdcard_app_command -D <file>"
set -uo pipefail
S="$1"; K="$2"; BASE="$3"; L="$4"; T="$5"; APP="${6:-serial=on memprot=on}"
mkdir -p "$S/serial" "$S/cards"
CARD="$S/cards/$L.img"; rm -f "$CARD"; cp -c "$BASE" "$CARD"
LOG="$S/serial/$L.log"; rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" ${QEMU_EXTRA:-} &
QPID=$!
sleep "$T"; kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" | head -1 | awk '{print $1}')
for p in s1 s2; do [ -e "${DEV}$p" ] && { echo "== fsck_msdos -n $p"; fsck_msdos -n "${DEV}$p" 2>&1; }; done > "$S/serial/$L.fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
shasum -a 256 "$CARD" > "$S/serial/$L.card-sha256"; rm -f "$CARD"
