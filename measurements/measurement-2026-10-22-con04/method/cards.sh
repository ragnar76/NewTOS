#!/usr/bin/env bash
# cards.sh -- the scratch base cards of this run (one per content; each QEMU run uses an APFS clone, run.sh):
#   vt52a / vt52b : C:\AUTO\VT52A.NCP / VT52B.NCP (the probe, screens 1 / 2), NEWDESK.INF = empty.inf (no #Z)
#   vt52c         : C:\AUTO\VT52C.NCP = screen 1 with Cursconf(3) (blink check, run-blink.sh)
#   regauto       : C:\AUTO\HELLO2.NCP (sdk/examples/hello.c, Cconws "\r\n") + C:\AUTO\COREVAL.NCP (CoreMark validation run,
#                   mintlib printf in text mode) -> then the NEWDESK desktop
#   regtera       : TeraDesk by #Z (z-teradesk.inf, TERADESK.INF from measurement-2026-10-18-proc04-fdelete/method/inter)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
mkdir -p "$S/ncp" "$S/cards" "$S/logs"
printf '#E 1A E0 00 1C 60\r\n' > "$S/cards/empty.inf"
for n in 1 2; do L=$([ $n = 1 ] && echo A || echo B); "$WS/sdk/build.sh" "$HERE/vt52prb$n.c" "$S/ncp/VT52$L.NCP" > "$S/logs/ncp-$L.log"; done
sed 's/cursconf(2, 0);/cursconf(3, 0);/' "$HERE/vt52prb.c" > "$S/vt52prb-blink.c"
printf '#define SCREEN 1\n#include "%s"\n' "$S/vt52prb-blink.c" > "$S/blink1.c"
"$WS/sdk/build.sh" "$S/blink1.c" "$S/ncp/VT52C.NCP" > "$S/logs/ncp-C.log"
"$WS/sdk/build.sh" "$WS/sdk/examples/hello.c" "$S/ncp/HELLO2.NCP" > "$S/logs/ncp-hello.log"
bash "$HERE/mkcard.sh" "$S/cards/vt52a.img" "$S/cards/empty.inf" AUTO/VT52A.NCP="$S/ncp/VT52A.NCP" > /dev/null
bash "$HERE/mkcard.sh" "$S/cards/vt52b.img" "$S/cards/empty.inf" AUTO/VT52B.NCP="$S/ncp/VT52B.NCP" > /dev/null
bash "$HERE/mkcard.sh" "$S/cards/vt52c.img" "$S/cards/empty.inf" AUTO/VT52C.NCP="$S/ncp/VT52C.NCP" > /dev/null
bash "$HERE/mkcard.sh" "$S/cards/regauto.img" "$S/cards/empty.inf" AUTO/HELLO2.NCP="$S/ncp/HELLO2.NCP" \
  AUTO/COREVAL.NCP="$WS/sdk/apps/coremark/build/COREVAL.NCP" > /dev/null
T="$WS/sdk/apps/teradesk/build/dist"
bash "$HERE/mkcard.sh" "$S/cards/regtera.img" "$HERE/z-teradesk.inf" \
  Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" \
  Apps/TERADESK/cicons.rsc="$T/cicons.rsc" Apps/TERADESK/TERADESK.INF="$HERE/TERADESK.INF" > /dev/null
shasum -a 256 "$S"/ncp/*.NCP "$WS/sdk/apps/coremark/build/COREVAL.NCP" "$T/DESKTOP.NAP"
