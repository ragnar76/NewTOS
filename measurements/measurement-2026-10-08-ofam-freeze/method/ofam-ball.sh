#!/usr/bin/env bash
# ofam-ball.sh <scratch> <BALL.NAP> <tag> -- the ball demo from the desktop (C:\APPS\BALL\BALL.NAP),
# BALL.CFG/BALL.NTX in C:\ root (where ball.c reads them). Card = one clone of base.img per run.
set -uo pipefail
S="${1:?}"; NAP="${2:?}"; TAG="${3:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OFF=$((63 * 512)); C="$S/cards"; export SOCKROOT="$S"; BD=/Volumes/Daten/Development/ball-demo
CARD="$C/ball-run.img"; rm -f "$CARD"; cp -c "$S/base.img" "$CARD"
mmd -i "$CARD@@$OFF" ::APPS/BALL
mcopy -o -i "$CARD@@$OFF" "$NAP" ::APPS/BALL/BALL.NAP
mcopy -o -i "$CARD@@$OFF" "$BD/BALL.CFG" ::BALL.CFG
mcopy -o -i "$CARD@@$OFF" "${NTX:-$BD/BALL.NTX}" ::BALL.NTX
mcopy -o -i "$CARD@@$OFF" "$HERE/newdesk-ball.inf" ::NEWDESK.INF
KEEP=1 APPEND=serial=on bash "$HERE/run-rpi4-drive-m3.sh" "$S/kinj-evb-deskball.img" "$CARD" "$TAG" --mark-delay 0.3 --end '\[m3\] MARK END' --end-delay 1 --timeout 200
R="$C/run-$TAG.img"
mcopy -n -i "$R@@$OFF" ::BALL.LOG "$S/$TAG-BALL.LOG" 2>/dev/null && cp "$S/$TAG-BALL.LOG" "$HERE/../checks/$TAG-BALL.LOG" && echo "BALL.LOG saved"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$R" 2>/dev/null | head -1 | awk '{print $1}')
fsck_msdos -n "${DEV}s1" > "$HERE/../checks/$TAG-fsck.txt" 2>&1; hdiutil detach "$DEV" >/dev/null 2>&1; tail -2 "$HERE/../checks/$TAG-fsck.txt"
shasum -a 256 "$R" | sed "s|  .*|  $TAG run clone (deleted)|" >> "$HERE/../checks/sha256-scratch-images.txt"; rm -f "$CARD" "$R"
