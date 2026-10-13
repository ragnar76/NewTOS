#!/usr/bin/env bash
# cm-kernels.sh <label> <scratch-root> -- the instrumented raspi4b kernels the
# crt0-main series needs, each from a scratch copy of the shared tree
# (make-scratch.sh; the instrumentation never touches the shared tree). Trimmed
# copy of scratch-kernels.sh (a13) / fs10-kernels.sh (fs10):
#   kernel8-crash-<L>   IKBD injector answers the crash prompt (strict S2ALLS)
#   kernel8-keys-<L>    KEYS02 injector + emucon-schedule.py (KBDVEC + EmuCON)
#   kernel8-tdkeys-<L>  KEYS02 injector + tdqed-keys.py (TeraDesk -> QED, F1)
# The kernel source is HEAD (this task changes no kernel code); the product
# kernel8-<L>.img comes from checks.sh.
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/../bin"
mk() { sh "$HERE/make-scratch.sh" "$R/$1"; }
bld() { # bld <dir> <out>
  (cd "$R/$1" && cargo xtask build --board rpi4 --release > "$R/$1-rpi4.log" 2>&1) || { tail -20 "$R/$1-rpi4.log"; exit 1; }
  cp "$R/$1/kernel8.img" "$BIN/$2"; echo "built $2"
}
mk crash; python3 "$HERE/inj-0c-apply-instrumentation.py" "$R/crash" "$BIN" > /dev/null
python3 "$HERE/uacc-instrumentation.py" "$R/crash" "$BIN" > /dev/null
python3 "$HERE/mprot-instrumentation.py" "$R/crash" "$BIN" > /dev/null
python3 "$HERE/e10-instrumentation.py" "$R/crash" "$BIN" > /dev/null
bld crash "kernel8-crash-$L.img"
mk keys; python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/keys" > /dev/null
python3 "$HERE/emucon-schedule.py" "$R/keys" > /dev/null
bld keys "kernel8-keys-$L.img"
mk tdkeys; python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/tdkeys" > /dev/null
python3 "$HERE/tdqed-keys.py" "$R/tdkeys" > /dev/null
bld tdkeys "kernel8-tdkeys-$L.img"
