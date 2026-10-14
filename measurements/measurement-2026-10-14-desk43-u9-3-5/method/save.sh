#!/usr/bin/env bash
# save.sh <mode> -- DESK43 U9-5: Options -> Save desktop (save.script after Alt-C) on ONE kernel (bin/kernel8-ksave.img),
# EL0 NEWDESK and `desk=kernel`; the run's card copy is kept (run-rpi4.sh KEEP=1), C:\NEWDESK.INF read back from it
# (read-only mount) into ../checks/save-<path>-<mode>-NEWDESK.INF, the card's original INF as save-orig-NEWDESK.INF.
# (First attempt dumped at 12/20 s and quit before the click at ~22 s -- nothing saved; re-run with 25/40 s.)
set -uo pipefail
m="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; CK="$HERE/../checks"
K="$HERE/../bin/kernel8-ksave.img"; W=/tmp/m5j/u935; mkdir -p "$W/save"; cp /tmp/m5j/u5b/cards/plain.img "$W/save/plain.img"
M="$m"; . "$HERE/mode.sh"
BASEARGS="$APPEND"
getinf() { # getinf <img> <out>
  DEV=$(hdiutil attach -readonly -nomount -imagekey diskimage-class=CRawDiskImage "$1" 2>/dev/null | head -1 | awk '{print $1}')
  MNT="$W/save/mnt"; mkdir -p "$MNT"; diskutil mount readOnly -mountPoint "$MNT" "${DEV}s1" > /dev/null
  cp "$MNT/NEWDESK.INF" "$2"; ls -l "$MNT/NEWDESK.INF" | awk '{print $5, $6, $7, $8}' > "$2.ls"
  diskutil unmount "$MNT" > /dev/null; hdiutil detach "$DEV" > /dev/null 2>&1
}
getinf "$W/save/plain.img" "$CK/save-orig-NEWDESK.INF"
for p in el0 el1; do
  if [ $p = el1 ]; then export APPEND="$BASEARGS desk=kernel"; else export APPEND="$BASEARGS"; fi
  KEEP=1 bash "$HERE/run-rpi4.sh" "$K" "$W/save/plain.img" "save-$p-$m-rpi4" 25:t25.png 40:t40.png
  getinf "$W/save/run-save-$p-$m-rpi4.img" "$CK/save-$p-$m-NEWDESK.INF"; rm -f "$W/save/run-save-$p-$m-rpi4.img"
done
cmp "$CK/save-el0-$m-NEWDESK.INF" "$CK/save-el1-$m-NEWDESK.INF" && echo "save $m: EL0 and EL1 NEWDESK.INF byte-identical"
