#!/usr/bin/env bash
# build-kernel.sh -- fresh release rpi4 kernel from `git archive $HEADREV` in a scratch tree with its own target dir.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; export SOURCE_DATE_EPOCH=1792368000
T="$S/tree"; mkdir -p "$T" "$S/bin" "$S/logs"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
(cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-rpi4.log" 2>&1 || { tail -30 "$S/logs/build-rpi4.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-head.img"; shasum -a 256 "$S/bin/kernel8-head.img"
