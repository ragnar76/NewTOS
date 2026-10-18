#!/usr/bin/env bash
# card.sh <probe-bin-dir> <img> -- the probe card (one base image, cloned per run by run-probe.sh). P4E.NCP, P4KA.NCP,
# P4KZ.NAP and P4INNER.TXT exist ONLY in C:\SUB, so finding them by a bare name proves the relative resolution.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$1"
bash "$HERE/mkcard.sh" "$2" "$HERE/p4.inf" AUTO/P4AUTO.NCP="$B/P4AUTO.NCP" P4Z.NAP="$B/P4Z.NAP" P4Z2.NAP="$B/P4Z2.NAP" \
  ACC/P4ACC.NAC="$B/P4ACC.NAC" SUB/P4E.NCP="$B/P4E.NCP" SUB/P4KA.NCP="$B/P4KA.NCP" SUB/P4KZ.NAP="$B/P4KZ.NAP" \
  SUB/P4INNER.TXT="$B/P4INNER.TXT"
