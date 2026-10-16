#!/usr/bin/env bash
# build-programs.sh <outdir> -- the EL0 programs of this run, built from the shared tree:
#   FPPEXEC.NCP   sdk/build.sh sdk/examples/fppexec.c    (the ARCH02 probe, parent and child in one binary)
#   PTERMCH.NCP   sdk/build.sh sdk/examples/ptermch.c    (APITEST T33 child)
#   DTACHILD.NCP  sdk/build.sh sdk/examples/dtachild.c   (APITEST T29 child)
#   APITEST.NCP   sdk/apps/apitest/build-apitest.sh      (rebuilt: the tree copy is stale, 106 tests)
set -euo pipefail
O="${1:?outdir}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"
mkdir -p "$O"; cd "$WS"
sdk/build.sh sdk/examples/fppexec.c "$O/FPPEXEC.NCP" > "$O/fppexec-build.log" 2>&1
sdk/build.sh sdk/examples/ptermch.c "$O/PTERMCH.NCP" > /dev/null 2>&1
sdk/build.sh sdk/examples/dtachild.c "$O/DTACHILD.NCP" > /dev/null 2>&1
sdk/apps/apitest/build-apitest.sh "$O/APITEST.NCP" > "$O/apitest-build.log" 2>&1
shasum -a 256 "$O"/*.NCP
