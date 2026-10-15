#!/usr/bin/env bash
# crchw2.sh -- crash with open windows: card f1crchw = the saved card (NEWDESK.INF holds the open C:\ window) + F1 -> CRCHILD (write fault, crash report + key).
# kernel kf2; EL0 and desk=kernel, on/off/strict. Dumps: after the crash report is shown, and after the desktop is back (second "desktop up") + 10 s.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in ${MODES:-on off strict}; do
  bash "$HERE/pair2.sh" kf2 /tmp/m5j/a15/cards/f1crchw.img crchw $m '\[desk\] desktop up@1=2:start.png' 'Press any key@1=3:report.png' '\[desk\] desktop up@2=10:end.png'
done
