#!/bin/sh
# run-profile.sh <tag> <DOOM.PRG> <out-dir> <scratch-dir> [enhancer]
#   env DETAIL (0/1, default 0), BLOCKS (screenblocks 3..11, default 10)
#   tag: tt306 | ttemu   (TT030, TT-low 320x480x8, 68882, 4 MiB ST + 16 MiB TT-RAM)
#   env FRAMES (default 200): profile window = FRAMES calls of R_RenderPlayerView
#   env PEXECN (default 1; 2 with enhancer): which Pexec(0) is DOOM.PRG
# Derived from measurement-2026-10-04-doom-aspect/method/run-hatari-doom.sh.
# Profiling: Hatari debugger breakpoint chain (docs: Hatari doc/debugger.html,
# "Chaining breakpoints", "Collecting profile data"), no stdin needed:
#   pexec.ini  -> b <Pexec(0)>        :file $D/program.ini
#   program.ini-> b pc = TEXT         :file $D/trace.ini     (DOOM entry)
#   trace.ini  -> symbols prg; b _R_Init -> cfg.ini (memwrite detail/blocks); b R_RenderPlayerView :once :file $D/start.ini
#   start.ini  -> profile on; b R_RenderPlayerView :FRAMES :file $D/stop.ini
#   stop.ini   -> profile stats/cycles/callers/addresses (program text + ROM); quit
set -eu
TAG=$1; PRG=$2; OUT=$3; SCR=$4; ENH=${5:-}
S=$(cd "$(dirname "$0")" && pwd)
WS=$(cd "$S/../../.." && pwd)
H=/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
ROMS=/Volumes/Daten/Development/RuSTari/roms
FRAMES=${FRAMES:-200}
VBLS=${VBLS:-200000}
mkdir -p "$OUT" "$SCR"
HD=$SCR/hd-$TAG; rm -rf "$HD"; mkdir -p "$HD"
cp "$PRG" "$HD/DOOM.PRG"
cp "$WS/Applications/doom-wad/doom1.wad" "$HD/DOOM1.WAD"
PEXECN=${PEXECN:-1}
if [ "$ENH" = enhancer ]; then
  mkdir -p "$HD/AUTO"
  cp "$WS/Applications/z-tools/zview/_dist/goodies/Enhancer/Enhancer.prg" "$HD/AUTO/ENHANCER.PRG"
  PEXECN=${PEXECN_ENH:-2}
fi
TT="--machine tt --tos-res ttlow --monitor vga --memsize 4 --ttram 16 --addr24 off"
case $TAG in
  tt306) MACH="$TT --tos $ROMS/tos306de.rom" ;;
  ttemu) MACH="$TT --tos $ROMS/etos512de.img" ;;
  *) echo "unknown tag $TAG" >&2; exit 1 ;;
esac
D=$SCR/ini-$TAG; mkdir -p "$D"
CNT=""; [ "$PEXECN" -gt 1 ] && CNT=":$PEXECN"
cat > "$D/pexec.ini" <<EOT
b GemdosOpcode = 0x4B && OsCallParam = 0x0 $CNT :trace :once :file $D/program.ini
EOT
cat > "$D/program.ini" <<EOT
b pc = TEXT :trace :once :file $D/trace.ini
EOT
# detail/screenblocks: doomgeneric is built with ORIGCODE undefined (config.h:97),
# so default.cfg is never read; the values are poked into DOOM's variables
# (m_menu.c detailLevel / screenblocks) when R_Init is entered, before it
# calls R_SetViewSize(screenblocks, detailLevel).
cat > "$D/cfg.ini" <<EOT
memwrite l _detailLevel ${DETAIL:-0}
memwrite l _screenblocks ${BLOCKS:-10}
EOT
cat > "$D/trace.ini" <<EOT
symbols prg
b pc = _R_Init :trace :once :file $D/cfg.ini
b pc = _R_RenderPlayerView :trace :once :file $D/start.ini
EOT
cat > "$D/start.ini" <<EOT
profile on
b pc = _R_RenderPlayerView :$FRAMES :trace :once :file $D/stop.ini
EOT
cat > "$D/stop.ini" <<EOT
profile stats
profile cycles 70
profile callers
profile addresses 0x101a000-0x1090000
profile addresses 0xe00000-0xe80000
quit
EOT
FIFO=$SCR/cmd-$TAG.fifo; rm -f "$FIFO"
echo "hatari $MACH --fpu 68882 (enhancer: ${ENH:-no}) FRAMES=$FRAMES PEXECN=$PEXECN DETAIL=${DETAIL:-0} BLOCKS=${BLOCKS:-10}" > "$OUT/cmdline.txt"
# shellcheck disable=SC2086
SDL_VIDEODRIVER=dummy "$H" $MACH --fpu 68882 --sound off \
  --fast-forward on --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\DOOM.PRG' \
  --cmd-fifo "$FIFO" --run-vbls "$VBLS" --log-level info --parse "$D/pexec.ini" \
  < /dev/null > "$OUT/hatari.log" 2>&1 &
HPID=$!
MAXWALL=${MAXWALL:-1500}
t=0
while kill -0 "$HPID" 2>/dev/null && [ "$t" -lt "$MAXWALL" ]; do sleep 2; t=$((t + 2)); done
if kill -0 "$HPID" 2>/dev/null; then
  echo "wall limit $MAXWALL s reached" | tee -a "$OUT/hatari.log"
  [ -p "$FIFO" ] && echo "hatari-shortcut quit" > "$FIFO" &
  n=0; while kill -0 "$HPID" 2>/dev/null && [ "$n" -lt 10 ]; do sleep 1; n=$((n + 1)); done
  kill -9 "$HPID" 2>/dev/null || true
fi
wait "$HPID" 2>/dev/null || true
echo "wall $t s" >> "$OUT/cmdline.txt"
cp "$D"/*.ini "$OUT/" 2>/dev/null || true
rm -f "$FIFO"; rm -rf "$HD"
