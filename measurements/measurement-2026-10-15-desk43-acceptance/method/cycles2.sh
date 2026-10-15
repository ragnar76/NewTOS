#!/usr/bin/env bash
# cycles2.sh -- 20 program start/exit cycles (kxcy: cycles.script = 20 x F1/Return, then the configuration dialog) vs 0 cycles (kxcy0), card f1hg,
# EL0 and desk=kernel, mode on. Dump 3 s after the dialog opens (last click). Compare the "Free RAM" box (pixdiff) and the MEM05 stats lines.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/tmp/m5j/a15/cards
n20=$(python3 "$HERE/fx.py" "$HERE/cycles.script" 12810); n0=$(python3 "$HERE/fx.py" "$HERE/cycles0.script" 410)
TMO=300 bash "$HERE/pair2.sh" kxcy "$C/f1hg.img" cyc20 on "\\[kxcy\\] desktop feed@$n20=3:cfg.png"
TMO=300 bash "$HERE/pair2.sh" kxcy0 "$C/f1hg.img" cyc0 on "\\[kxcy0\\] desktop feed@$n0=3:cfg.png"
