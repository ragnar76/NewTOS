#!/bin/sh
# run-hatari-tt-doomstat.sh <DOOMSTAT.PRG> <out-dir> <scratch-dir> [seconds=60]
# Machine as measurement-2026-10-04-keys02-tt/method/run-hatari-tt.sh (TT, TOS
# 3.06, 4 MiB ST + 16 MiB TT-RAM, ttlow, real time, --auto C:\DOOM.PRG), console
# to stdout (--conout 2). No keys: DOOM runs its title/demo loop. After
# the first frame's VERIFIED line plus <seconds> Hatari is asked to quit over the cmd-fifo, then kill -9 (it does not
# exit on SIGTERM). DOOMSTAT.TXT is copied out of the GEMDOS drive directory.
set -eu
PRG=$1; OUT=$2; SCR=$3; SECS=${4:-60}
S=$(cd "$(dirname "$0")" && pwd)
WS=$(cd "$S/../../.." && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=/Volumes/Daten/Development/RuSTari/roms/tos306de.rom
mkdir -p "$OUT/shots" "$SCR"; HD=$SCR/hd-tt306; rm -rf "$HD"; mkdir -p "$HD"
cp "$PRG" "$HD/DOOM.PRG"; cp "$WS/Applications/doom-wad/doom1.wad" "$HD/DOOM1.WAD"
FIFO=$SCR/cmd-tt306.fifo; rm -f "$FIFO"
SDL_VIDEODRIVER=dummy "$H" --machine tt --tos-res ttlow --monitor vga --memsize 4 --ttram 16 --addr24 off \
  --fpu 68882 --tos "$TOS" --sound off --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\DOOM.PRG' \
  --screenshot-dir "$OUT/shots" --screenshot-format png --crop on \
  --cmd-fifo "$FIFO" --log-level info > "$OUT/hatari.log" 2> "$OUT/hatari.err" &
HPID=$!
i=0; while [ ! -p "$FIFO" ] && [ $i -lt 30 ]; do sleep 1; i=$((i+1)); done
# TOS 3.06 tests 20 MiB at real time first: wait (max 600 s) for the first
# frame's VERIFIED line, then let DOOM run <seconds>.
i=0; while ! grep -q "VERIFIED" "$OUT/hatari.log" 2>/dev/null && [ $i -lt 600 ]; do sleep 1; i=$((i+1)); done
echo "VERIFIED after ${i} s of waiting"
sleep "$SECS"
echo "hatari-shortcut screenshot" > "$FIFO"; sleep 2
echo "hatari-shortcut quit" > "$FIFO"; sleep 5
kill -0 $HPID 2>/dev/null && { echo "hatari still alive after quit -> kill -9"; kill -9 $HPID; }
wait $HPID 2>/dev/null || true
ls -l "$HD" > "$OUT/hd-listing.txt"
cp "$HD/DOOMSTAT.TXT" "$OUT/DOOMSTAT.TXT" 2>/dev/null || echo "NO DOOMSTAT.TXT in $HD"
rm -f "$FIFO"; rm -rf "$HD"
