#!/bin/sh
S=$(dirname "$0"); HD=$1; SHOTS=$2; TAG=$3
H=$HOME/Downloads/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=${TOS_OVERRIDE:-/Volumes/Daten/Development/PiRusTOS/references/emutos/etos512us.img}
FIFO=$S/fifo-$TAG; rm -f "$FIFO"; mkdir -p "$SHOTS"
send() { ( echo "$1" > "$FIFO" ) & w=$!; sleep 1.5; kill $w 2>/dev/null; }
SDL_VIDEODRIVER=dummy "$H" --machine falcon --tos "$TOS" --monitor vga --sound off --fast-forward on \
  --conout 2 -d "$HD" --gemdos-drive C --screenshot-dir "$SHOTS" --screenshot-format png \
  --cmd-fifo "$FIFO" > "$S/hatari-$TAG.log" 2>&1 &
PID=$!
sleep 10
send "hatari-shortcut screenshot"; sleep 2
kill $PID 2>/dev/null; wait $PID 2>/dev/null
