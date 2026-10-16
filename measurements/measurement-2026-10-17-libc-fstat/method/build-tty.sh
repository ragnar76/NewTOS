#!/usr/bin/env bash
# (LIBC-FSTAT: copied unchanged from measurement-2026-10-13-mem05-acceptance/method, with tests/ and crt0-before-libc-main.S.)
#!/usr/bin/env bash
# build-tty.sh <outdir> -- build the LIBC-TTY probe (tests/ttyprobe.c) as an
# LP64 PIE libc program with the libc toolchain path of build-fs11.sh (copied
# from measurement-2026-10-13-fs11-aux-acceptance/method, trimmed):
#   TTYPROBE.NAP  T1-T3 (isatty, file handles, Fforce'd child) + T4 (keys:
#                 getchar/fgets with the flags as mintlib leaves them)
#   TTYA.NAP      T1-T3 only (-DTTY_NO_KEYS), for the product kernel runs; its
#                 child is C:\TTYPROBE.NAP (same child code)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-tty.sh <outdir>}"
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
. "$SDK/lib-stale.sh"
lib_ensure gemlib || exit 1
MINT="$LIBC/mintlib/include"
CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os
        -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$HERE/tests")
W="$(mktemp -d "$OUT/w.XXXX")"
build() { # build <src> <OUT> [cflags...]
  local src="$1" out="$2"; shift 2
  "$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "$@" -c "$HERE/tests/$src.c" -o "$W/$src.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$W/crt0.o" "$W/$src.o" \
      --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$out" || exit 1
  echo "built $OUT/$out ($(wc -c < "$OUT/$out" | tr -d ' ') bytes)"
}
build ttyprobe TTYPROBE.NAP
build ttyprobe TTYA.NAP -DTTY_NO_KEYS
rm -rf "$W"
