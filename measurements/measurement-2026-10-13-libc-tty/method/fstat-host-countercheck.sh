#!/usr/bin/env bash
# fstat-host-countercheck.sh <scratch-dir> -- red control for the adjusted
# sdk/libc/tests/fstat-host.sh: run it against the REMOVED shim
# (git show HEAD:sdk/libc/newtos-arch/isatty.c, HEAD = ad789346) instead of
# mintlib/isatty.c. Expected: the new isatty checks fail (status/flags never
# set, isatty(-1) = 0); every FS08 check stays green.
set -uo pipefail
S="${1:?scratch dir}"; mkdir -p "$S"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
git -C "$WS" show ad789346:sdk/libc/newtos-arch/isatty.c > "$S/oldisatty.c"
sed "s|\"\$TOP/mintlib/isatty.c\"|\"$S/oldisatty.c\"|; s|^HERE=.*|HERE=$WS/sdk/libc/tests|" \
    "$WS/sdk/libc/tests/fstat-host.sh" > "$S/fstat-host-old.sh"
bash "$S/fstat-host-old.sh"
