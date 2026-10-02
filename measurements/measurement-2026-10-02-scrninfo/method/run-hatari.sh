#!/bin/sh
# run-hatari.sh <machine-args-tag> <out-dir> [enhancer]
#   tag: tt-ttmed | tt-ttlow | falcon-tc
#   enhancer: also put the VDI-Enhancer 1.00 (Behne&Behne, from
#   Applications/z-tools/zview/_dist/goodies/Enhancer/Enhancer.prg) into C:\AUTO,
#   which installs an EdDI cookie on the Atari VDI -- the only EdDI available
#   in an emulator, used to drive the probe's EdDI path.
# Boots EmuTOS 1.4 (etos512de.img, sha256 5805c0a4...) in Hatari 2.6.1 headless,
# autostarts C:\SCRNINFO.TTP (no argument -> C:\SCRNINFO.TXT), lets it run for
# a fixed number of VBLs, then copies the report and the Hatari log out.
# C: is a fresh GEMDOS-emulated folder under <out-dir>/hd per run.
set -eu
TAG=$1; OUT=$2; ENH=${3:-}
S=$(cd "$(dirname "$0")" && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
TOS=/Volumes/Daten/Development/RuSTari/roms/etos512de.img
PRG=$S/../SCRNINFO.TTP
HD=$OUT/hd
rm -rf "$HD"; mkdir -p "$HD"
cp "$PRG" "$HD/SCRNINFO.TTP"
if [ "$ENH" = enhancer ]; then
  mkdir -p "$HD/AUTO"
  cp "$S/../../../Applications/z-tools/zview/_dist/goodies/Enhancer/Enhancer.prg" \
     "$HD/AUTO/ENHANCER.PRG"
fi
case $TAG in
  tt-ttmed)  MACH="--machine tt --tos-res ttmed --monitor vga" ;;
  tt-ttlow)  MACH="--machine tt --tos-res ttlow --monitor vga" ;;
  falcon-tc) MACH="--machine falcon --monitor vga"
             cp "$S/EMUDESK-falcon-truecolor.INF" "$HD/EMUDESK.INF" ;;
  *) echo "unknown tag $TAG" >&2; exit 1 ;;
esac
# shellcheck disable=SC2086
SDL_VIDEODRIVER=dummy "$H" $MACH --tos "$TOS" --sound off --fast-forward on \
  --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\SCRNINFO.TTP' \
  --run-vbls 3000 --log-level info > "$OUT/hatari.log" 2>&1 || true
cp "$HD/SCRNINFO.TXT" "$OUT/SCRNINFO.TXT" 2>/dev/null || echo "no SCRNINFO.TXT" >&2
rm -rf "$HD"   # scratch C: drive, the report was copied out above
