#!/usr/bin/env bash
#
# quickstat-host.sh -- build and run the host harness for the executable bit
# of mintlib's __quickstat (user decision 2026-10-02, TASK/PORT10): the
# overlay sdk/libc/overlay/mintlib/quickstat.c and the lean
# sdk/libc/newtos-arch/access.c, compiled with the include path and DEFS of
# sdk/libc/build-libc.sh, <mint/osbind.h> held back and
# quickstat-host-gemdos.h forced in (Fsfirst/Fsetdta/Fgetdta/Dgetdrv ->
# quickstat-host-fs.c); mintlib's own mintlib/ctype.c (minus its weak_alias
# lines) supplies the table isupper()/_toupper() read.
#
# QUICKSTAT_SRC=<file> builds that quickstat.c instead of the overlay -- for
# the before/after counter-check against the pristine mintlib file
# (tar xf sdk/libc/mintlib-0.60.1.tar.xz mintlib-0.60.1/mintlib/quickstat.c).
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (mintlib tree not extracted, or host not LP64); 1 = did not build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"
SRC="${QUICKSTAT_SRC:-$LIBC/overlay/mintlib/quickstat.c}"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -f "$TOP/include/mint/ostruct.h" ] || {
  echo "quickstat-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
[ -f "$SRC" ] || { echo "error: $SRC missing" >&2; exit 1; }
RES="$("$CC" -print-resource-dir)/include"
echo "quickstat.c under test: $SRC"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

MFLAGS=(-O2 -fno-builtin -nostdinc -fno-blocks -Wall -Wextra
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        -D_ctype=qs_test_ctype -Dgeteuid=qs_test_geteuid -Dgetegid=qs_test_getegid
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
"$CC" "${MFLAGS[@]}" -D_MINT_OSBIND_H=1 -include "$HERE/quickstat-host-gemdos.h" \
    -I"$TOP/mintlib" -c "$SRC" -o "$W/quickstat.o" || exit 1
# newtos-arch/access.c verbatim except its weak_alias() line (an ELF .weak
# directive the Mach-O host assembler rejects; the harness calls __access).
sed '/^weak_alias/d' "$LIBC/newtos-arch/access.c" > "$W/access.c"
"$CC" "${MFLAGS[@]}" -c "$W/access.c" -o "$W/access.o" || exit 1
sed '/^weak_alias/d' "$TOP/mintlib/ctype.c" > "$W/ctype.c"
"$CC" "${MFLAGS[@]}" -I"$TOP/mintlib" -c "$W/ctype.c" -o "$W/ctype.o" || exit 1
"$CC" "${MFLAGS[@]}" -D_MINT_OSBIND_H=1 -include "$HERE/quickstat-host-gemdos.h" \
    -c "$HERE/quickstat-host-fs.c" -o "$W/fs.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/quickstat-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/quickstat-host" || exit 1

"$W/quickstat-host"
