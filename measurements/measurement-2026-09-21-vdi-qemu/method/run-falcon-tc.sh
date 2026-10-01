#!/bin/sh
# run-falcon-tc.sh <hd-dir> <tos-image> <tag>
# Boots EmuTOS on Hatari Falcon (VGA, truecolor via EMUDESK.INF "#E 1A E0 00 1C 60"),
# autostarts C:\DUMPVDI.PRG (m68k build, tosdump/deploy-tt), asks the debugger for
# the VIDEL shift mode over the command fifo, then kills the emulator.
S=$(cd "$(dirname "$0")" && pwd); HD=$1; TOS=$2; TAG=$3
H=$HOME/Downloads/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari
FIFO=$S/fifo-$TAG; rm -f "$FIFO"
send() { ( echo "$1" > "$FIFO" ) & w=$!; sleep 1.5; kill $w 2>/dev/null; }
rm -f "$HD/VDI.BIN" "$HD/VDI.LOG"
SDL_VIDEODRIVER=dummy "$H" --machine falcon --tos "$TOS" --monitor vga --sound off \
  --fast-forward on --conout 2 -d "$HD" --gemdos-drive C --auto 'C:\DUMPVDI.PRG' \
  --cmd-fifo "$FIFO" --log-level info > "$S/hatari-$TAG.log" 2>&1 &
PID=$!
sleep 14
send "hatari-debug info videl"; sleep 1
kill -9 $PID 2>/dev/null; wait $PID 2>/dev/null
rm -f "$FIFO"
ls -la "$HD"
