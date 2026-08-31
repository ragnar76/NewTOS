#!/usr/bin/env bash
#
# build-apitest.sh -- build the NewTOS console API diagnostic APITEST.NCP.
#
# APITEST is a mintlib-linked native console program (`.NCP`, EL0) that runs
# from C:\AUTO\ and traces the GEMDOS/BIOS + low-memory-sysvar surface to the
# HDMI framebuffer console AND C:\APITEST.LOG (see apitest.c).
#
# It is a thin wrapper over the proven libc client link path
# (sdk/libc/build-libc.sh): that script extracts pristine mintlib-0.60.1,
# applies the NewTOS overlay (sdk/libc/overlay/ -- the DTA/BASEPAGE ABI fixes
# in mint/ostruct.h + mint/basepage.h are in there), builds libc.a with the
# AArch64 arch shims + libc-aware crt0, then links crt0 + our client + libc.a
# through sdk/newtos.ld into the .NCP. APITEST pulls only the header-only
# <mint/osbind.h> trap macros plus crt0's __libc_init_heap, so the link is
# 0 undef.
#
# Usage:  sdk/apps/apitest/build-apitest.sh [output.NCP]
#   output.NCP  defaults to sdk/apps/apitest/APITEST.NCP
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"   # sdk/apps/apitest
SDK="$(cd "$HERE/../.." && pwd)"                        # sdk
SRC="$HERE/apitest.c"
OUT="${1:-$HERE/APITEST.NCP}"

[ -f "$SRC" ] || { echo "error: $SRC missing" >&2; exit 1; }

echo "building APITEST.NCP via sdk/libc/build-libc.sh"
echo "  src = $SRC"
echo "  out = $OUT"
echo

exec "$SDK/libc/build-libc.sh" "$SRC" "$OUT"
