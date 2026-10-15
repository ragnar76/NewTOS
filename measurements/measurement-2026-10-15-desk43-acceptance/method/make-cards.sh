#!/usr/bin/env bash
# make-cards.sh -- DESK43 U9-5: the derived cards of this run in /tmp/m5j/a15/cards (derive-card.sh; never a real card).
# Desktop cards with a program on F1 (`#G|#F 03 FF   <path>@ @ 001 @`, the U9-0 newdesk-f1.inf form), from the MEM05
# plain card /tmp/m5j/u5b/cards/plain.img unless noted:
#   f1hg  HELLOGEM.NAP (+file)   f1hgp  HGPANIC.NAP (+file, hello-gem --features panic-test)
#   f1crch C:\CRCHILD.NAP (bare: case 'w', a write fault)   f1crash C:\CRASHTST.NAP (six crash cases + one clean)
#   f1qed C:\Apps\QED\QED.NAP   f1tera C:\Apps\TERADESK\DESKTOP.NAP   f1api C:\APITEST.NCP (#F)
#   f1doom C:\DOOM\DOOM.NAP, from /tmp/m5j/cards/doom.img with its #Z line turned off (ZOFF=1)
# acc.img (SYSACC.NAC in C:\ACC) and the four series cards are used as they are.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; W=/tmp/m5j/a15; C="$W/cards"; P=/tmp/m5j/u5b/cards/plain.img
mkdir -p "$C"
d() { bash "$HERE/derive-card.sh" "$P" "$C/$1.img" "${@:2}"; }
d f1hg '#G 03 FF   C:\HELLOGEM.NAP@ @ 001 @' "$B/HELLOGEM.NAP"
d f1hgp '#G 03 FF   C:\HGPANIC.NAP@ @ 001 @' "$B/HGPANIC.NAP"
d f1crch '#G 03 FF   C:\CRCHILD.NAP@ @ 001 @'
d f1crash '#G 03 FF   C:\CRASHTST.NAP@ @ 001 @'
d f1qed '#G 03 FF   C:\Apps\QED\QED.NAP@ @ 001 @'
d f1tera '#G 03 FF   C:\Apps\TERADESK\DESKTOP.NAP@ @ 001 @'
d f1api '#F 03 FF   C:\APITEST.NCP@ @ 001 @'
ZOFF=1 bash "$HERE/derive-card.sh" /tmp/m5j/cards/doom.img "$C/f1doom.img" '#G 03 FF   C:\DOOM\DOOM.NAP@ @ 001 @'
(cd "$C" && shasum -a 256 *.img) > "$HERE/../checks/acc-cards-sha256.txt"
