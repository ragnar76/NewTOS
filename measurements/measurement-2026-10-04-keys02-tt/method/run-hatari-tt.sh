#!/bin/sh
# run-hatari-tt.sh <tag: tt306|ttemu> <mode: shipped|keys> <DOOM.PRG> <out-dir> [scratch]
# Derived from keys02-doom/method/run-hatari-doom-keys.sh (machine settings of
# doom-aspect/run-hatari-doom.sh: TT, 4 MiB ST + 16 MiB TT-RAM, real time).
#  shipped: wait for VERIFIED line, shots, F10 + y, desktop keys.
#  keys:    wait for "player 1 of 1"(-warp 1), +5 s, then the key script
#           (drive-keys.py), quit via hatari-shortcut quit over the cmd-fifo.
set -eu
TAG=$1; MODE=$2; PRG=$3; OUT=$4; SCR=${5:-$OUT}
S=$(cd "$(dirname "$0")" && pwd)
WS=$(cd "$S/../../.." && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
ROMS=/Volumes/Daten/Development/RuSTari/roms
case $TAG in tt306) TOS=$ROMS/tos306de.rom;; ttemu) TOS=$ROMS/etos512de.img;; esac
mkdir -p "$OUT/shots" "$SCR"; HD=$SCR/hd-$TAG-$MODE; rm -rf "$HD"; mkdir -p "$HD"
cp "$PRG" "$HD/DOOM.PRG"; cp "$WS/Applications/doom-wad/doom1.wad" "$HD/DOOM1.WAD"
FIFO=$SCR/cmd-$TAG-$MODE.fifo; rm -f "$FIFO"
SDL_VIDEODRIVER=dummy "$H" --machine tt --tos-res ttlow --monitor vga --memsize 4 --ttram 16 --addr24 off \
  --fpu 68882 --tos "$TOS" --sound off --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\DOOM.PRG' \
  --screenshot-dir "$OUT/shots" --screenshot-format png --crop on \
  --cmd-fifo "$FIFO" --log-level info > "$OUT/hatari.log" 2> "$OUT/hatari.err" &
HPID=$!
python3 "$S/drive-keys.py" "$MODE" "$FIFO" "$OUT" "$HPID"
kill -0 $HPID 2>/dev/null && { echo "hatari still alive after quit, waiting 10s"; sleep 10; }
kill -0 $HPID 2>/dev/null && { echo "WARNING: hatari hung, kill -9"; kill -9 $HPID; }
wait $HPID 2>/dev/null || true
rm -f "$FIFO"; rm -rf "$HD"
