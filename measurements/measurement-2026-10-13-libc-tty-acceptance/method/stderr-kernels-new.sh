#!/usr/bin/env bash
# stderr-kernels-new.sh <scratch-root> -- like stderr-kernels.sh but only the HEAD keys kernel with emucon-stderr-schedule.py
# (the pre-FS11 one is byte-copied from the previous acceptance run). Never applied to the shared tree.
set -euo pipefail
R="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; BIN="$HERE/../bin"
sh "$HERE/make-scratch.sh" "$R/ks-new"
python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/ks-new" > /dev/null
python3 "$HERE/emucon-stderr-schedule.py" "$R/ks-new"
(cd "$R/ks-new" && cargo xtask build --board rpi4 --release > "$R/ks-new.log" 2>&1) || { tail -20 "$R/ks-new.log"; exit 1; }
cp "$R/ks-new/kernel8.img" "$BIN/kernel8-keys-stderr-new.img"
