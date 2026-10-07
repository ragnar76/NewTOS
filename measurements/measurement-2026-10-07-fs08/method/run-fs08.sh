#!/usr/bin/env bash
# run-fs08.sh <scratch> <kernel8.img> <case> -- one QEMU raspi4b run (serial=on),
# private card copy patched with mtools, serial to <scratch>/serial/<case>.log,
# screendumps via qmp_timed.py, C:\FS08.TXT + C:\FREADBEN.LOG extracted after.
# Cases: zview  (C:\ZVLAUNCH.NAP via #Z opens C:\PHOTO\BIG.JPG in zView)
#        bench  (C:\AUTO\FREADBEN.NCP reads C:\FS08TEST.BIN, then desktop)
#        deskbench (interactive FREADBEN.NCP via #Z, as the user starts it)
#        idle   (plain boot to the desktop, probe only)
set -uo pipefail
S="$1"; K="$2"; CASE="$3"; SECS="${SECS:-60}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OFF=$((63 * 512)); CARD="$S/cards/$CASE.img"; mkdir -p "$S/cards" "$S/serial" "$S/shots" "$S/out"
cp "$S/base.img" "$CARD"
mi() { mcopy -o -i "$CARD@@$OFF" "$@"; }
mmd -i "$CARD@@$OFF" ::PHOTO 2>/dev/null || true
mi "$S/files/BIG.JPG" ::PHOTO/BIG.JPG
mi "$S/files/FS08TEST.BIN" ::FS08TEST.BIN
case "$CASE" in
zview*)
  sed 's|^#Z .*|#Z 01 C:\\ZVLAUNCH.NAP@|' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mi "$S/NEWDESK.INF" ::NEWDESK.INF
  mi "$S/bin/ZVLAUNCH.NAP" ::ZVLAUNCH.NAP
  printf 'C:\\PHOTO\\BIG.JPG\r\n' > "$S/ZVARG.TXT"; mi "$S/ZVARG.TXT" ::ZVARG.TXT ;;
bench*)
  sed 's|^#Z .*||' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mi "$S/NEWDESK.INF" ::NEWDESK.INF
  mmd -i "$CARD@@$OFF" ::AUTO 2>/dev/null || true
  mi "$S/bin/FREADBEN-NOWAIT.NCP" ::AUTO/FREADBEN.NCP ;;
deskbench*)
  # the interactive FREADBEN.NCP (waits for a key) started from the desktop (#Z)
  sed 's|^#Z .*|#Z 01 C:\\FREADBEN.NCP@|' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mi "$S/NEWDESK.INF" ::NEWDESK.INF
  mi "$S/bin/FREADBEN.NCP" ::FREADBEN.NCP ;;
idle*)
  sed 's|^#Z .*||' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mi "$S/NEWDESK.INF" ::NEWDESK.INF ;;
esac
SD="$(mktemp -d /private/tmp/claude-501/f08.XXXXXX)"
LOG="$S/serial/$CASE.log"; rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:"$SD/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/qmp_timed.py" "$SD/qmp.sock" "$S/shots" "$((SECS/2)):$CASE-mid.png" "$SECS:$CASE-end.png"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
mcopy -o -i "$CARD@@$OFF" ::FS08.TXT "$S/out/$CASE-FS08.TXT" 2>/dev/null && echo "FS08.TXT extracted"
mcopy -o -i "$CARD@@$OFF" ::FREADBEN.LOG "$S/out/$CASE-FREADBEN.LOG" 2>/dev/null && echo "FREADBEN.LOG extracted"
echo "run $CASE done"
