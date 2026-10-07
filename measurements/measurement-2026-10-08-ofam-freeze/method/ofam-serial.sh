#!/usr/bin/env bash
# ofam-serial.sh <scratch> <kernel> <base card> <tag> <secs> <DEST=SRC>... -- AUTO-folder / serial-verdict run:
# clone of <base card> (cp -c), the listed files copied in with mtools (dest relative to C:\), headless
# QEMU raspi4b -append serial=on, serial to serial/<tag>-serial.log, stopped after <secs>; fsck + sha256; clone deleted.
set -uo pipefail
S="$1"; K="$2"; BASE="$3"; TAG="$4"; SECS="$5"; shift 5
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; mkdir -p "$HERE/../serial"
OFF=$((63 * 512)); RUN="$S/cards/run-$TAG.img"; LOG="$HERE/../serial/$TAG-serial.log"
rm -f "$RUN"; cp -c "$BASE" "$RUN"
for spec in "$@"; do dst="${spec%%=*}"; src="${spec#*=}"; d="$(dirname "$dst")"
  [ "$d" != . ] && mmd -i "$RUN@@$OFF" "::$d" 2>/dev/null
  mcopy -o -i "$RUN@@$OFF" "$src" "::$dst"; done
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" -kernel "$K" \
  -drive file="$RUN",if=sd,format=raw -serial file:"$LOG" &
QPID=$!; sleep "$SECS"; kill "$QPID" 2>/dev/null; sleep 1; kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
echo "ran $SECS s -> $LOG ($(wc -c < "$LOG" | tr -d ' ') B)"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$RUN" 2>/dev/null | head -1 | awk '{print $1}')
fsck_msdos -n "${DEV}s1" > "$HERE/../checks/$TAG-fsck.txt" 2>&1; hdiutil detach "$DEV" >/dev/null 2>&1
tail -2 "$HERE/../checks/$TAG-fsck.txt"
shasum -a 256 "$RUN" | sed 's|  .*|  '"$TAG"' run clone (deleted)|' >> "$HERE/../checks/sha256-scratch-images.txt"
rm -f "$RUN"
