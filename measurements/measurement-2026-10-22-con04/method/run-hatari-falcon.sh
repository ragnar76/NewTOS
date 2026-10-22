#!/bin/sh
# run-hatari-falcon.sh <hd-dir> <shots-dir> <prog> <tag>
# REFERENCE: pinned EmuTOS (references/emutos/etos512us.img) on Hatari 2.6.1 Falcon030, VGA,
# truecolor (EMUDESK.INF "#E 1A E0 00 1C 60"), headless; Hatari autostarts C:\<prog> (a .TOS
# probe that ends in Cnecin), then six screenshots ~0.3 s apart (cursor steadiness), then kill -9
# (Hatari ignores SIGTERM).
# Pattern of measurements/measurement-2026-10-17-desk64/method/run-hatari-adbkgnd.sh.
S=$(cd "$(dirname "$0")" && pwd); HD=$1; SHOTS=$2; PROG=$3; TAG=$4
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=/Volumes/Daten/Development/PiRusTOS/references/emutos/etos512us.img
FIFO=$HD/../fifo-$TAG; rm -f "$FIFO"; mkdir -p "$SHOTS"
send() { ( echo "$1" > "$FIFO" ) & w=$!; sleep 1.5; kill $w 2>/dev/null; }
quick() { ( echo "$1" > "$FIFO" ) & w=$!; sleep 0.3; kill $w 2>/dev/null; }
cp "$S/EMUDESK.INF" "$HD/EMUDESK.INF"
SDL_VIDEODRIVER=dummy "$H" --machine falcon --tos "$TOS" --monitor vga --sound off --statusbar off --fast-forward on \
  --conout 2 -d "$HD" --gemdos-drive C --auto "C:\\$PROG" --screenshot-dir "$SHOTS" --screenshot-format png \
  --cmd-fifo "$FIFO" > "$SHOTS/../hatari-$TAG.log" 2>&1 &
PID=$!
sleep 14
send "hatari-shortcut screenshot"
for i in 2 3 4 5 6; do quick "hatari-shortcut screenshot"; done
sleep 1
kill -9 $PID 2>/dev/null; wait $PID 2>/dev/null
rm -f "$FIFO"
ls "$SHOTS"
