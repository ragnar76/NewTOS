#!/usr/bin/env bash
#
# lp64-limits.sh -- compile lp64-limits.c for the NewTOS TARGET with the
# include path and DEFS of sdk/libc/build-libc.sh (TASK/SDK01 A). The file is
# all _Static_assert, so "compiles" == "every limit matches its LP64 type".
# Host-independent: only clang's aarch64 target is used, nothing is run.
#
# Exit status: 0 = all limits right; 1 = a limit is wrong (clang's diagnostic
# names it); 77 = skipped (mintlib tree not extracted). LIMITS_HEADER_DIR=<dir>
# puts <dir> in front of the include path (red runs with pristine headers).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk/libc/tests
LIBC="$(cd "$HERE/.." && pwd)"                                 # .../sdk/libc
TOP="$LIBC/mintlib"
CLANG="${CLANG:-clang}"
[ -f "$TOP/include/stdint.h" ] || {
  echo "lp64-limits: skipped -- mintlib tree missing ($TOP); run build-libc.sh"
  exit 77
}
RES="$("$CLANG" -print-resource-dir)/include"
PRE=(); [ -n "${LIMITS_HEADER_DIR:-}" ] && PRE=(-I"$LIMITS_HEADER_DIR")
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT

if "$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 \
     -fno-builtin -nostdinc \
     -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH \
     -Wno-macro-redefined ${PRE[@]+"${PRE[@]}"} \
     -I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES" \
     -c "$HERE/lp64-limits.c" -o "$W/lp64-limits.o" 2> "$W/err"; then
  echo "lp64-limits: all limits match their AArch64/LP64 types"
  exit 0
fi
grep -E "static assertion failed|error:" "$W/err" | sed 's/^/  /'
exit 1
