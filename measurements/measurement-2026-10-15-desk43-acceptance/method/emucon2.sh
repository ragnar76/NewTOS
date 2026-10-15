#!/usr/bin/env bash
# emucon2.sh -- kernel kxe (emucon.script: Ctrl-Z -> EmuCON, "run hello2.ncp", "run oldexec.ncp", "exit"), plain card, EL0 and desk=kernel, on/off/strict.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
read -ra N <<< "$(python3 "$HERE/fx.py" "$HERE/emucon.script" 230 745 1745 2645)"
for m in ${MODES:-on off strict}; do
  bash "$HERE/pair2.sh" kxe /tmp/m5j/u5b/cards/plain.img emucon $m "\\[kxe\\] desktop feed@${N[0]}=3:cli.png" "\\[kxe\\] desktop feed@${N[1]}=3:hello2.png" "\\[kxe\\] desktop feed@${N[2]}=3:oldexec.png" "\\[kxe\\] desktop feed@${N[3]}=4:exit.png"
done
