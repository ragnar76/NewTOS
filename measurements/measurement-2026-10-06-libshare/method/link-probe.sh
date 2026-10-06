#!/usr/bin/env bash
# link-probe.sh <scratch dir> -- link the two probes like an LDG module
# (sdk/ldg/build-ldg-demo.sh `link module`: crt0, libshare.a FIRST in the
# group, libldg, gemlib, libm, libc, -Map) and run the libshare map check.
set -uo pipefail
S="${1:?scratch}"; mkdir -p "$S"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK="$(cd "$HERE/../../../sdk" && pwd)"
. "$SDK/libshare/libshare-link.sh"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
RES="$(clang -print-resource-dir)/include"
MINT="$SDK/libc/mintlib/include"
CF=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc -D__NEWTOS__ -Wno-macro-redefined)
clang "${CF[@]}" -I"$SDK/include" -I"$MINT" -c "$SDK/libc/crt0.S" -o "$S/crt0.o" || exit 1
for p in memalign-probe internal-probe; do
  clang "${CF[@]}" -I"$MINT" -isystem "$RES" -c "$HERE/$p.c" -o "$S/$p.o" || exit 1
  echo "=== $p"
  if "$BIN/rust-lld" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$S/crt0.o" "$S/$p.o" \
      --start-group "$SDK/libshare/libshare.a" "$SDK/ldg/libldg.a" "$SDK/gemlib/gemlib.a" "$SDK/libm/libm.a" "$SDK/libc/libc.a" --end-group \
      -Map="$S/$p.map" -o "$S/$p.NLD"; then
    echo "linker: OK"
  else
    echo "linker: FAILED"; continue
  fi
  echo "allocator members in the map:"
  grep -o '\(libshare\|libc\)\.a([^)]*\(malloc\|calloc\|realloc\|sbrk\|memalign\|valloc\|chdir\)[^)]*)' "$S/$p.map" | sort -u | sed 's/^/    /'
  echo "symbols:"
  "$BIN/llvm-nm" "$S/$p.NLD" | grep -E ' (malloc|free|calloc|realloc|sbrk|chdir|memalign|posix_memalign|aligned_alloc|valloc|__malloc|_mchunk_free_list|libshare_mchunk_free_list|static_mblock)$' | sed 's/^/    /'
  if ls_check_module_map "$S/$p.map" "$p"; then echo "map check: PASS"; else echo "map check: FAIL (build would stop)"; fi
done
