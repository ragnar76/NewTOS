#!/usr/bin/env bash
# tdqed2.sh <mode> -- TeraDesk -> QED chain and TeraDesk exit (kernel kxtd, tdf1.script on launch:DESKTOP), card tdqed (TeraDesk autostarted by #Z, F1 -> QED),
# EL0 and desk=kernel. Dumps: TeraDesk up; QED up (F1 +12 s); after Ctrl-Q in QED; after Ctrl-Q + Return in TeraDesk (the desktop must come back).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
read -ra N <<< "$(python3 "$HERE/fx.py" "$HERE/tdf1.script" 3010 4630 6210)"
TMO=200 bash "$HERE/pair2.sh" kxtd /tmp/m5j/acc/cards/tdqed.img tdqed "${1:-on}" 'starting C:\\Apps\\TERADESK@1=3:td.png' "\\[kxtd\\] launch:DESKTOP feed@${N[0]}=5:qed.png" "\\[kxtd\\] launch:DESKTOP feed@${N[1]}=3:after-qed.png" "\\[kxtd\\] launch:DESKTOP feed@${N[2]}=12:after-td.png"
