#!/usr/bin/env bash
#
# check-ilp32.sh -- type-check the demo's shared source the way the Atari
# sees it, without the Atari's compiler.
#
# ball.c and ball-tex.c are built for two very different machines: AArch64
# LP64, where a long is eight bytes, and m68k, where it is four. Most of what
# that changes is caught by the m68k compiler -- but when that toolchain is
# unavailable (on macOS it is regularly quarantined, and then it does not
# fail, it HANGS), the ILP32 half of the contract goes unchecked and a long
# that quietly became 32 bits ships unnoticed.
#
# So: compile the same sources for a 32-bit target with a compiler that is
# available, at both composition depths, with warnings as errors. It proves
# nothing about m68k code generation and nothing about the trap macros, and
# it is not a substitute for the real build. It does catch the whole class of
# "this only fits in a long on the Pi".
#
# Usage:  sdk/apps/ball/hostsim/check-ilp32.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BALL="$(cd "$HERE/.." && pwd)"
CC="${CC:-clang}"
TARGET="${TARGET:-i386-unknown-linux-gnu}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }

# The freestanding target has no libc headers; ball.c wants two functions of
# one of them and nothing else.
cat > "$TMP/string.h" <<'HDR'
#ifndef ILP32_CHECK_STRING_H
#define ILP32_CHECK_STRING_H
#include <stddef.h>
void *memcpy(void *d, const void *s, size_t n);
void *memset(void *d, int c, size_t n);
#endif
HDR

rc=0
for d in 0 1; do
    for s in ball.c ball-tex.c; do
        if "$CC" -target "$TARGET" -ffreestanding -nostdlib -std=c99 \
                 -Wall -Wextra -Werror -DBALL_TRUECOLOR=$d \
                 -I"$TMP" -I"$HERE" -I"$BALL" \
                 -c "$BALL/$s" -o /dev/null 2>"$TMP/err"; then
            echo "  OK  $s  (ILP32, BALL_TRUECOLOR=$d)"
        else
            echo "FAILED  $s  (ILP32, BALL_TRUECOLOR=$d):"
            sed 's/^/    /' "$TMP/err"
            rc=1
        fi
    done
done
exit $rc
