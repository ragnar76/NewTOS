#!/usr/bin/env bash
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$HERE/inter-new.sh"
bash "$HERE/m113/matrix.sh"
bash "$HERE/series-n.sh"
echo ALLDONE
