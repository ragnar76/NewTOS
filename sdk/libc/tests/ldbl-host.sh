#!/bin/bash
#
# ldbl-host.sh -- host test of `long double' (IEEE binary128) in the SDK's
# libc and libm (TASK/SDK01 J). Run by sdk/run-host-tests.sh; exit 0 = pass,
# 77 = skip, else fail.
#
#   1. ldbl-elf.c links with sdk/bin/newtos-cc -lm (crt0 + libm.a + libc.a +
#      compiler-rt builtins), 0 undefined symbols. The link records which
#      archive members it pulled (--why-extract).
#   2. Rebuild parity: every pulled member is compiled again from its source
#      with the command that built it -- libc.a: the line of
#      sdk/libc/build/compile-commands.txt (written by build-libc.sh);
#      libm.a and the builtins: the flags of build-libm.sh /
#      build-compiler-rt.sh, as sdk/compiler-rt/tests/rt-host.sh does -- and
#      must be byte-identical to the shipped member.
#   3. The same commands with -ffixed-x18 (macOS does not preserve x18; see
#      rt-host.c) build the objects that are linked, with ldbl-elf.c, into the
#      program the host runs: newtos.ld with the data segment on a 16 KiB page.
#   4. ldbl-oracle.py drives ldbl-host (which runs that AArch64 code on the
#      host CPU) and judges printf/scanf/strtold/q*cvt/classification/libm
#      results against exact rational arithmetic.
#
# Skips when an archive or the extracted trees are missing, clang or python3
# is missing, or the host is not AArch64. LDBL_KEEP=<dir> keeps ldbl-host and
# the program it runs in <dir>.
set -u

SDK=$(cd "$(dirname "$0")/../.." && pwd)
HERE="$SDK/libc/tests"
CC=${CC:-cc}
CLANG=${CLANG:-clang}

command -v "$CLANG" >/dev/null 2>&1 || { echo "skip: clang not found"; exit 77; }
command -v python3 >/dev/null 2>&1 || { echo "skip: python3 not found (the oracle)"; exit 77; }
case $(uname -m) in arm64|aarch64) ;; *) echo "skip: host is not AArch64"; exit 77 ;; esac
for a in libc/libc.a libm/libm.a compiler-rt/libclang_rt.builtins.a; do
  [ -f "$SDK/$a" ] || { echo "skip: sdk/$a not built"; exit 77; }
done
CRT_TOP="$SDK/compiler-rt/compiler-rt-21.1.8.src"
OL_TOP="$SDK/libm/openlibm-0.8.8"
MINT="$SDK/libc/mintlib/include"
CMDS="$SDK/libc/build/compile-commands.txt"
for f in "$CMDS" "$SDK/compiler-rt/build/srcs.txt" "$CRT_TOP/lib/builtins" "$OL_TOP/src" "$MINT"; do
  [ -e "$f" ] || { echo "skip: $f missing (run the library's build script)"; exit 77; }
done
. "$SDK/toolchain.sh"
BIN="$(newtos_rust_bindir)"
NM="$BIN/llvm-nm"; AR="$BIN/llvm-ar"; LLD="$BIN/rust-lld"; OBJDUMP="$BIN/llvm-objdump"
for t in "$NM" "$AR" "$LLD" "$OBJDUMP"; do [ -x "$t" ] || { echo "skip: $t missing"; exit 77; }; done
RES="$("$CLANG" -print-resource-dir)/include"

WORK=$(mktemp -d "${TMPDIR:-/tmp}/ldbl-host.XXXXXX") || exit 1
trap 'rm -rf "$WORK"' EXIT
fail=0

# --- 1. the newtos-cc link --------------------------------------------------------
"$SDK/bin/newtos-cc" -lm -ffixed-x18 -c "$HERE/ldbl-elf.c" -o "$WORK/elf.o" || { echo "FAIL: ldbl-elf.c does not compile"; exit 1; }
"$SDK/bin/newtos-cc" -lm -Wl,--why-extract="$WORK/why.txt" "$WORK/elf.o" -o "$WORK/LDBL.NCP" ||
  { echo "FAIL: ldbl-elf does not link with newtos-cc -lm"; exit 1; }
n_und=$("$NM" -u "$WORK/LDBL.NCP" | grep -c .)
echo "1  newtos-cc -lm: LDBL.NCP linked ($(wc -c < "$WORK/LDBL.NCP" | tr -d ' ') bytes), undefined symbols: $n_und"
[ "$n_und" -eq 0 ] || fail=1
# "<archive>(<member>)" of every extracted member
awk -F'\t' 'NR > 1 {print $2}' "$WORK/why.txt" | grep '\.a(' | sort -u > "$WORK/members.txt"

# --- 2. rebuild parity + 3. the -ffixed-x18 objects -------------------------------
CRT_FLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc
           -std=c11 -fvisibility=hidden -DVISIBILITY_HIDDEN -fomit-frame-pointer -DCOMPILER_RT_HAS_FLOAT16
           -I"$CRT_TOP/lib/builtins" -isystem "$RES")
OL_FLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc
          -std=c99 -fno-strict-aliasing -D__BSD_VISIBLE
          -I"$OL_TOP" -I"$OL_TOP/include" -I"$OL_TOP/aarch64" -I"$OL_TOP/src" -I"$OL_TOP/ld128" -I"$MINT" -isystem "$RES")
