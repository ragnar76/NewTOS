#!/usr/bin/env bash
# save2.sh <mode> -- Options > Save desktop (kernel ksave, save.script, injector tag sav): EL0 and desk=kernel, marker-keyed; the run's card copy is kept
# and C:\NEWDESK.INF read back read-only (hdiutil) into ../checks/save2-<path>-<mode>-NEWDESK.INF; the kept card is stored as /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/save/saved-<path>.img
set -uo pipefail
m="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; CK="$HERE/../checks"; W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/save; mkdir -p $W; cp /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/plain.img $W/plain.img
read -ra NS <<< "$(python3 "$HERE/fx.py" "$HERE/save.script" 1310)"
getinf() { DEV=$(hdiutil attach -readonly -nomount -imagekey diskimage-class=CRawDiskImage "$1" 2>/dev/null | head -1 | awk '{print $1}')
  MNT="$W/mnt"; mkdir -p "$MNT"; diskutil mount readOnly -mountPoint "$MNT" "${DEV}s1" > /dev/null
  cp "$MNT/NEWDESK.INF" "$2"; diskutil unmount "$MNT" > /dev/null; hdiutil detach "$DEV" > /dev/null 2>&1; }
getinf $W/plain.img "$CK/save2-orig-NEWDESK.INF"
M="$m"; . "$HERE/mode.sh"; BASEARGS="$APPEND"
for p in el0; do
  if [ $p = el1 ]; then export APPEND="$BASEARGS desk=kernel"; else export APPEND="$BASEARGS"; fi
  KEEP=1 bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-ksave.img" $W/plain.img "save2-$p-$m" "\\[sav\\] desktop feed@${NS[0]}=6:t.png"
  getinf "$W/run-save2-$p-$m.img" "$CK/save2-$p-$m-NEWDESK.INF"; mv "$W/run-save2-$p-$m.img" "$W/saved-$p-$m.img"
done
ls -l "$CK"/save2-*-NEWDESK.INF
