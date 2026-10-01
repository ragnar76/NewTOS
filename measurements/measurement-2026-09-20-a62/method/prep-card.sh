#!/usr/bin/env bash
# prep-card.sh BASE OUT MODE  -- MODE = nest | seq
set -euo pipefail
BASE="$1"; OUT="$2"; MODE="$3"
APPS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cp -f "$BASE" "$OUT"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"
if [ "$MODE" = nest ]; then
  cp -f "$APPS/NESTTST.NAP" "$V/NESTTST.NAP"
  printf '#Z 01 C:\\NESTTST.NAP@\r\n' > "$V/NEWDESK.INF"
else
  for n in 1 2 3 4 5 6 7 8 9; do cp -f "$APPS/SEQTST.NAP" "$V/SEQ$n.NAP"; done
  printf '#Z 01 C:\\SEQ1.NAP@\r\n' > "$V/NEWDESK.INF"
fi
ls "$V" | tr '\n' ' '; echo
sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "prepared $OUT ($MODE)"
