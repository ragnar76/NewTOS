#!/usr/bin/env bash
# port09-kernels.sh <label> <scratch-root> -- the two instrumented raspi4b
# kernels this run needs, from scratch copies of the shared tree (subset of
# scratch-kernels.sh, same recipes; the instrumentation never touches the
# shared tree; kernel source is unchanged by PORT09):
#   kernel8-crash-<L>   IKBD injector answers the crash prompt (strict S2ALLS:
#                       the denied children crash by design)
#   kernel8-tdkeys-<L>  injector presses F1 once, 5 s after the first EL0
#                       program (TeraDesk) runs: TeraDesk Pexecs the F1 app
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/../.." && pwd)"
BIN="$HERE/../bin"
mk() { sh "$HERE/make-scratch.sh" "$R/$1"; }
bld() { # bld <dir> <out>
  (cd "$R/$1" && cargo xtask build --board rpi4 --release > "$R/$1-rpi4.log" 2>&1) || { tail -20 "$R/$1-rpi4.log"; exit 1; }
  cp "$R/$1/kernel8.img" "$BIN/$2"; echo "built $2"
}
mk crash; python3 "$M/measurement-2026-10-05-arch03-0c/method/apply-instrumentation.py" "$R/crash" "$BIN" > /dev/null
python3 "$HERE/uacc-instrumentation.py" "$R/crash" "$BIN" > /dev/null
python3 "$HERE/mprot-instrumentation.py" "$R/crash" "$BIN" > /dev/null
python3 "$HERE/e10-instrumentation.py" "$R/crash" "$BIN" > /dev/null
bld crash "kernel8-crash-$L.img"
mk tdkeys; python3 "$M/measurement-2026-10-04-keys02/method/apply-instrumentation.py" "$R/tdkeys" > /dev/null
python3 "$HERE/tdqed-keys.py" "$R/tdkeys" > /dev/null
bld tdkeys "kernel8-tdkeys-$L.img"
