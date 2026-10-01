#!/usr/bin/env bash
# seed_d33.sh <src.img> <dst.img>
#
# The DESK33 fixture: copy the image the `d33-seed` run left behind (its
# C:\NEWDESK.INF was written by the desktop's OWN app_save) and insert one
# installed-application line that carries a FUNCTION KEY:
#
#   #G 06 FF C:\ARGVTEST.NAP@ *.XYZ@ 107 @
#
# `#G` = a GEM application ANODE; `06` = IG_APPL, `FF` = no data icon;
# `C:\ARGVTEST.NAP` is `a_pappl`, `*.XYZ` the document wildcard `a_pdata`, and
# the extra block `1` `07` `` is `a_flags = AF_APPDIR`, `a_funkey = 7`, empty
# `a_pargs` (kernel/src/desk/inf.rs `parse_anode`/`InfData::to_bytes`, EmuTOS
# `deskapp.c:380-397` / `:1263-1274`).  The line is written in exactly the
# syntax our own writer emits, and the round trip is proven by the run's Save
# desktop, which writes it back unchanged.
#
# The line goes in front of the generic `#G 06 FF *.NAP@ @`, where a real
# installed application belongs (app_alloc links newest first).
set -euo pipefail
SRC="${1:?usage: seed_d33.sh <src.img> <dst.img>}"
DST="${2:?usage: seed_d33.sh <src.img> <dst.img>}"
cp "$SRC" "$DST"

DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$DST" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1

python3 - "$V/NEWDESK.INF" <<'PY'
import sys
p = sys.argv[1]
s = open(p, 'rb').read()
line = b'#G 06 FF C:\\ARGVTEST.NAP@ *.XYZ@ 107 @\r\n'
anchor = b'#G 06 FF *.NAP@ @\r\n'
assert s.count(anchor) == 1, 'generic *.NAP line not found'
assert line not in s
open(p, 'wb').write(s.replace(anchor, line + anchor, 1))
print('NEWDESK.INF: inserted', line.decode().strip())
PY

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $DST (F7 -> C:\\ARGVTEST.NAP)"
