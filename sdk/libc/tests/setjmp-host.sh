#!/usr/bin/env bash
#
# setjmp-host.sh -- build and run the host harness for newtos-arch/setjmp.S.
#
# The AArch64 setjmp/longjmp is AAPCS64 code with no OS dependency on the
# path NewTOS takes (__mint == 0), so a macOS/arm64 host can run the same
# checks the NewTOS program sdk/examples/setjmptst.c runs. -O2 so the locals
# in the callee-saved-register check really live in x19-x28 / d8-d15, and
# -fno-builtin as in the SDK builds.
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# because the host is not AArch64.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
case "$(uname -m)" in
  arm64|aarch64) ;;
  *) echo "setjmp-host: skipped -- host is $(uname -m), not AArch64"; exit 77 ;;
esac

CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
OUT="$(mktemp -d)/setjmp-host"

"$CLANG" -O2 -fno-builtin -Wall -Wextra -o "$OUT" \
    "$HERE/setjmp-host.c" "$HERE/../newtos-arch/setjmp.S" || exit 1
"$OUT"; rc=$?
rm -rf "$(dirname "$OUT")"
exit $rc
