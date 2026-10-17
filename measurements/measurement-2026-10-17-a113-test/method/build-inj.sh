#!/usr/bin/env bash
# build-inj.sh <id> <green|head> <tag> ARM... -- scratch kernel (rpi4 release, QEMU raspi4b only) with the scripted IKBD injector:
# tools/inject-arm.py (T10/T16) applied to a scratch tree: green = rsync copy of the shared working tree (make-scratch.sh),
# head = git archive dc3388d1. ARM = <trigger>=<script-file>. -> $S/bin/kernel8-<id>-<side>.img
# The shared tree is never touched; the scratch trees keep their own target dir (incremental).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
id=$1; side=$2; tag=$3; shift 3
T="$S/trees/inj-$side"; export SOURCE_DATE_EPOCH=1792152000
if [ $side = green ]; then rsync -a --delete --exclude target --exclude .git --exclude measurements --exclude references --exclude Applications --exclude graphify-out --exclude hardware --exclude 'sd-card-backup-*' --exclude tosdump --exclude sdk/apps --exclude docs "$WS/" "$T/"
else mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +; (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"; fi
python3 "$WS/tools/inject-arm.py" "$T" --tag "$tag" "$@"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-$id-$side.log" 2>&1 || { tail -30 "$S/logs/build-$id-$side.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-$id-$side.img"; shasum -a 256 "$S/bin/kernel8-$id-$side.img"
