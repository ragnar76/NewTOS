# vendor-lib.sh -- the steps shared by the build scripts of the vendored
# third-party libraries that zView's codecs need (TASK/LDG-M2): zlib, libpng,
# libjpeg (IJG), giflib, libexif. Sourced, not executed.
#
# Each of those libraries is built the way the other ported libraries are
# (sdk/ldg/build-ldg.sh is the model): a PRISTINE upstream release tarball,
# pinned by SHA-256 in <lib>-sources.sha256 next to it, is extracted, our
# overlay/ (configuration headers only -- the library sources stay
# unmodified) is copied over it, the library's OWN source list is compiled
# with the SDK's freestanding AArch64 flags against mintlib's headers, and
# the objects are archived into a static library. What differs per library
# (source list, defines, include dirs, which upstream file becomes the config
# header) stays in the library's build script; what is the same for all five
# lives here once, so the five scripts cannot drift apart.
#
# The dependency/staleness declaration for these libraries is in
# sdk/lib-stale.sh like for every other library; this file is one of the
# inputs it watches for each of them.
#
# Functions (all return non-zero on failure; callers run with `set -uo
# pipefail` and `|| exit 1`):
#   vl_toolchain                  CLANG, RES, AR, NM; prints the versions
#   vl_extract <tarball> <sums> <top> <inner> <refresh>
#                                 verify the tarball against <sums>, extract
#                                 its top directory <inner> as <top> when
#                                 <refresh>=1, <top> is absent, or the tarball
#                                 is newer than <top> (PORT03: a version swap
#                                 must not leave the old sources compiled)
#   vl_overlay <overlay> <top>    copy overlay/ over the extracted tree
#   vl_compile <top> <objdir> <src...>
#                                 compile each <src> (relative to <top>) with
#                                 VL_CFLAGS + the caller's LIB_CFLAGS/LIB_INCS
#                                 arrays; prints the "compiled OK=" block that
#                                 lib_ensure echoes; ANY failure stops the
#                                 build (there is no expected-failure list for
#                                 these libraries: all of them are portable C)
#   vl_archive <out> <objdir> <builddir>
#                                 archive, list defined + dangling symbols

VL_SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"       # .../sdk

# The SDK's freestanding set, identical to sdk/ldg/build-ldg.sh and the app
# builds (build-qed.sh): AArch64 ELF, no host headers, -fPIE because every
# NewTOS program and .NLD module is linked -pie (newtos.ld, R_AARCH64_RELATIVE
# only, -z text) and these archives are linked into them.
VL_CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
           -fPIE -fno-builtin -nostdinc)

vl_toolchain() {
  CLANG="${CLANG:-clang}"
  command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; return 1; }
  RES="$($CLANG -print-resource-dir)/include"
  local sysroot bin
  sysroot="$(rustc --print sysroot 2>/dev/null)"
  bin="$sysroot/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
  AR="${AR:-$bin/llvm-ar}"
  NM="$bin/llvm-nm"
  [ -x "$AR" ] || { echo "error: missing LLVM tool: $AR" >&2; return 1; }
  [ -x "$NM" ] || { echo "error: missing LLVM tool: $NM" >&2; return 1; }
  echo "clang : $($CLANG --version | head -1)"
  echo "ar    : $AR"
  echo
  return 0
}

vl_extract() {
  local tarball=$1 sums=$2 top=$3 inner=$4 refresh=$5 want have tmp
  if [ "$refresh" = "1" ] || [ ! -d "$top" ] || [ "$tarball" -nt "$top" ]; then
    [ -f "$tarball" ] || { echo "error: pristine tarball $tarball missing" >&2; return 1; }
    want="$(sed -e 's/#.*//' "$sums" | awk -v f="$(basename "$tarball")" '$2==f{print $1}')"
    have="$(shasum -a 256 "$tarball" | awk '{print $1}')"
    if [ -z "$want" ] || [ "$want" != "$have" ]; then
      echo "error: $(basename "$tarball") SHA-256 $have does not match $(basename "$sums") ($want)" >&2
      return 1
    fi
    echo "extracting pristine $inner from $(basename "$tarball") (sha256 verified)…"
    rm -rf "$top"
    tmp="$(mktemp -d)"
    tar xf "$tarball" -C "$tmp" || { rm -rf "$tmp"; return 1; }
    [ -d "$tmp/$inner" ] || { echo "error: $(basename "$tarball") has no top directory $inner" >&2; rm -rf "$tmp"; return 1; }
    mv "$tmp/$inner" "$top"
    rm -rf "$tmp"
    touch "$top"
  fi
  return 0
}

vl_overlay() {
  local overlay=$1 top=$2
  [ -d "$overlay" ] || return 0
  ( cd "$overlay" && find . -type f ! -name '._*' ! -name '.DS_Store' -print0 | while IFS= read -r -d '' f; do
      mkdir -p "$top/$(dirname "$f")"
      cp "$f" "$top/$f"
      echo "  overlay: ${f#./}"
    done )
}

vl_compile() {
  local top=$1 objdir=$2; shift 2
  local c o base ok=0 fail=0 warn=0
  local build; build="$(dirname "$objdir")"
  mkdir -p "$objdir"
  rm -f "$objdir"/*.o "$objdir"/*.err
  : > "$build/compiled.txt"; : > "$build/failed.txt"
  for c in "$@"; do
    base="$(printf '%s' "${c%.c}" | tr '/' '_')"
    o="$objdir/$base.o"
    if "$CLANG" "${VL_CFLAGS[@]}" ${LIB_CFLAGS[@]+"${LIB_CFLAGS[@]}"} \
         ${LIB_INCS[@]+"${LIB_INCS[@]}"} -isystem "$RES" \
         -c "$top/$c" -o "$o" 2>"$objdir/$base.err"; then
      echo "$c" >> "$build/compiled.txt"; ok=$((ok+1))
      if [ -s "$objdir/$base.err" ]; then
        warn=$((warn+1)); echo "  warnings in $c:"; sed 's/^/      /' "$objdir/$base.err"
      fi
    else
      echo "$c" >> "$build/failed.txt"; fail=$((fail+1)); rm -f "$o"
    fi
  done
  echo "compiled OK=$ok  FAILED=$fail  (files with warnings: $warn)"
  echo
  if [ "$fail" -ne 0 ]; then
    echo "STOP: sources that did not compile (fix the configuration overlay or the toolchain):"
    while IFS= read -r c; do
      echo "  === $c ==="
      sed 's/^/      /' "$objdir/$(printf '%s' "${c%.c}" | tr '/' '_').err" | head -20
    done < "$build/failed.txt"
    return 1
  fi
  return 0
}

vl_archive() {
  local out=$1 objdir=$2 build=$3
  rm -f "$out"
  "$AR" cr "$out" "$objdir"/*.o || return 1
  echo "archived -> $out ($(wc -c < "$out" | tr -d ' ') bytes)"
  "$NM" --defined-only -g "$out" 2>/dev/null | awk 'NF>=3 && $1 !~ /:$/ {print $NF}' | sort -u > "$build/defined.txt"
  "$NM" -u "$out" 2>/dev/null | awk 'NF>=2 && $1 !~ /:$/ && $NF !~ /:$/ {print $NF}' | sort -u > "$build/undefined.txt"
  comm -23 "$build/undefined.txt" "$build/defined.txt" > "$build/external.txt"
  echo "defined symbols: $(wc -l < "$build/defined.txt" | tr -d ' ')  external references: $(wc -l < "$build/external.txt" | tr -d ' ') (build/external.txt: libc/libm/other-library symbols the archive needs)"
  return 0
}
