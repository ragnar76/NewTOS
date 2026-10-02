#!/bin/sh
# run-hatari-doom.sh <tag> <DOOM.PRG> <out-dir> <run-vbls> [enhancer] [-- doom args]
#   tag: falcon-tc | tt-ttlow
#   enhancer: also C:\AUTO\ENHANCER.PRG = VDI-Enhancer 1.00 (Behne&Behne,
#   Applications/z-tools/zview/_dist/goodies/Enhancer/Enhancer.prg), which
#   installs an 'EdDI' cookie and answers vq_scrninfo on the Atari VDI --
#   a real EdDI, the second implementation next to NewTOS.
# Based on measurement-2026-10-02-scrninfo/method/run-hatari.sh: EmuTOS 1.4
# (etos512de.img), Hatari 2.6.1 headless, fresh GEMDOS C: per run,
# --conout 2 puts the console (DOOM's printf) into hatari.log. DOOM.PRG and
# DOOM1.WAD sit in C:\ and are autostarted by --auto. 14 MiB ST-RAM (DOOM's
# zone). --fpu 68882: the -m68020-60 build (libc multilib) needs an FPU, as
# a TT030 has one; a Falcon gets the usual 68882 add-on. Screenshots: "hatari-shortcut screenshot" through --cmd-fifo at
# fixed wall-clock offsets (SHOTS="s1 s2 ...", default "40 80").
# A DOOM command tail cannot be given through --auto; the doom args after
# "--" are therefore NOT passed (kept for symmetry, unused).
set -eu
TAG=$1; PRG=$2; OUT=$3; VBLS=$4; ENH=${5:-}
S=$(cd "$(dirname "$0")" && pwd)
WS=$(cd "$S/../../.." && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=/Volumes/Daten/Development/RuSTari/roms/etos512de.img
SCN=$WS/measurements/measurement-2026-10-02-scrninfo/method
mkdir -p "$OUT"
HD=$OUT/hd
rm -rf "$HD"; mkdir -p "$HD" "$OUT/shots"
cp "$PRG" "$HD/DOOM.PRG"
cp "$WS/Applications/doom-wad/doom1.wad" "$HD/DOOM1.WAD"
if [ "$ENH" = enhancer ]; then
  mkdir -p "$HD/AUTO"
  cp "$WS/Applications/z-tools/zview/_dist/goodies/Enhancer/Enhancer.prg" "$HD/AUTO/ENHANCER.PRG"
fi
case $TAG in
  tt-ttlow)  MACH="--machine tt --tos-res ttlow --monitor vga" ;;
  falcon-tc) MACH="--machine falcon --monitor vga"
             cp "$SCN/EMUDESK-falcon-truecolor.INF" "$HD/EMUDESK.INF" ;;
  *) echo "unknown tag $TAG" >&2; exit 1 ;;
esac
FIFO=$OUT/cmd.fifo
rm -f "$FIFO"
# shellcheck disable=SC2086
SDL_VIDEODRIVER=dummy "$H" $MACH --fpu 68882 --tos "$TOS" --memsize 14 --sound off \
  --fast-forward on --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\DOOM.PRG' \
  --screenshot-dir "$OUT/shots" --screenshot-format png --crop on \
  --cmd-fifo "$FIFO" --run-vbls "$VBLS" --log-level info > "$OUT/hatari.log" 2>&1 &
HPID=$!
t=0
for s in ${SHOTS:-40 80}; do
  while [ "$t" -lt "$s" ]; do
    kill -0 "$HPID" 2>/dev/null || break 2
    sleep 1; t=$((t + 1))
  done
  [ -p "$FIFO" ] && echo "hatari-shortcut screenshot" > "$FIFO" && echo "shot at ${t}s"
done
wait "$HPID" 2>/dev/null || true
rm -f "$FIFO"
rm -rf "$HD"   # scratch C: drive
ls "$OUT/shots"
