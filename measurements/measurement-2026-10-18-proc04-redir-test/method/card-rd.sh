#!/usr/bin/env bash
# card-rd.sh <probe-bin-dir> <img> -- the P4R probe card: C:\AUTO\P4R.NCP (parent), C:\RKID.NCP (child), C:\RIN.TXT
# ("line one" CR LF "X", 11 bytes), NEWDESK.INF = rd.inf (no #Z). One base image, cloned per run by run-probe.sh.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$1"
bash "$HERE/mkcard.sh" "$2" "$HERE/rd.inf" AUTO/P4R.NCP="$B/P4R.NCP" RKID.NCP="$B/RKID.NCP" RIN.TXT="$HERE/RIN.TXT"
