#!/usr/bin/env bash
# kernels-all.sh -- builds every scratch/instrumented kernel of this run (sequential), from scratch copies of HEAD.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
R=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/0c54029c-ee4e-40c3-bb6a-48ecd5c51e63/scratchpad/ac/s
bash $HERE/scratch-kernels.sh cm $R
NEG=0 bash $HERE/asid-kernels.sh cm $R
bash $HERE/fs11-kernels.sh ax $R
bash $HERE/stderr-kernels-new.sh $R
echo ALLKERNELS-DONE
