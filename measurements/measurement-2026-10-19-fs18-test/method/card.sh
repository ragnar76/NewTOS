#!/usr/bin/env bash
# card.sh <probe-bin-dir> <img> -- the F18 probe card: C:\AUTO\F18.NCP, folders C:\SUB and C:\TMP (mkcard.sh),
# NEWDESK.INF = fs18.inf (no #Z). One base image, cloned per run by run-probe.sh.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$1"
bash "$HERE/mkcard.sh" "$2" "$HERE/fs18.inf" AUTO/F18.NCP="$B/F18.NCP"
