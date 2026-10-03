#!/bin/sh
# run-hatari-doom.sh <tag> <DOOM.PRG> <out-dir> <scratch-dir> [enhancer]
#
# Starts DOOM.PRG headless in Hatari 2.6.1 and takes screenshots.
# Derived from measurement-2026-10-02-port06-eddi/method/run-hatari-doom.sh
# (same autostart, console capture and screenshot mechanism); new here: the
# TT gets a realistic memory layout (4 MiB ST-RAM + 16 MiB TT-RAM, 32-bit
# addressing) and the real TT ROM TOS 3.06 next to EmuTOS.
#
#   tag   tt306     TT030, TOS 3.06 (tos306de.rom), TT-low 320x480x8, VGA,
#                   68882, ST 4 MiB, TT-RAM 16 MiB
#         ttemu     the same machine with EmuTOS 1.4 (etos512de.img)
#         falcon    Falcon030, EmuTOS 1.4, truecolour desktop
#                   (EMUDESK-falcon-truecolor.INF of the scrninfo measurement),
#                   68882, 14 MiB ST-RAM, NO TT-RAM (Malloc falls back to ST)
#         falcon8   the same Falcon in 640x480 with 256 colours (Videl mode
#                   0x1B = VGA | 80 columns | 8 bpp), method/EMUDESK-falcon-256.INF
#                   (2026-10-04, doom-aspect: the one 256-colour mode Hatari
#                   offers besides TT-low -- does an EdDI driver call it packed?)
#   enhancer        also C:\AUTO\ENHANCER.PRG = Behne&Behne VDI-Enhancer 1.00
#                   (Applications/z-tools/zview/_dist/goodies/Enhancer/),
#                   which installs the 'EdDI' cookie and answers vq_scrninfo.
#
# DOOM.PRG and DOOM1.WAD sit in C:\ (GEMDOS-emulated drive = a fresh scratch
# directory per run) and are autostarted with --auto. --conout 2 puts DOOM's
# console output into hatari.log. Screenshots through --cmd-fifo
# "hatari-shortcut screenshot" at wall-clock offsets SHOTS (default
# "30 60 90 120"), --fast-forward on; the run ends after VBLS VBLs.
set -eu
TAG=$1; PRG=$2; OUT=$3; SCR=$4; ENH=${5:-}
S=$(cd "$(dirname "$0")" && pwd)
WS=$(cd "$S/../../.." && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
ROMS=/Volumes/Daten/Development/RuSTari/roms
VBLS=${VBLS:-30000}
mkdir -p "$OUT/shots" "$SCR"
HD=$SCR/hd-$TAG; rm -rf "$HD"; mkdir -p "$HD"
cp "$PRG" "$HD/DOOM.PRG"
cp "$WS/Applications/doom-wad/doom1.wad" "$HD/DOOM1.WAD"
if [ "$ENH" = enhancer ]; then
  mkdir -p "$HD/AUTO"
  cp "$WS/Applications/z-tools/zview/_dist/goodies/Enhancer/Enhancer.prg" "$HD/AUTO/ENHANCER.PRG"
fi
TT="--machine tt --tos-res ttlow --monitor vga --memsize 4 --ttram 16 --addr24 off"
case $TAG in
  tt306)  MACH="$TT --tos $ROMS/tos306de.rom" ;;
  ttemu)  MACH="$TT --tos $ROMS/etos512de.img" ;;
  falcon) MACH="--machine falcon --monitor vga --memsize 14 --ttram 0 --tos $ROMS/etos512de.img"
          cp "$WS/measurements/measurement-2026-10-02-scrninfo/method/EMUDESK-falcon-truecolor.INF" "$HD/EMUDESK.INF" ;;
  falcon8) MACH="--machine falcon --monitor vga --memsize 14 --ttram 0 --tos $ROMS/etos512de.img"
          cp "$S/EMUDESK-falcon-256.INF" "$HD/EMUDESK.INF" ;;
  *) echo "unknown tag $TAG" >&2; exit 1 ;;
esac
FIFO=$SCR/cmd-$TAG.fifo
rm -f "$FIFO"
echo "hatari $MACH --fpu 68882 (enhancer: ${ENH:-no})" > "$OUT/cmdline.txt"
# shellcheck disable=SC2086
SDL_VIDEODRIVER=dummy "$H" $MACH --fpu 68882 --sound off \
  --fast-forward on --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\DOOM.PRG' \
  --screenshot-dir "$OUT/shots" --screenshot-format png --crop on \
  --cmd-fifo "$FIFO" --run-vbls "$VBLS" --log-level info > "$OUT/hatari.log" 2>&1 &
HPID=$!
t=0
for s in ${SHOTS:-30 60 90 120}; do
  while [ "$t" -lt "$s" ]; do
    kill -0 "$HPID" 2>/dev/null || break 2
    sleep 1; t=$((t + 1))
  done
  [ -p "$FIFO" ] && echo "hatari-shortcut screenshot" > "$FIFO" && echo "shot at ${t}s"
done
# End of the run: SIGTERM leaves a headless Hatari 2.6.1 hanging (0 % CPU,
# observed twice here), so ask through the FIFO first, then SIGKILL.
[ -p "$FIFO" ] && kill -0 "$HPID" 2>/dev/null && echo "hatari-shortcut quit" > "$FIFO" &
n=0
while kill -0 "$HPID" 2>/dev/null && [ "$n" -lt 10 ]; do sleep 1; n=$((n + 1)); done
kill -9 "$HPID" 2>/dev/null || true
wait "$HPID" 2>/dev/null || true
rm -f "$FIFO"
rm -rf "$HD"
ls "$OUT/shots"
