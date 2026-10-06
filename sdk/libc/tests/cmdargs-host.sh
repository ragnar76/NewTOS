#!/usr/bin/env bash
#
# cmdargs-host.sh -- build and run the host harness for
# sdk/libc/newtos-arch/cmdargs.c (__libc_parseargs: environment block +
# BasePage command tail -> environ, argc/argv; TASK/PORT01, TASK/PORT09).
#
# Two translation units on purpose: cmdargs.c needs mintlib's include path
# (<mint/basepage.h>), the harness needs the HOST's <stdio.h>, and the two
# include paths cannot both be first. The numbers that connect them -- the
# 256-byte BasePage with p_cmdlin at 0x80 -- are pinned by _Static_asserts
# inside cmdargs.c.
#
# -O2 -fno-builtin as in the SDK builds, so the loops are compiled the way
# they are on the target.
#
# Exit status: the number of failed cases (0 = all passed); 77 = skipped
# because the host is not LP64 little-endian; 1 = did not build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
MINT="$LIBC/mintlib/include"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -d "$MINT" ] || {
  echo "cmdargs-host: skipped -- mintlib headers missing ($MINT); run build-libc.sh"
  exit 77
}

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

# -Denviron=...: cmdargs.c publishes mintlib's `environ` (mintlib/globals.c);
# on the host that name belongs to the C runtime, so the harness provides the
# variable under its own name and checks it (TASK/PORT10). Likewise mintlib's
# program_invocation_name / _short_name (mintlib/program-name.c, TASK/PORT09).
"$CC" -O2 -fno-builtin -Wall -Wextra -Wno-macro-redefined \
      -Denviron=cmdargs_test_environ \
      -Dprogram_invocation_name=cmdargs_test_pin \
      -Dprogram_invocation_short_name=cmdargs_test_pisn \
      -I "$MINT" -c "$LIBC/newtos-arch/cmdargs.c" -o "$W/cmdargs.o" || exit 1
"$CC" -O2 -fno-builtin -Wall -Wextra \
      -c "$HERE/cmdargs-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W/cmdargs.o" "$W/harness.o" -o "$W/cmdargs-host" || exit 1

"$W/cmdargs-host"
exit $?
