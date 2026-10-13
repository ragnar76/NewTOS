#!/usr/bin/env bash
#
# malloc-host.sh -- build and run the host harness for the hunk schedule of
# mintlib's malloc with the NewTOS overlay (TASK/MEM05 part B): the overlay
# sdk/libc/overlay/mintlib/malloc.c, compiled with the include path and DEFS
# of sdk/libc/build-libc.sh, <mint/osbind.h> held back and
# malloc-host-gemdos.h forced in (Mfree -> malloc-host.c); malloc-host.c plays
# __sbrk (one hunk = one GEMDOS Malloc), getpagesize and the mintlib globals.
# The weak_alias lines are dropped (an ELF .weak directive the Mach-O host
# assembler rejects); the harness calls __malloc / __mallocChunkSize under
# host-safe names. Every scenario runs in its own process (malloc.c's
# MINHUNK/MAXHUNK/free list are static).
#
# MALLOC_SRC=<file> builds that malloc.c instead of the overlay -- for the
# before/after counter-check against the pristine mintlib file
# (tar xf sdk/libc/mintlib-0.60.1.tar.xz mintlib-0.60.1/mintlib/malloc.c):
# the pristine file has no __newtos_malloc_maxhunk and does not link.
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (mintlib tree not extracted, or host not LP64); 1 = did not build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"
SRC="${MALLOC_SRC:-$LIBC/overlay/mintlib/malloc.c}"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
[ -f "$TOP/mintlib/malloc_int.h" ] || {
  echo "malloc-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
[ -f "$SRC" ] || { echo "error: $SRC missing" >&2; exit 1; }
RES="$("$CC" -print-resource-dir)/include"
echo "malloc.c under test: $SRC"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

MFLAGS=(-O2 -fno-builtin -nostdinc -fno-blocks -Wall
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        -D__malloc=mh_malloc -D__free=mh_free -D__mallocChunkSize=mh_mallocchunksize
        -D__malloczero=mh_malloczero -Dgetpagesize=mh_getpagesize
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
sed '/^weak_alias/d' "$SRC" > "$W/malloc.c"
"$CC" "${MFLAGS[@]}" -D_MINT_OSBIND_H=1 -include "$HERE/malloc-host-gemdos.h" \
    -I"$TOP/mintlib" -c "$W/malloc.c" -o "$W/malloc.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/malloc-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/malloc-host" || exit 1

fail=0
for s in pristine maxhunk acc small chunksize; do
  "$W/malloc-host" "$s"; rc=$?
  [ $rc -eq 77 ] && exit 77
  fail=$((fail + rc))
done
echo "malloc-host: $fail failed"
exit $fail
