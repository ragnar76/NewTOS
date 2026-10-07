#!/usr/bin/env bash
# OFAM freeze: S2ALL (#Z C:\S2ALL.NAP) on the HEAD kernel; probes from build-s2-tests.sh (current libc),
# card = clone of base.img + the probes in C:\; verdict line "S2ALL: PASS|FAIL".
set -uo pipefail
S="${1:?scratch}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; OFF=$((63 * 512)); C="$S/cards"; export SOCKROOT="$S"
CARD="$C/s2all.img"; rm -f "$CARD"; cp -c "$S/base.img" "$CARD"
for f in "$S"/s2bin/*.NAP "$S"/s2bin/*.NCP; do mcopy -o -i "$CARD@@$OFF" "$f" "::$(basename "$f")"; done
sed 's|^#Z .*|#Z 01 C:\\S2ALL.NAP@|' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
mcopy -o -i "$CARD@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
KEEP=1 APPEND=serial=on bash "$HERE/run-rpi4-drive-m3.sh" "$S/kernel8-evb.img" "$CARD" s2all --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$C/run-s2all.img" 2>/dev/null | head -1 | awk '{print $1}')
fsck_msdos -n "${DEV}s1" > "$HERE/../checks/s2all-fsck.txt" 2>&1; hdiutil detach "$DEV" >/dev/null 2>&1; tail -2 "$HERE/../checks/s2all-fsck.txt"
shasum -a 256 "$C/run-s2all.img" | sed 's|  .*|  s2all run clone (deleted)|' >> "$HERE/../checks/sha256-scratch-images.txt"; rm -f "$CARD" "$C/run-s2all.img"
