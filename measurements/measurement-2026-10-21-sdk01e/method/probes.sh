#!/usr/bin/env bash
# probes.sh <before|after> -- the per-symbol link probe (link-probe.sh = measurement-2026-10-20-sdk01a's method) over
# every global symbol of libm.a (linked with libm.a + libc.a [+ builtins]) and of libc.a (libc.a [+ builtins]).
# "before" ran against copies of the HEAD archives ($S/before/{libc,libm}.a, sha256 in RUNINFO) without the builtins
# archive; "after" against the same libc.a/libm.a (unchanged by SDK01 E) plus sdk/compiler-rt/libclang_rt.builtins.a.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; cd "$WS"
case ${1:?} in
  before) bash "$HERE/link-probe.sh" "$S/probe-before-libm" - "$S/before/libm.a" "$S/before/libc.a"
          bash "$HERE/link-probe.sh" "$S/probe-before-libc" - "$S/before/libc.a" ;;
  after)  B=sdk/compiler-rt/libclang_rt.builtins.a
          bash "$HERE/link-probe.sh" "$S/probe-after-libm" - sdk/libm/libm.a sdk/libc/libc.a "$B"
          bash "$HERE/link-probe.sh" "$S/probe-after-libc" - sdk/libc/libc.a "$B" ;;
esac
