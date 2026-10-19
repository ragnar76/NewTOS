#!/usr/bin/env bash
# cards.sh -- base cards of the interactive delete/save runs (../mkcard.sh, private 64 MiB FAT16, label P4FPRB, never a real card):
#   dsk  NEWDESK: README.TXT, SUB\NOTE.TXT, SUB\OTHER.TXT + desk48.inf (= measurement-2026-10-17-desk48/method/NEWDESK.INF)
#   tdd  TeraDesk #Z (z-teradesk.inf, TERADESK.INF from measurement-2026-10-18-con01-test m113) + DELME.TXT, KEEP.TXT
#   qsv  QED #Z (z-qed.inf), no data files
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; mkdir -p "$C"
T="$WS/sdk/apps/teradesk/build/dist"; Q="$WS/sdk/apps/qed/build"
DIRS="SUB" bash "$HERE/../mkcard.sh" "$C/dsk.img" "$HERE/desk48.inf" README.TXT="$HERE/README.TXT" SUB/NOTE.TXT="$HERE/NOTE.TXT" \
  SUB/OTHER.TXT="$HERE/OTHER.TXT" > "$C/dsk.log" 2>&1
DIRS="" bash "$HERE/../mkcard.sh" "$C/tdd.img" "$HERE/z-teradesk.inf" DELME.TXT="$HERE/DELME.TXT" KEEP.TXT="$HERE/KEEP.TXT" \
  Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" \
  Apps/TERADESK/cicons.rsc="$T/cicons.rsc" Apps/TERADESK/TERADESK.INF="$HERE/TERADESK.INF" > "$C/tdd.log" 2>&1
DIRS="" bash "$HERE/../mkcard.sh" "$C/qsv.img" "$HERE/z-qed.inf" Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc" > "$C/qsv.log" 2>&1
shasum -a 256 "$T/DESKTOP.NAP" "$Q/QED.NAP" "$C"/dsk.img "$C"/tdd.img "$C"/qsv.img
