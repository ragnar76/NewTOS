#!/usr/bin/env bash
# probes.sh <before|after> -- the per-symbol link probe (link-probe.sh = measurement-2026-10-20-sdk01a's method, copied
# unchanged) over every global symbol of libm.a (linked with libm.a + libc.a + builtins) and of libc.a (libc.a +
# builtins). "before" runs against copies of the archives as found at HEAD 04f81551 ($S/before/, sha256 in RUNINFO),
# "after" against the rebuilt sdk/libc/libc.a, sdk/libm/libm.a and the unchanged builtins archive.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; cd "$WS"
case ${1:?} in
  before) A=$S/before; bash "$HERE/link-probe.sh" "$S/probe-before-libm" - "$A/libm.a" "$A/libc.a" "$A/libclang_rt.builtins.a"
          bash "$HERE/link-probe.sh" "$S/probe-before-libc" - "$A/libc.a" "$A/libclang_rt.builtins.a" ;;
  after)  B=sdk/compiler-rt/libclang_rt.builtins.a
          bash "$HERE/link-probe.sh" "$S/probe-after-libm" - sdk/libm/libm.a sdk/libc/libc.a "$B"
          bash "$HERE/link-probe.sh" "$S/probe-after-libc" - sdk/libc/libc.a "$B" ;;
esac
for p in libm libc; do
  cp "$S/probe-${1}-$p/linkfail.txt" "$HERE/../checks/probe-${1}-$p-linkfail.txt"
  cp "$S/probe-${1}-$p/pub.txt" "$HERE/../checks/probe-${1}-$p-symbols.txt"
done
