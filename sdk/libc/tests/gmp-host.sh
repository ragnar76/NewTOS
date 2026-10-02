#!/usr/bin/env bash
#
# gmp-host.sh -- build and run the host harness for the limb width of
# mintlib's GMP core (TASK/PORT10): gmp-probe.c + the UNMODIFIED mintlib
# members stdlib/fpioconst.c, gmp/dbl2mpn.c, gmp/mpn-mul_1.c (plus what they
# pull: gmp/mpn-add_n.c, mpn-sub_n.c, mp_clz_tab.c), compiled with
# the include path and DEFS of sdk/libc/build-libc.sh (so the overlaid
# stdlib/gmp-mparam.h is the one in effect), against gmp-host.c on the host.
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (mintlib tree not extracted, or host not LP64 little-endian); 1 = did not
# build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -f "$TOP/stdlib/fpioconst.c" ] || {
  echo "gmp-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
RES="$("$CC" -print-resource-dir)/include"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

# build-libc.sh's CFLAGS/INCS, minus the cross target (host object format).
MFLAGS=(-O2 -fno-builtin -nostdinc
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
for f in stdlib/fpioconst.c gmp/dbl2mpn.c gmp/mpn-mul_1.c \
         gmp/mpn-add_n.c gmp/mpn-sub_n.c gmp/mp_clz_tab.c; do
  d="$(dirname "$f")"
  "$CC" "${MFLAGS[@]}" -I"$TOP/$d" -c "$TOP/$f" -o "$W/$(basename "${f%.c}").o" || exit 1
done
# -fgnu89-inline: gmp.h's `extern __inline__` mpn helpers (mpn_add, ...) are
# then inline-only in the probe; fpioconst.o already carries the one external
# copy the harness link needs (with C99 inline semantics both would).
"$CC" "${MFLAGS[@]}" -fgnu89-inline -I"$TOP/gmp" -c "$HERE/gmp-probe.c" -o "$W/probe.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/gmp-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/gmp-host" || exit 1

"$W/gmp-host"
exit $?
