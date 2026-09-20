#!/bin/sh
# run.sh <hd-dir> <shots-dir> <tag>  -- boot EmuTOS (pinned build) on Hatari Falcon, open C: via Alt+C, screenshot
S=$(dirname "$0"); HD=$1; SHOTS=$2; TAG=$3
H=$HOME/Downloads/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=/Volumes/Daten/Development/PiRusTOS/references/emutos/etos512us.img
FIFO=$S/fifo-$TAG; rm -f "$FIFO"; mkdir -p "$SHOTS"
send() { ( echo "$1" > "$FIFO" ) & w=$!; sleep 1.5; kill $w 2>/dev/null; }
SDL_VIDEODRIVER=dummy "$H" --machine falcon --tos "$TOS" --monitor vga --sound off --fast-forward on \
  --conout 2 -d "$HD" --gemdos-drive C --screenshot-dir "$SHOTS" --screenshot-format png \
  --cmd-fifo "$FIFO" > "$S/hatari-$TAG.log" 2>&1 &
PID=$!
sleep 8
send "hatari-debug info videl"; sleep 1
send "hatari-shortcut screenshot"; sleep 1
send "hatari-event keydown 56"; sleep 0.3; send "hatari-event keypress 46"; sleep 0.3; send "hatari-event keyup 56"; sleep 3
send "hatari-shortcut screenshot"; sleep 2
send "hatari-debug info videl"; sleep 1
kill $PID 2>/dev/null; wait $PID 2>/dev/null
ls "$SHOTS"
