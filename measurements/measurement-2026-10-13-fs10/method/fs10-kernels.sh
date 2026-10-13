#!/usr/bin/env bash
# fs10-kernels.sh <label> <scratch-root> -- the one instrumented kernel the FS10
# regression sample needs: kernel8-keys-<L>.img (KBDVEC + EmuCON session, the
# KEYS02 injector + emucon-schedule.py), built from a scratch copy of the
# shared tree (make-scratch.sh; never applied to the shared tree). Copied and
# trimmed from measurement-2026-10-13-arch03-acceptance/method/scratch-kernels.sh.
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/../bin"
sh "$HERE/make-scratch.sh" "$R/keys"
python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/keys" > /dev/null
python3 "$HERE/emucon-schedule.py" "$R/keys" > /dev/null
(cd "$R/keys" && cargo xtask build --board rpi4 --release > "$R/keys-rpi4.log" 2>&1) || { tail -20 "$R/keys-rpi4.log"; exit 1; }
cp "$R/keys/kernel8.img" "$BIN/kernel8-keys-$L.img"
echo "built kernel8-keys-$L.img"
