#!/usr/bin/env bash
#
# fstat-host.sh -- build and run the host harness for st_blksize and the stdio
# buffer size it selects (TASK/FS08 part 4): the lean
# sdk/libc/newtos-arch/do_fstat.c and newtos-arch/isatty.c (each minus its
# weak_alias line, an ELF .weak directive the Mach-O host assembler rejects),
# and mintlib's own stdio/stdio_init.c, compiled with the include path and
# DEFS of sdk/libc/build-libc.sh, <mint/osbind.h> held back and
# fstat-host-gemdos.h forced in (Fseek -> fstat-host-fs.c).
#
# DO_FSTAT_SRC=<file> builds that do_fstat.c instead of the tree's -- for the
# before/after counter-check against the previous shim
# (git show f14942c:sdk/libc/newtos-arch/do_fstat.c).
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (mintlib tree not extracted, or host not LP64); 1 = did not build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -f "$TOP/stdio/stdio_init.c" ] || {
  echo "fstat-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
RES="$("$CC" -print-resource-dir)/include"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

MFLAGS=(-O2 -fno-builtin -nostdinc -fno-blocks -Wall -Wextra
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined -D_MINT_OSBIND_H=1 -include "$HERE/fstat-host-gemdos.h"
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
SRC="${DO_FSTAT_SRC:-$LIBC/newtos-arch/do_fstat.c}"
echo "do_fstat.c under test: $SRC"
sed '/^weak_alias/d' "$SRC" > "$W/do_fstat.c"
sed '/^weak_alias/d' "$LIBC/newtos-arch/isatty.c" > "$W/isatty.c"
"$CC" "${MFLAGS[@]}" -I"$LIBC/newtos-arch" -c "$W/do_fstat.c" -o "$W/do_fstat.o" || exit 1
"$CC" "${MFLAGS[@]}" -c "$W/isatty.c" -o "$W/isatty.o" || exit 1
"$CC" "${MFLAGS[@]}" -Wno-void-pointer-to-int-cast \
    -I"$TOP/stdio" -c "$TOP/stdio/stdio_init.c" -o "$W/stdio_init.o" || exit 1
"$CC" "${MFLAGS[@]}" -c "$HERE/fstat-host-fs.c" -o "$W/fs.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/fstat-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/fstat-host" || exit 1

"$W/fstat-host"
