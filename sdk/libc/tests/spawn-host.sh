#!/usr/bin/env bash
#
# spawn-host.sh -- build and run the host harness for the LP64 spawn*
# overlays (TASK/PORT10): sdk/libc/overlay/mintlib/spawnve.c + spawnvp.c,
# compiled with the include path and DEFS of sdk/libc/build-libc.sh, their
# four outside references renamed into spawn-host.c's recorders.
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (mintlib tree not extracted, or host not LP64); 1 = did not build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -f "$TOP/include/process.h" ] || {
  echo "spawn-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
RES="$("$CC" -print-resource-dir)/include"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

MFLAGS=(-O2 -fno-builtin -nostdinc -Wall -Wextra
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        -D_spawnve=spawn_test__spawnve -D_buffindfile=spawn_test__buffindfile
        -Dgetenv=spawn_test_getenv -Denviron=spawn_test_environ
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
for f in spawnve spawnvp; do
  "$CC" "${MFLAGS[@]}" -c "$LIBC/overlay/mintlib/$f.c" -o "$W/$f.o" || exit 1
done
"$CC" -O2 -Wall -Wextra -c "$HERE/spawn-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/spawn-host" || exit 1

"$W/spawn-host"
exit $?
