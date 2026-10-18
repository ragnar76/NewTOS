#!/usr/bin/env bash
# cards.sh -- the regression base cards (one per content; runs clone them): api (APITEST, A113API label via api-card.sh),
# s2 / s2s / vf (every S2 program at C:\, #Z S2ALL.NAP / S2ALLS.NAP / VFORKT.NAP), tdq (TeraDesk #Z + QED,
# TERADESK-QED.INF: F1 starts QED). Private 64 MiB FAT16 images in the scratch dir, never a real card.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/../env.sh"; C="$S/cards"; mkdir -p "$C"
bash "$HERE/api-card.sh" "$C/api.img" api "$S/api/APITEST.NCP" "$S/api/PTERMCH.NCP" "$S/api/DTACHILD.NCP" > "$C/api.log" 2>&1
S2=""; for f in "$S"/s2/*.N??; do S2="$S2 $(basename "$f")=$f"; done
for k in s2:S2ALL.NAP s2s:S2ALLS.NAP vf:VFORKT.NAP; do
  n="${k%%:*}"; p="${k#*:}"; printf '#Z 01 C:\\%s@\n' "$p" > "$C/$n.inf"
  bash "$HERE/../mkcard.sh" "$C/$n.img" "$C/$n.inf" $S2 > "$C/$n.log" 2>&1
done
T="$WS/sdk/apps/teradesk/build/dist"; Q="$WS/sdk/apps/qed/build"
printf '#Z 01 C:\\Apps\\TERADESK\\DESKTOP.NAP@\n' > "$C/tdq.inf"
bash "$HERE/../mkcard.sh" "$C/tdq.img" "$C/tdq.inf" Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc" \
  Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" \
  Apps/TERADESK/cicons.rsc="$T/cicons.rsc" TERADESK.INF="$HERE/TERADESK-QED.INF" Apps/TERADESK/TERADESK.INF="$HERE/TERADESK-QED.INF" > "$C/tdq.log" 2>&1
shasum -a 256 "$T/DESKTOP.NAP" "$Q/QED.NAP" "$S"/s2/*.N?? "$S"/api/*.NCP
