#!/usr/bin/env bash
# build-kernel-inj.sh <workdir> -- DESK43 U6a: the scratch key-injector kernel.
# Scratch copy of the shared tree (make-scratch.sh), inj-u6a.py applied to the
# COPY only, release rpi4 build; result -> ../bin/kernel8-inj.img (gitignored).
set -euo pipefail
W="${1:?workdir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
sh "$HERE/make-scratch.sh" "$W/tree"
python3 "$HERE/inj-u6a.py" "$W/tree"
(cd "$W/tree" && cargo xtask build --board rpi4 --release) > "$W/build-inj.log" 2>&1 \
  || { tail -30 "$W/build-inj.log"; exit 1; }
grep -E "generated [0-9]+ warning|^error" "$W/build-inj.log" || true
cp "$W/tree/kernel8.img" "$M/bin/kernel8-inj.img"
shasum -a 256 "$M/bin/kernel8-inj.img"
