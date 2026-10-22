#!/usr/bin/env bash
# build-probe.sh -- A116 probe binaries (sdk/examples/a116w.c, a116acc.c of the workspace) -> $S/bin/A116W.NAP, A116ACC.NAC
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; mkdir -p "$S/bin"
bash "$WS/sdk/build.sh" "$WS/sdk/examples/a116w.c" "$S/bin/A116W.NAP" >/dev/null
bash "$WS/sdk/build.sh" "$WS/sdk/examples/a116acc.c" "$S/bin/A116ACC.NAC" >/dev/null
shasum -a 256 "$S/bin/A116W.NAP" "$S/bin/A116ACC.NAC"
