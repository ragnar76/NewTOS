#!/usr/bin/env bash
# runs-e9-extra.sh <label> <cards-dir> <on|off|strict> -- the E9-specific
# raspi4b runs on top of runs-e9.sh:
#   TeraDesk starts QED (TERADESK-QED.INF assigns F1 to QED; the injector
#     kernel kernel8-tdkeys-<L>.img, tdqed-keys.py, presses F1 5 s after
#     TeraDesk runs): a child Pexec from a GEM app that shrank its
#     largest-block TPA at start. (TeraDesk's AT_AUTO autostart is _MINT_ +
#     multitasking-AES only, main.c:1203-1211, so it cannot be used.)
#   EmuCON runs five programs in a row (kernel8-keys9-<L>.img, built from a
#     scratch copy with emucon-schedule-e9.py): HELLO2, E9PLAIN, E9MEM (which
#     starts its own child), HELLO2 again, OLDEXEC (refused), exit.
set -uo pipefail
L="$1"; C="$2"; M="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
. "$HERE/mode.sh"
T="$L-$M"
mkdir -p "$C"
[ -f "$C/tdqed.img" ] || TDINF="$HERE/TERADESK-QED.INF" bash "$HERE/seed-card.sh" "$C/tdqed.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
[ -f "$C/emucon9.img" ] || bash "$HERE/seed-card.sh" "$C/emucon9.img" 'C:\KBDVEC.NAP' > /dev/null
export APPEND="$A4"
bash "$HERE/run-rpi4.sh" "$B/kernel8-tdkeys-$L.img" "$C/tdqed.img" "$T-rpi4-tdqed" 30:t30.png 50:t50.png
bash "$HERE/run-rpi4.sh" "$B/kernel8-keys9-$L.img" "$C/emucon9.img" "$T-rpi4-emucon9" 20:t20.png 32:t32.png 45:t45.png 70:t70.png
