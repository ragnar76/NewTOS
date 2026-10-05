#!/usr/bin/env bash
#
# system-host.sh -- build and run the host harness for mintlib's system()
# (TASK/PORT10, TASK/PROC01): mintlib's own posix/system.c from the extracted
# tree -- unchanged since TASK/PROC01 removed the PORT10 overlay that had cut
# its POSIX branch -- compiled with the include path and DEFS of
# sdk/libc/build-libc.sh, its GEMDOS calls (osbind.h held back,
# system-host-gemdos.h forced in), spawnvp and _unx2dos routed into
# system-host.c's recorders; mintlib's own mintlib/ctype.c (minus its
# weak_alias lines) supplies the table isspace() reads.
#
# The /bin/sh + $SHELL probe (access, getenv) is routed into the harness,
# which answers "no shell": system() takes the TOS branch, whose behaviour is
# what the checks cover (the POSIX branch -- vfork, execve, waitpid,
# sigaction, sigprocmask -- runs on the target, measured by ARCH03 stage 2's
# VFORKT probe; here its calls are routed to stubs that count as failures if
# reached). The script also checks that no overlay replaces posix/system.c
# any more and that the object does carry the POSIX branch.
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
  echo "system-host: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
RES="$("$CC" -print-resource-dir)/include"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

MFLAGS=(-O2 -fno-builtin -nostdinc -fno-blocks -Wall -Wextra
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined
        -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
"$CC" "${MFLAGS[@]}" -D_MINT_OSBIND_H=1 -include "$HERE/system-host-gemdos.h" \
    -Dspawnvp=system_test_spawnvp -D_unx2dos=system_test_unx2dos \
    -D_ctype=system_test_ctype -I"$TOP/posix" \
    -Daccess=system_test_access -Dgetenv=system_test_getenv \
    -Dvfork=system_test_vfork -Dexecve=system_test_execve -Dwaitpid=system_test_waitpid \
    -Dsigaction=system_test_sigaction -Dsigprocmask=system_test_sigprocmask \
    -Dsigemptyset=system_test_sigemptyset -Dsigaddset=system_test_sigaddset \
    -D_exit=system_test__exit -Denviron=system_test_environ \
    -c "$TOP/posix/system.c" -o "$W/system.o" || exit 1
# mintlib's ctype.c verbatim except its weak_alias() lines (an ELF .weak
# directive the Mach-O host assembler rejects; the aliases are not used here).
sed '/^weak_alias/d' "$TOP/mintlib/ctype.c" > "$W/ctype.c"
"$CC" "${MFLAGS[@]}" -D_ctype=system_test_ctype -I"$TOP/mintlib" \
    -c "$W/ctype.c" -o "$W/ctype.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/system-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/system-host" || exit 1

fail=0
# mintlib's own file again: no overlay, and the object carries the POSIX
# branch (the renamed calls). nm prints Mach-O names with a leading
# underscore; strip it.
if [ -e "$LIBC/overlay/posix/system.c" ]; then
  echo "FAIL  sdk/libc/overlay/posix/system.c is back"; fail=$((fail+1))
else
  echo "PASS  no overlay replaces mintlib's posix/system.c"
fi
undef="$(nm -u "$W/system.o" | sed 's/^_//' | sort)"
echo "system.o references: $(echo $undef)"
missing=""
for s in vfork execve waitpid sigaction sigprocmask access getenv; do
  echo "$undef" | grep -qx "system_test_$s" || missing="$missing $s"
done
if [ -n "$missing" ]; then
  echo "FAIL  the POSIX branch is not in system.o:$missing"; fail=$((fail+1))
else
  echo "PASS  system.o carries the POSIX branch (vfork/execve/waitpid/sigaction/sigprocmask, access/getenv probe)"
fi

"$W/system-host"; rc=$?
[ $rc -eq 77 ] && exit 77
exit $((fail + rc))
