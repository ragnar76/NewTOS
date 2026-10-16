#!/usr/bin/env bash
# programs.sh -- PROC06: the three EL0 programs of this run, built from the shared tree into the scratch dir (copies + sha256 in ../bin):
#   PTERMCH.NCP   sdk/build.sh sdk/examples/ptermch.c     (freestanding; exit code by command tail m/h/z/c)
#   CTRLCPAR.NCP  sdk/build.sh sdk/examples/ctrlcpar.c    (freestanding; Pexec PTERMCH "c", print the result)
#   APITEST.NCP   sdk/apps/apitest/build-apitest.sh       (mintlib; + case T33)
# APITEST-before.NCP = the shared tree's sdk/apps/apitest/APITEST.NCP before the rebuild (byte-identical to the series card's
# C:\APITEST.NCP, checks/cards.txt). The shared tree's APITEST.NCP is left as it was.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"; W=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06
export SOURCE_DATE_EPOCH=1791979200
mkdir -p "$W/b"; cd "$WS"
sdk/build.sh sdk/examples/ptermch.c "$W/b/PTERMCH.NCP"
sdk/build.sh sdk/examples/ctrlcpar.c "$W/b/CTRLCPAR.NCP"
cp sdk/apps/apitest/APITEST.NCP "$W/b/APITEST-before.NCP"
sdk/apps/apitest/build-apitest.sh "$W/b/APITEST.NCP" > "$W/b/apitest-build.log" 2>&1
