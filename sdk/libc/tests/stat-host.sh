#!/usr/bin/env bash
#
# stat-host.sh -- build and run the host harness for the stat()/lstat()
# timestamps (TASK/SDK01 F): sdk/libc/newtos-arch/do_stat.c (__do_stat,
# __sys_stat), compiled with the include path and DEFS of
# sdk/libc/build-libc.sh, <mint/osbind.h> and <mint/mintbind.h> held back
# and stat-host-gemdos.h forced in (Fstat64, Fsfirst, Fsetdta, Fgetdta,
# Dgetdrv -> stat-host-fs.c); mintlib's own mintlib/unixtime.c (the DOS
# time/date conversion, unchanged) and mintlib/ctype.c (minus its weak_alias
# lines: isupper/_toupper).
#
# STAT_SRC=<file> builds that do_stat.c instead of the tree's -- for the
# before/after counter-check (git show 33ef861a:sdk/libc/newtos-arch/do_stat.c,
# whose __do_stat was __quickstat: then also link the overlay quickstat.c,
# which this script does whenever STAT_SRC is set).
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (mintlib tree not extracted, or host not LP64); 1 = did not build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"
SRC="${STAT_SRC:-$LIBC/newtos-arch/do_stat.c}"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -f "$TOP/mintlib/unixtime.c" ] || {
  echo "stat-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
[ -f "$SRC" ] || { echo "error: $SRC missing" >&2; exit 1; }
RES="$("$CC" -print-resource-dir)/include"
echo "do_stat.c under test: $SRC"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

MFLAGS=(-O2 -fno-builtin -nostdinc -fno-blocks -Wall -Wextra
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        -D_ctype=st_test_ctype -Dgeteuid=st_test_geteuid -Dgetegid=st_test_getegid
        -D_MINT_OSBIND_H=1 -D_MINT_MINTBIND_H=1 -include "$HERE/stat-host-gemdos.h"
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
"$CC" "${MFLAGS[@]}" -c "$SRC" -o "$W/do_stat.o" || exit 1
if [ -n "${STAT_SRC:-}" ]; then
  "$CC" "${MFLAGS[@]}" -c "$LIBC/overlay/mintlib/quickstat.c" -o "$W/quickstat.o" || exit 1
fi
"$CC" "${MFLAGS[@]}" -c "$TOP/mintlib/unixtime.c" -o "$W/unixtime.o" || exit 1
sed '/^weak_alias/d' "$TOP/mintlib/ctype.c" > "$W/ctype.c"
"$CC" "${MFLAGS[@]}" -I"$TOP/mintlib" -c "$W/ctype.c" -o "$W/ctype.o" || exit 1
"$CC" "${MFLAGS[@]}" -c "$HERE/stat-host-fs.c" -o "$W/fs.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/stat-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/stat-host" || exit 1

"$W/stat-host"
