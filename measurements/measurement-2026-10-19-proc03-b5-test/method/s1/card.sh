#!/usr/bin/env bash
# card.sh <probe-bin-dir> <img> -- the probe card (one base image, cloned per run by runts-mode.sh)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$1"
bash "$HERE/mkcard.sh" "$2" "$HERE/p3s1.inf" AUTO/P3AUTO.NCP="$B/P3AUTO.NCP" P3Z.NAP="$B/P3Z.NAP" P3Z2.NAP="$B/P3Z2.NAP" \
  ACC/P3ACC.NAC="$B/P3ACC.NAC" P3E.NCP="$B/P3E.NCP" SUB/P3E.NCP="$B/P3E.NCP"
