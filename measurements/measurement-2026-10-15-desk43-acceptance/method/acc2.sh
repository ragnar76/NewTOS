#!/usr/bin/env bash
# acc2.sh -- accessory SYSACC: kernel ka (acc.script: Desk menu -> first accessory; injector tag acc), card /tmp/m5j/acc/cards/acc.img,
# EL0 and desk=kernel, on/off/strict, marker-keyed.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
read -ra NS <<< "$(python3 "$HERE/fx.py" "$HERE/acc.script" 510 1310)"
for m in ${MODES:-on off strict}; do
  bash "$HERE/pair2.sh" ka /tmp/m5j/acc/cards/acc.img acc $m '\[desk\] desktop up@1=3:desk.png' "\\[acc\\] desktop feed@${NS[0]}=3:open.png" "\\[acc\\] desktop feed@${NS[1]}=3:after.png"
done
