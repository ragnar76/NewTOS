#!/bin/sh
# run-hatari-keyseq.sh <DOOM.PRG> <out-dir>
# EmuTOS 1.4 (etos512de.img) on Hatari 2.6.1, TT low (the port06-eddi
# "tt-ttlow" setup, no Enhancer, real-time -- NO --fast-forward, so wall time
# ~ emulated time), the K06-instrumented m68k DOOM.PRG autostarted from C:.
# After the "player 1 of 1" console line (WAD loaded, game loop next) it waits 5 s,
# then drives the key sequence through Hatari's --cmd-fifo ("hatari-event
# keydown|keyup <ST scancode>", src/control.c Control_InsertKey ->
# IKBD_PressSTKey):  UP 72 down t=0, RIGHT 77 down t=1.5, RIGHT up t=3, UP up t=5.
set -eu
PRG=$1; OUT=$2
S=$(cd "$(dirname "$0")" && pwd)
WS=$(cd "$S/../../.." && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=/Volumes/Daten/Development/RuSTari/roms/etos512de.img
mkdir -p "$OUT"; HD=$OUT/hd; rm -rf "$HD"; mkdir -p "$HD" "$OUT/shots"
cp "$PRG" "$HD/DOOM.PRG"; cp "$WS/Applications/doom-wad/doom1.wad" "$HD/DOOM1.WAD"
FIFO=$OUT/cmd.fifo; rm -f "$FIFO"
SDL_VIDEODRIVER=dummy "$H" --machine tt --tos-res ttlow --monitor vga --fpu 68882 --tos "$TOS" \
  --memsize 14 --sound off --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\DOOM.PRG' \
  --screenshot-dir "$OUT/shots" --screenshot-format png --crop on \
  --cmd-fifo "$FIFO" --log-level info > "$OUT/hatari.log" 2> "$OUT/hatari.err" &
HPID=$!
n=0
until grep -q "player 1 of 1" "$OUT/hatari.log" 2>/dev/null; do
  kill -0 $HPID 2>/dev/null || { echo "hatari died"; exit 1; }
  sleep 1; n=$((n+1)); [ $n -gt 300 ] && { echo timeout; kill $HPID; exit 1; }
done
echo "doom main loop up after ${n}s wall"
sleep 5
python3 - "$FIFO" "$OUT/inject.log" <<'PY'
import sys, time
fifo, log = sys.argv[1:3]
steps = [(0.0,"keydown 72"),(1.5,"keydown 77"),(3.0,"keyup 77"),(5.0,"keyup 72")]
t0 = time.time(); f = open(fifo, "w"); l = open(log, "w")
for at, ev in steps:
    d = at - (time.time() - t0)
    if d > 0: time.sleep(d)
    f.write("hatari-event %s\n" % ev); f.flush()
    l.write("%.3f s  %s\n" % (time.time() - t0, ev)); l.flush()
time.sleep(6)
f.write("hatari-shortcut screenshot\n"); f.flush(); time.sleep(2)
PY
kill $HPID 2>/dev/null || true; wait $HPID 2>/dev/null || true
rm -f "$FIFO"; rm -rf "$HD"
