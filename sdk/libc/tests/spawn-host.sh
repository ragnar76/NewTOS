#!/usr/bin/env bash
#
# spawn-host.sh -- build and run the host harness for the LP64 spawn*
# overlays (TASK/PORT10): sdk/libc/overlay/mintlib/spawnve.c + spawnvp.c,
# compiled with the include path and DEFS of sdk/libc/build-libc.sh, their
# four outside references renamed into spawn-host.c's recorders. Then, as a
# second binary, the program-name search: spawn-search-host.c (see there).
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

"$W/spawn-host"; rc=$?
[ $rc -eq 77 ] && exit 77

# --- the program-name search (user decision 2026-10-02) ----------------------
# spawn-search-host: spawnvp's { ntp, nap, ncp } through mintlib's REAL
# _buffindfile (mintlib/buffindf.c, unmodified), and posix/execvp.c's
# { "", .ntp, .nap, .ncp }, over the fake FAT directory in spawn-search-fs.c.
# Only _spawnve/getenv/environ (recorders) and __execve/__mempcpy are renamed.
S="$W/search"; mkdir -p "$S"
SFLAGS=(-O2 -fno-builtin -nostdinc -fno-blocks -Wall -Wextra
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        -D_spawnve=spawn_test__spawnve -Dgetenv=spawn_test_getenv
        -Denviron=spawn_test_environ
        -D__execve=spawn_test_execve -D__mempcpy=spawn_test_mempcpy
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
for f in mintlib/spawnve mintlib/spawnvp posix/execvp; do
  "$CC" "${SFLAGS[@]}" -I"$TOP/$(dirname $f)" -c "$LIBC/overlay/$f.c" \
      -o "$S/$(basename $f).o" || exit 1
done
"$CC" "${SFLAGS[@]}" -c "$TOP/mintlib/buffindf.c" -o "$S/buffindf.o" || exit 1
"$CC" "${SFLAGS[@]}" -c "$HERE/spawn-search-fs.c" -o "$S/fs.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/spawn-search-host.c" -o "$S/harness.o" || exit 1
"$CC" "$S"/*.o -o "$S/spawn-search-host" || exit 1
"$S/spawn-search-host"; rc2=$?
[ $rc2 -eq 77 ] && exit 77

# mintlib/spawn.c's copy of the list (#! interpreter lookup, reached only
# after a Pexec ENOEXEC) is not driven; its compiled overlay object must carry
# the native extension strings and none of the m68k ones.
fail=0
if "$CC" "${SFLAGS[@]}" -I"$TOP/mintlib" -c "$LIBC/overlay/mintlib/spawn.c" \
     -o "$S/spawn-c.o" 2>"$S/spawn-c.err"; then
  strs="$(strings -a -n 3 "$S/spawn-c.o" | grep -xE 'ntp|nap|ncp|ttp|prg|tos' | sort | tr '\n' ' ')"
  if [ "$strs" = "nap ncp ntp " ]; then
    echo "PASS  overlay mintlib/spawn.c object: extension strings = { $strs}"
  else
    echo "FAIL  overlay mintlib/spawn.c object: extension strings = { $strs}"; fail=1
  fi
else
  echo "FAIL  overlay mintlib/spawn.c does not compile on the host:"
  sed 's/^/      /' "$S/spawn-c.err"; fail=1
fi
exit $((rc + rc2 + fail))
