#!/usr/bin/env bash
# apps3.sh -- programs started with F1 from the desktop (kernel kf2: desktop=f1.script, launch:X=ret.script/crash-keys.script; injector tag u6a),
# marker-keyed. EL0 NEWDESK and desk=kernel; cards /tmp/m5j/a15/cards. ONLY=<names> limits.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/tmp/m5j/a15/cards
want() { [ -z "${ONLY:-}" ] || [[ " $ONLY " == *" $1 "* ]]; }
b() { bash "$HERE/pair2.sh" kf2 "$@"; }
for m in ${MODES:-on off strict}; do
  want hg && b "$C/f1hg.img" f1hg $m 'starting C:\\HELLOGEM@1=5:app.png' '\[desk\] desktop up@2=4:end.png'
  want hgp && b "$C/f1hgp.img" f1hgp $m 'starting C:\\HGPANIC@1=5:app.png' '\[desk\] desktop up@2=4:end.png'
  want crch && b "$C/f1crch.img" f1crch $m 'starting C:\\CRCHILD@1=5:app.png' '\[desk\] desktop up@2=4:end.png'
done
want crash && TMO=250 b "$C/f1crash.img" f1crash on 'starting C:\\CRASHTST@1=20:app.png' '\[desk\] desktop up@2=4:end.png'
want qed && b "$C/f1qed.img" f1qed on 'starting C:\\Apps\\QED@1=15:app.png'
want tera && b "$C/f1tera.img" f1tera on 'starting C:\\Apps\\TERADESK@1=15:app.png'
want api && b "$C/f1api.img" f1api on 'TOTAL +[0-9]+@1=2:t-a.png' 'TOTAL +[0-9]+@1=15:t-b.png'
want doom && b "$C/f1doom.img" f1doom on 'starting C:\\DOOM\\DOOM.NAP@1=25:t1.png'
