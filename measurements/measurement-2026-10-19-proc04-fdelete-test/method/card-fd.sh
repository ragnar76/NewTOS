#!/usr/bin/env bash
# card-fd.sh <probe-bin-dir> <img> -- the P4F probe card: C:\AUTO\P4F.NCP (parent), C:\FKID.NCP (child), folders C:\SUB and
# C:\TMP (mkcard.sh), NEWDESK.INF = fd.inf (no #Z). One base image, cloned per run by run-probe.sh.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$1"
bash "$HERE/mkcard.sh" "$2" "$HERE/fd.inf" AUTO/P4F.NCP="$B/P4F.NCP" FKID.NCP="$B/FKID.NCP"
