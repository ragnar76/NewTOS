#!/usr/bin/env bash
#
# waith-host.sh -- build and run the host harness for the <sys/wait.h> status
# macros (TASK/PORT-WAITH): waith-probe.c, compiled with the include path and
# DEFS of sdk/libc/build-libc.sh (so the overlaid include/sys/wait.h is the
# one in effect), plus mintlib's UNMODIFIED unix/wait4.c, waitpid.c and
# wait.c (minus their weak_alias lines -- an ELF .weak directive the Mach-O
# host assembler rejects; waith-probe.c defines wait4/waitpid/wait as plain
# calls of __wait4/__waitpid/__wait instead), their MiNT calls routed into
# the probe (waith-host-mintbind.h).
#
# With mintlib's pristine header (big-endian bit-field order) the checks that
# read an exit code, signal or union-wait member fail on this
# little-endian host -- WEXITSTATUS is 0 for every status.
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (mintlib tree not extracted, or host not little-endian LP64); 1 = did not
# build. WAITH_HEADER_DIR=<dir> puts <dir>/sys/wait.h in front (red runs).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -f "$TOP/unix/wait4.c" ] || {
  echo "waith-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
RES="$("$CC" -print-resource-dir)/include"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

PRE=(); [ -n "${WAITH_HEADER_DIR:-}" ] && PRE=(-I"$WAITH_HEADER_DIR")
MFLAGS=(-O2 -fno-builtin -nostdinc -Wall -Wextra
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        ${PRE[@]+"${PRE[@]}"}
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
if [ -z "${WAITH_HEADER_DIR:-}" ] && ! cmp -s "$LIBC/overlay/include/sys/wait.h" "$TOP/include/sys/wait.h"; then
  echo "FAIL  the extracted tree's include/sys/wait.h is not the overlay; run build-libc.sh"
  exit 1
fi
for f in wait4 waitpid wait; do
  sed '/^weak_alias/d' "$TOP/unix/$f.c" > "$W/$f.c"
  "$CC" "${MFLAGS[@]}" -D_MINT_MINTBIND_H=1 -include "$HERE/waith-host-mintbind.h" \
      -I"$TOP/unix" -c "$W/$f.c" -o "$W/$f.o" || exit 1
done
"$CC" "${MFLAGS[@]}" -c "$HERE/waith-probe.c" -o "$W/probe.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/waith-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/waith-host" || exit 1

"$W/waith-host"
exit $?
