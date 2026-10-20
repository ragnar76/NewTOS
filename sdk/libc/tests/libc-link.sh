#!/usr/bin/env bash
#
# libc-link.sh -- link checks against the built sdk/libc/libc.a (TASK/SDK01 A,
# docs/analysis/sdk-inventory-2026-10-20.md W1, W3, W6, W7).
#
#  1. One program per function, crt0 + newtos.ld + libc.a, exactly as an
#     application links: each of these must link (they did not before SDK01 A:
#     the pwdgrp/ family, __sys_fstat, the .stabs files, NatFeats).
#  2. A program WITHOUT main must fail with "undefined symbol: main" -- libc.a
#     carries no test program's or host generator's main any more
#     (unix/ucontext.c, stdlib/dtest.c, tz/date.c, tz/tzinit.c, syscall/*).
#  3. No archive member references the host generator's yyin/yyparse.
#
# Nothing is run; host-independent (clang's aarch64 target + rust-lld).
# Exit status: number of failed checks; 77 = skipped (libc.a not built).
# LIBC_A=<archive> checks another archive (red run against an older libc.a).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
SDK="$(cd "$LIBC/.." && pwd)"
TOP="$LIBC/mintlib"
CLANG="${CLANG:-clang}"
LIBA="${LIBC_A:-$LIBC/libc.a}"
[ -f "$LIBA" ] && [ -f "$TOP/include/stdint.h" ] || {
  echo "libc-link: skipped -- libc.a or mintlib tree missing; run build-libc.sh"
  exit 77
}
. "$SDK/toolchain.sh"   # rustc asked from inside sdk/, whatever the cwd (SDK01 I)
BIN="$(newtos_rust_bindir)"
LLD="$BIN/rust-lld"; NM="$BIN/llvm-nm"
[ -x "$LLD" ] && [ -x "$NM" ] || { echo "libc-link: skipped -- rust-lld/llvm-nm missing"; exit 77; }
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
CF=(-target aarch64-unknown-none-elf -ffreestanding -fPIE -nostdinc -fno-builtin
    -I"$SDK/include" -I"$TOP/include" -I"$TOP/mintlib" -isystem "$("$CLANG" -print-resource-dir)/include")
"$CLANG" "${CF[@]}" -c "$LIBC/crt0.S" -o "$W/crt0.o" || { echo "libc-link: crt0 did not build"; exit 1; }
lnk() { "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z max-page-size=0x1000 \
          --build-id=none "$W/crt0.o" "$@" "$LIBA" -o "$W/a.elf" 2> "$W/err"; }

fail=0; n=0
for f in tmpfile mkstemp mktemp mkdtemp tmpnam tmpnam_r tempnam glob globfree \
         wordexp wordfree getlogin cuserid getpwuid getgrent sem_open sem_unlink \
         ttyname ttyname_r ctermid gets getwd siggetmask \
         nf_init nf_get_id nf_version nf_debug nf_debugprintf nf_get_name nf_shutdown nf_exec; do
  n=$((n+1))
  printf 'extern char %s; void *volatile ref = (void *)&%s; int main(void) { return 0; }\n' "$f" "$f" > "$W/r.c"
  "$CLANG" "${CF[@]}" -c "$W/r.c" -o "$W/r.o" && lnk "$W/r.o" && continue
  echo "FAIL  $f does not link: $(grep -oE 'undefined symbol: [^ ]+|duplicate symbol: [^ ]+' "$W/err" | sort -u | tr '\n' ' ')"
  fail=$((fail+1))
done
echo "libc-link: $((n-fail))/$n functions link"

printf 'int not_main(void) { return 0; }\n' > "$W/nomain.c"
"$CLANG" "${CF[@]}" -c "$W/nomain.c" -o "$W/nomain.o"
if lnk "$W/nomain.o"; then
  echo "FAIL  a program without main LINKED -- libc.a defines main: $("$NM" -A -g --defined-only "$LIBA" 2>/dev/null | grep ' T main$' | sed 's/.*libc.a://' | tr '\n' ' ')"
  fail=$((fail+1))
elif grep -q "undefined symbol: main" "$W/err"; then
  echo "libc-link: program without main -> undefined symbol: main"
else
  echo "FAIL  program without main failed for another reason:"; sed 's/^/      /' "$W/err" | head -5
  fail=$((fail+1))
fi

"$NM" -u "$LIBA" > "$W/undef" 2>/dev/null
if grep -qE '^ +U (yyin|yyparse)$' "$W/undef"; then
  echo "FAIL  libc.a still references the syscall generator's yyin/yyparse"
  fail=$((fail+1))
else
  echo "libc-link: no yyin/yyparse references in libc.a"
fi
exit $fail
