#!/usr/bin/env bash
# ta-kernels.sh <scratch-root> -- all instrumented scratch kernels of this acceptance, from scratch copies of HEAD.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; R="${1:?}"
bash "$HERE/scratch-kernels.sh" tta "$R/sk" || echo "scratch-kernels FAILED"
bash "$HERE/tty-kernels.sh" tta "$R/tt" || echo "tty-kernels FAILED"
bash "$HERE/stderr-kernels-new.sh" "$R/se" || echo "stderr-kernels-new FAILED"
bash "$HERE/vreg-kernels.sh" "$R/vr" || echo "vreg-kernels FAILED"
echo ta-kernels-done
