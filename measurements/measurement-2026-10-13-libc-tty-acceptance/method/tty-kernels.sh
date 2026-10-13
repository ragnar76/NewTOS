#!/usr/bin/env bash
# tty-kernels.sh <label> <scratch-root> -- the instrumented "keys" kernel for
# the probe's T4: KEYS02 injector (inj-keys02-apply-instrumentation.py,
# unchanged copy) + the LIBC-TTY key schedule (tty-schedule.py), from a scratch
# copy of the shared tree (make-scratch.sh). Copy of
# measurement-2026-10-13-fs11/method/fs11-kernels.sh without the git-rev branch.
# Never applied to the shared tree.
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/../bin"
D="$R/keys-$L"
sh "$HERE/make-scratch.sh" "$D"
python3 "$HERE/inj-keys02-apply-instrumentation.py" "$D" > /dev/null
python3 "$HERE/tty-schedule.py" "$D"
(cd "$D" && cargo xtask build --board rpi4 --release > "$R/keys-$L-rpi4.log" 2>&1) || { tail -20 "$R/keys-$L-rpi4.log"; exit 1; }
cp "$D/kernel8.img" "$BIN/kernel8-keys-$L.img"
echo "built kernel8-keys-$L.img"
