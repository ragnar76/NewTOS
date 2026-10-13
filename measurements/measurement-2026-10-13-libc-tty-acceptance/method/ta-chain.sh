#!/usr/bin/env bash
# ta-chain.sh <mode> -- all raspi4b series of this acceptance for one protection mode, sequentially
# (tty-series, cm-series + cm-batch2, stderr matrix on the aux/pre/prefs11 kernels, EmuCON stderr).
# cwd must be the session scratchpad (relative socket dir s/, see run-rpi4*.sh). Card roots under cards-ta/<mode>.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; m="${1:?}"
CR="/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/0c54029c-ee4e-40c3-bb6a-48ecd5c51e63/scratchpad/cards-ta/$m"
mkdir -p "$CR"
bash "$HERE/tty-series.sh" tta "$CR/tt" $m
bash "$HERE/cm-series.sh" tta "$CR/cm" $m
bash "$HERE/cm-batch2.sh" tta "$CR/cm" $m
bash "$HERE/stderr-matrix-aux.sh" "$CR/ac" $m
bash "$HERE/stderr-emucon.sh" "$CR/ac" $m
echo "CHAIN $m DONE"
