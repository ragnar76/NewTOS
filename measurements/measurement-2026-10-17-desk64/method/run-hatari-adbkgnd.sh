#!/bin/sh
# run-hatari-adbkgnd.sh <hd-dir> <shots-dir> <tag> -- REFERENCE: pinned EmuTOS (references/emutos/etos512us.img) on Hatari
# Falcon030, VGA, truecolor (EMUDESK.INF #E .. 00 1C), headless. EMUDESK.INF carries a #K line whose BACKGRND slot
# (index 21, deskmain.c:226-227/294) is 'B', so Ctrl+B opens "Set background" (ADBKGND) without a mouse.
# Pattern of measurements/measurement-2026-09-20-a79/hatari-falcon-reference/method/run-hatari-falcon.sh.
S=$(cd "$(dirname "$0")" && pwd); HD=$1; SHOTS=$2; TAG=$3
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=/Volumes/Daten/Development/PiRusTOS/references/emutos/etos512us.img
FIFO=$HD/../fifo-$TAG; rm -f "$FIFO"; mkdir -p "$SHOTS"
send() { ( echo "$1" > "$FIFO" ) & w=$!; sleep 1.5; kill $w 2>/dev/null; }
SDL_VIDEODRIVER=dummy "$H" --machine falcon --tos "$TOS" --monitor vga --sound off --fast-forward on \
  --conout 2 -d "$HD" --gemdos-drive C --screenshot-dir "$SHOTS" --screenshot-format png \
  --cmd-fifo "$FIFO" > "$SHOTS/../hatari-$TAG.log" 2>&1 &
PID=$!
sleep 10
send "hatari-shortcut screenshot"; sleep 2
send "hatari-event keydown 29"; sleep 0.3; send "hatari-event keypress 48"; sleep 0.3; send "hatari-event keyup 29"; sleep 3
send "hatari-shortcut screenshot"; sleep 2
kill $PID 2>/dev/null; wait $PID 2>/dev/null
ls "$SHOTS"
