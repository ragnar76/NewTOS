#!/usr/bin/env bash
# build-plain.sh green|head -- fresh release rpi4 build in a scratch tree (own target dir), no instrumentation.
# green = rsync copy of the shared working tree (make-scratch.sh); head = git archive dc3388d1. -> $S/bin/kernel8-plain-<id>.img
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
id=$1; T="$S/trees/plain-$id"; export SOURCE_DATE_EPOCH=1792152000
if [ $id = green ]; then bash "$HERE/make-scratch.sh" "$T"; else rm -rf "$T"; mkdir -p "$T"; (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"; fi
(cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-plain-$id.log" 2>&1 || { tail -30 "$S/logs/build-plain-$id.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-plain-$id.img"; shasum -a 256 "$S/bin/kernel8-plain-$id.img"
