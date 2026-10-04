#!/bin/sh
# run-hatari-doom-keys.sh <DOOM.PRG> <tos.img> <out-dir>
# NOT YET RUN (2026-10-04): the m68k DOOM.PRG could not be built -- the
# x86_64-only m68k-atari-mint binutils hang at exec on this host (RUNINFO.TXT).
#
# Hatari 2.6.1 TT (ttlow, VGA, 68882, 14 MiB, real time) with the given TOS
# (tos306de.rom or etos512de.img), the K02-instrumented DOOM.PRG built by
# build-doom-instrumented.sh <dir> m68k (-warp 1: straight into E1M1),
# autostarted from C:. After DOOM's "player 1 of 1" line + 5 s it drives the
# user's sequence through --cmd-fifo ("hatari-event keydown|keyup <ST scancode
# decimal>", recipe of measurement-2026-10-03-keyseq):
#   UP(72) down 0, RIGHT(77) down 1.5, RIGHT up 3.0, UP up 5.0,
#   LEFT(75) tap 7.0-7.1 (100 ms), LEFT tap 8.0-8.04 (40 ms),
#   F10(68) 10.0-10.05, then 'y' on both layouts: 21 (US y / DE z) and
#   44 (DE y / US z) -- DOOM's quit prompt ignores the other one,
#   then in the desktop: ESC(1) and Alt-C after DOOM is gone (a kbdvec left
#   pointing into freed memory would crash on the first key: bombs).
# Screenshots: during UP+RIGHT, after the quit, after the desktop keys.
set -eu
PRG=$1; TOS=$2; OUT=$3
S=$(cd "$(dirname "$0")" && pwd)
WS=$(cd "$S/../../.." && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
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
steps = [(0.0,"keydown 72"),(1.5,"keydown 77"),(2.5,"shot"),(3.0,"keyup 77"),(5.0,"keyup 72"),
         (7.0,"keydown 75"),(7.1,"keyup 75"),(8.0,"keydown 75"),(8.04,"keyup 75"),
         (10.0,"keydown 68"),(10.05,"keyup 68"),
         (13.0,"keydown 21"),(13.05,"keyup 21"),(14.0,"keydown 44"),(14.05,"keyup 44"),
         (30.0,"shot"),
         (32.0,"keydown 1"),(32.05,"keyup 1"),
         (33.0,"keydown 56"),(33.05,"keydown 46"),(33.10,"keyup 46"),(33.15,"keyup 56"),
         (38.0,"shot")]
t0 = time.time(); f = open(fifo, "w"); l = open(log, "w")
for at, ev in steps:
    d = at - (time.time() - t0)
    if d > 0: time.sleep(d)
    f.write("hatari-shortcut screenshot\n" if ev == "shot" else "hatari-event %s\n" % ev); f.flush()
    l.write("%.3f s  %s\n" % (time.time() - t0, ev)); l.flush()
time.sleep(3)
PY
kill $HPID 2>/dev/null || true; wait $HPID 2>/dev/null || true
rm -f "$FIFO"; rm -rf "$HD"
