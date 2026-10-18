#!/usr/bin/env bash
# inter-new.sh -- the key-input scenarios of this run, head vs wt, one QEMU at a time (marker-keyed dumps via m113/drive.sh)
#   qtype: QED typing (kqt, c-nq)   tkeys: TeraDesk Ctrl+W / Ctrl+Q (ktt, c-nt)   dk: NEWDESK keyboard (kdk, c-nd)   doom: Doom keys (kdoom, doom card)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SC="$HERE/../scripts"
for side in head wt; do
  DL=0.8 TMO=150 bash "$HERE/m113/drive.sh" "$S/bin/kernel8-$side-kqt.img" "$S/img/c-nq.img" "qtype-$side" kn up:QED="$SC/qtype.script"
  DL=0.8 TMO=150 bash "$HERE/m113/drive.sh" "$S/bin/kernel8-$side-ktt.img" "$S/img/c-nt.img" "tkeys-$side" kn up:DESKTOP="$SC/tkeys.script"
  DL=0.8 TMO=150 bash "$HERE/m113/drive.sh" "$S/bin/kernel8-$side-kdk.img" "$S/img/c-nd.img" "dk-$side" kd desktop="$SC/deskkeys.script"
  DL=1.0 TMO=200 bash "$HERE/m113/drive.sh" "$S/bin/kernel8-$side-kdoom.img" "$S/cards/doom.img" "doom-$side" kd launch:DOOM="$SC/doomkeys.script"
done
