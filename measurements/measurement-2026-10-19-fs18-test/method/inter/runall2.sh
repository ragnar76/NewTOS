#!/usr/bin/env bash
# runall2.sh <side...> -- the delete / copy / move matrix of measurement-2026-10-19-proc04-fdelete-test on the fs18 kernels:
# dsk (NEWDESK trash x2), dcm (NEWDESK copy file/folder, Ctrl-move, delete folder), tcp (TeraDesk copy, delete), tdd (TeraDesk trash),
# + dmv (NEWDESK folder move, rename file + folder), tmv (TeraDesk move file + folder, rename folder) when their kernels exist.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for side in "$@"; do
  LSDIRS="/ /SUB" DL=0.8 TMO=150 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-dsk.img" "$S/cards/dsk.img" "dsk-$side" inj desktop="$HERE/dsk.script"
  LSDIRS="/ /SUB /SUB/FOLD /SUB/FOLD/INNER" DL=0.8 TMO=200 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-dcm.img" "$S/cards/dcm.img" "dcm-$side" inj desktop="$HERE/dcm.script"
  LSDIRS="/ /SUB /SUB/FOLD /SUB/FOLD/INNER" DL=0.8 TMO=200 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-tcp.img" "$S/cards/tcp.img" "tcp-$side" inj up:DESKTOP="$HERE/tcp.script"
  LSDIRS="/" DL=0.8 TMO=150 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-tdd.img" "$S/cards/tdd.img" "tdd-$side" inj up:DESKTOP="$HERE/tdd.script"
  [ -f "$S/bin/kernel8-$side-dmv.img" ] && LSDIRS="/ /SUB /SUB/FOLD /SUB/FOLD/INNER /NEWSUB /NEWSUB/FOLD /NEWSUB/FOLD/INNER" MTYPE="BIG2.TXT" DL=0.8 TMO=200 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-dmv.img" "$S/cards/drn.img" "dmv-$side" inj desktop="$HERE/dmv.script"
  [ -f "$S/bin/kernel8-$side-tmv.img" ] && LSDIRS="/ /SUB /SUB/FOLD /SUB/FOLD/INNER" DL=0.8 TMO=200 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-tmv.img" "$S/cards/tcp.img" "tmv-$side" inj up:DESKTOP="$HERE/tmv.script"
done
