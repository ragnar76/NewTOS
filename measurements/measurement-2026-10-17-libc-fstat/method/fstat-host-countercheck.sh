#!/usr/bin/env bash
# fstat-host-countercheck.sh <scratch-dir> -- red control for the LIBC-FSTAT
# sdk/libc/tests/fstat-host.sh: run it against the PREVIOUS shim
# (git show 70a2e69b:sdk/libc/newtos-arch/do_fstat.c, fds 0..2 always S_IFCHR,
# 32 KiB only for fd >= 6). Expected: the redirected 0..2/3 cases, the device
# 3..5 cases, the isatty/fstat consistency and the stale-cache case fail;
# every FS08 and isatty check stays green.
set -uo pipefail
S="${1:?scratch dir}"; mkdir -p "$S"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
git -C "$WS" show 70a2e69b:sdk/libc/newtos-arch/do_fstat.c > "$S/old_do_fstat.c"
DO_FSTAT_SRC="$S/old_do_fstat.c" bash "$WS/sdk/libc/tests/fstat-host.sh"
