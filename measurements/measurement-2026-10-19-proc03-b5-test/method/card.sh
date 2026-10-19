#!/usr/bin/env bash
# card.sh <probe-bin-dir> <img> [deref] -- the B5 probe card: #Z C:\B5Z.NAP, C:\ACC\B5ACC.NAC (the -DB5_DEREF build for
# the memprot=off card when the third argument is "deref"). One base image per content, cloned per run.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$1"; A="$B/B5ACC.NAC"; [ "${3:-}" = deref ] && A="$B/B5ACCD.NAC"
printf '#Z 01 C:\\B5Z.NAP@\n' > "$(dirname "$2")/b5.inf"
bash "$HERE/mkcard.sh" "$2" "$(dirname "$2")/b5.inf" B5Z.NAP="$B/B5Z.NAP" ACC/B5ACC.NAC="$A"
