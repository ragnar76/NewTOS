#!/usr/bin/env bash
# (DESK43 U9-3/4/5 copy of measurement-2026-10-14-desk43-u9-1-2/method/derive-card.sh. Added: ZOFF=1 in the environment
#  turns an existing `#Z` line into `#z` (ignored by AES and desktop) before the insertion -- a #Z card becomes a desktop card.)
# derive-card.sh <src.img> <dst.img> <inf-line|-> [host-file ...] -- DESK43 U9-1/U9-2.
# A private copy of a QEMU test card (never a real card) with
#   * <inf-line> inserted into C:\NEWDESK.INF right after its #R line ('-' = INF unchanged), and
#   * every host-file copied to C:\ ;
# both given the fixed FAT time 2026-10-14 12:00, so two cards derived the same way differ only in
# the INF line's bytes (the directory window's text view shows sizes and dates).
# Pattern of seed-u6a.sh (hdiutil attach -nomount, private mount point, AppleDouble/fseventsd cleanup).
set -euo pipefail
SRC="${1:?}"; DST="${2:?}"; LINE="${3:?}"; shift 3
cp "$SRC" "$DST"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$DST" | head -1 | awk '{print $1}')
MNT="$(dirname "$DST")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
for f in "$@"; do cp -X "$f" "$MNT/"; touch -t 202610141200 "$MNT/$(basename "$f")"; done
if [ "$LINE" != "-" ]; then
  python3 - "$MNT/NEWDESK.INF" "$LINE" <<'PY'
import sys
p, line = sys.argv[1], sys.argv[2]
b = open(p, 'rb').read()
import os
if os.environ.get('ZOFF') == '1':
    assert b.count(b'\n#Z ') == 1, 'ZOFF: expected one #Z line'
    b = b.replace(b'\n#Z ', b'\n#z ')
i = b.index(b'#R')
j = b.index(b'\n', i) + 1
open(p, 'wb').write(b[:j] + line.encode() + b'\r\n' + b[j:])
PY
  touch -t 202610141200 "$MNT/NEWDESK.INF"
fi
find "$MNT" -name '._*' -delete 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
echo "derived $DST from $(basename "$SRC"): $(head -c 120 "$MNT/NEWDESK.INF" | tr '\r\n' '  ') | files: $*"
