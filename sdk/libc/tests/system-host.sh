#!/usr/bin/env bash
#
# system-host.sh -- build and run the host harness for the system() overlay
# (TASK/PORT10): sdk/libc/overlay/posix/system.c, compiled with the include
# path and DEFS of sdk/libc/build-libc.sh, its GEMDOS calls (osbind.h held
# back, system-host-gemdos.h forced in), spawnvp and _unx2dos routed into
# system-host.c's recorders; mintlib's own mintlib/ctype.c (minus its
# weak_alias lines) supplies the table isspace() reads.
#
# Also checks the overlay object itself: none of the POSIX branch's symbols
# (vfork, execve, waitpid, sigaction, sigprocmask) nor its /bin/sh + $SHELL
# probe (access, getenv) may be referenced any more.
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
    -c "$LIBC/overlay/posix/system.c" -o "$W/system.o" || exit 1
# mintlib's ctype.c verbatim except its weak_alias() lines (an ELF .weak
# directive the Mach-O host assembler rejects; the aliases are not used here).
sed '/^weak_alias/d' "$TOP/mintlib/ctype.c" > "$W/ctype.c"
"$CC" "${MFLAGS[@]}" -D_ctype=system_test_ctype -I"$TOP/mintlib" \
    -c "$W/ctype.c" -o "$W/ctype.o" || exit 1
"$CC" -O2 -Wall -Wextra -c "$HERE/system-host.c" -o "$W/harness.o" || exit 1
"$CC" "$W"/*.o -o "$W/system-host" || exit 1

fail=0
# The POSIX branch is gone: the overlay object references none of it.
# (nm prints Mach-O names with a leading underscore; strip it.)
undef="$(nm -u "$W/system.o" | sed 's/^_//' | sort)"
echo "overlay object references: $(echo $undef)"
bad=""
for s in vfork execve waitpid sigaction sigprocmask access getenv; do
  echo "$undef" | grep -qx "$s" && bad="$bad $s"
done
if [ -n "$bad" ]; then
  echo "FAIL  overlay still references the POSIX branch:$bad"; fail=$((fail+1))
else
  echo "PASS  no vfork/execve/waitpid/sigaction/sigprocmask/access/getenv referenced"
fi

"$W/system-host"; rc=$?
[ $rc -eq 77 ] && exit 77
exit $((fail + rc))
