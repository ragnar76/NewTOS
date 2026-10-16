#!/usr/bin/env bash
# old-controls.sh -- the same runs as crchw2.sh / readinf3.sh / apps3.sh(crash) on the CONTROL kernels (U9 acceptance base dab1fe62) with THIS run's cards,
# mode on, default boot (= EL0 NEWDESK there). Tags with suffix old: crchwold-el0-on-*, ri3old-el0-on-*, f1crashold-el0-on-*.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards
bash "$HERE/pair2.sh" kf2old $C/f1crchw.img crchwold on '\[desk\] desktop up@1=2:start.png' 'Press any key@1=3:report.png' '\[desk\] desktop up@2=10:end.png'
read -ra N <<< "$(python3 "$HERE/fx.py" "$HERE/ri3.script" 1110 4310 6010)"
bash "$HERE/pair2.sh" kxri3old /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/save/saved-el0-on.img ri3old on '\[desk\] desktop up@1=2:A-start.png' "\[kxri3\] desktop feed@${N[0]}=10:B-closed.png" "\[kxri3\] desktop feed@${N[1]}=10:B2-fsel.png" "\[kxri3\] desktop feed@${N[2]}=14:C-read.png"
TMO=250 bash "$HERE/pair2.sh" kf2old $C/f1crash.img f1crashold on 'starting C:\\CRASHTST@1=20:app.png' '\[desk\] desktop up@2=4:end.png'