mkdir -p "$WORK/ship" "$WORK/same" "$WORK/x18"
nsame=0; ndiff=0; nmem=0
while IFS= read -r am; do
  arch="${am%%(*}"; mem="${am#*(}"; mem="${mem%)}"
  nmem=$((nmem+1))
  (cd "$WORK/ship" && "$AR" x "$arch" "$mem") || { echo "FAIL: cannot extract $am"; fail=1; continue; }
  case "$(basename "$arch")" in
    libc.a)
      line=$(grep -F -- "-o $SDK/libc/build/obj/$mem " "$CMDS" | tail -1)
      [ -n "$line" ] || { echo "FAIL: no compile command for libc.a($mem) in $CMDS"; fail=1; continue; }
      eval "cmd=($line)"
      for i in "${!cmd[@]}"; do [ "${cmd[$i]}" = "-o" ] && oi=$((i+1)); done
      cmd[$oi]="$WORK/same/$mem"; "${cmd[@]}" 2>/dev/null || { echo "FAIL: rebuild of libc.a($mem)"; fail=1; continue; }
      cmd[$oi]="$WORK/x18/$mem"; "${cmd[@]}" -ffixed-x18 2>/dev/null || { echo "FAIL: x18 rebuild of libc.a($mem)"; fail=1; continue; } ;;
    libm.a)
      d="${mem%%__*}"; b="${mem#*__}"; src="$OL_TOP/$d/${b%.o}.c"
      "$CLANG" "${OL_FLAGS[@]}" -c "$src" -o "$WORK/same/$mem" 2>/dev/null &&
      "$CLANG" "${OL_FLAGS[@]}" -ffixed-x18 -c "$src" -o "$WORK/x18/$mem" 2>/dev/null ||
        { echo "FAIL: rebuild of libm.a($mem)"; fail=1; continue; } ;;
    libclang_rt.builtins.a)
      src=$(while IFS= read -r c; do [ "$(printf '%s' "${c%.c}" | tr '/' '_').o" = "$mem" ] && echo "$c"; done < "$SDK/compiler-rt/build/srcs.txt")
      [ -n "$src" ] || { echo "FAIL: no source for builtins($mem)"; fail=1; continue; }
      "$CLANG" "${CRT_FLAGS[@]}" -c "$CRT_TOP/$src" -o "$WORK/same/$mem" &&
      "$CLANG" "${CRT_FLAGS[@]}" -ffixed-x18 -c "$CRT_TOP/$src" -o "$WORK/x18/$mem" ||
        { echo "FAIL: rebuild of builtins($mem)"; fail=1; continue; } ;;
    *) echo "FAIL: member of an unexpected archive: $am"; fail=1; continue ;;
  esac
  if cmp -s "$WORK/same/$mem" "$WORK/ship/$mem"; then nsame=$((nsame+1)); else ndiff=$((ndiff+1)); echo "FAIL: rebuild of $am differs from the shipped member"; fi
done < "$WORK/members.txt"
echo "2  rebuild parity: $nsame of $nmem linked archive members byte-identical, $ndiff differ"
[ "$ndiff" -eq 0 ] && [ "$nsame" -eq "$nmem" ] || fail=1
n18=$("$OBJDUMP" -d "$WORK"/x18/*.o "$WORK/elf.o" | grep -E '^ +[0-9a-f]+:' | grep -cE '\b[xw]18\b')
[ "$n18" -eq 0 ] || { echo "FAIL: the -ffixed-x18 objects still use x18 ($n18 instructions)"; fail=1; }

sed 's/^\(    \)\.init_array : {/\1. = ALIGN(0x4000);\
\1.init_array : {/' "$SDK/newtos.ld" > "$WORK/host.ld"
grep -q 'ALIGN(0x4000)' "$WORK/host.ld" || { echo "FAIL: could not derive host.ld from newtos.ld"; exit 1; }
"$SDK/bin/newtos-cc" -c "$SDK/libc/crt0.S" -o "$WORK/crt0.o" || exit 1
"$LLD" -flavor gnu -T "$WORK/host.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
  "$WORK/crt0.o" "$WORK/elf.o" "$WORK"/x18/*.o -o "$WORK/LDBL-x18.elf" ||
  { echo "FAIL: x18 test program does not link from the rebuilt members alone"; exit 1; }
echo "3  -ffixed-x18 rebuild of the $nmem members: $n18 x18 instructions; test program linked with host.ld"

# --- 4. run + judge ---------------------------------------------------------------
"$CC" -O1 -Wall -Wextra -Werror -o "$WORK/ldbl-host" "$HERE/ldbl-host.c" || { echo "FAIL: ldbl-host.c does not build"; exit 1; }
# LDBL_KEEP=<dir>: keep the executor and the program (measurement scripts query them)
[ -n "${LDBL_KEEP:-}" ] && mkdir -p "$LDBL_KEEP" && cp "$WORK/ldbl-host" "$WORK/LDBL-x18.elf" "$LDBL_KEEP/"
python3 "$HERE/ldbl-oracle.py" "$WORK/ldbl-host" "$WORK/LDBL-x18.elf"; rc=$?
[ $rc -eq 77 ] && exit 77
[ $rc -eq 0 ] || fail=1
exit $fail
