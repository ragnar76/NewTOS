#!/usr/bin/env bash
#
# libshare-host.sh -- build and run the host harness for the libshare
# allocator as the SDK builds it (TASK/LDG-M3-B2): the overlaid
# sdk/libshare/libshare-0.1.1/{malloc,sbrk,calloc,realloc,chdir}.c, compiled
# for the host with mintlib's headers, against libshare-host.c (host headers),
# which models ldg_Malloc/ldg_Free and a Pterm that frees by owner. See the
# header of libshare-host.c for what is proved.
#
# The libshare names that the host C library also has are renamed on the
# libshare side only (malloc, free, calloc, realloc, sbrk, errno,
# getpagesize -> ut_*), so the harness keeps the host's allocator.
#
# Exit status: the number of failed checks (0 = all passed); 77 = skipped
# (the extracted libshare tree or mintlib's headers are missing, or the host
# is not LP64); 1 = did not build.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libshare/tests
SDK="$(cd "$HERE/../.." && pwd)"                               # .../sdk
TOP="$SDK/libshare/libshare-0.1.1"
MINT="$SDK/libc/mintlib/include"
LDGINC="$SDK/ldg/ldg-r133/include"

CC="${CC:-cc}"
command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
for d in "$TOP" "$MINT" "$LDGINC"; do
  [ -d "$d" ] || { echo "libshare-host: skipped -- $d missing; run sdk/libshare/build-libshare.sh"; exit 77; }
done

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

REN=(-Dmalloc=ut_malloc -Dfree=ut_free -Dcalloc=ut_calloc -Drealloc=ut_realloc
     -Dsbrk=ut_sbrk -Derrno=ut_errno -Dgetpagesize=ut_getpagesize)
# -O2 -fno-builtin as in the SDK build, so the code is compiled the way it is
# on the target (no memset/malloc folding).
for f in malloc sbrk calloc realloc chdir; do
  "$CC" -O2 -fno-builtin -w "${REN[@]}" -I "$TOP" -I "$LDGINC" -I "$MINT" \
        -c "$TOP/$f.c" -o "$W/$f.o" || exit 1
done
"$CC" -O2 -fno-builtin -Wall -I "$TOP" -I "$MINT" \
      -c "$HERE/libshare-host-layout.c" -o "$W/layout.o" || exit 1
"$CC" -O2 -fno-builtin -Wall -Wextra \
      -c "$HERE/libshare-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/libshare-host" || exit 1

"$W/libshare-host"
exit $?
