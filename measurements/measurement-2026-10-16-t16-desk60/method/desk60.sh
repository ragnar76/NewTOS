#!/usr/bin/env bash
# desk60.sh <mode> -- DESK60 (copy of measurement-2026-10-15-desk43-acceptance/method/tdqed2.sh, card path changed): TeraDesk -> QED chain and TeraDesk exit
# (kernel kxtd, tdf1.script on launch:DESKTOP), card tdqed (TeraDesk autostarted by #Z, F1 -> QED;
# /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/cards/tdqed.img), EL0 and desk=kernel. Dumps: TeraDesk up; QED up; after Ctrl-Q in QED;
# after Ctrl-Q + Quit in TeraDesk (the desktop must come back). The serial log carries the DESK60 line after TeraDesk's exit.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
read -ra N <<< "$(python3 "$HERE/fx.py" "$HERE/tdf1.script" 3010 4630 6210)"
TMO=200 bash "$HERE/pair2.sh" kxtd /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/cards/tdqed.img tdqed "${1:-on}" 'starting C:\\Apps\\TERADESK@1=3:td.png' "\\[kxtd\\] launch:DESKTOP feed@${N[0]}=5:qed.png" "\\[kxtd\\] launch:DESKTOP feed@${N[1]}=3:after-qed.png" "\\[kxtd\\] launch:DESKTOP feed@${N[2]}=12:after-td.png"
