#!/usr/bin/env bash
# build-kernel.sh -- one fresh release rpi4 kernel from `git archive $HEADREV` in a scratch tree with its own target dir.
# SDK01 A changes no kernel/ or tos-api/ file (checked below), so the product kernel of HEAD is the kernel of this run.
# (Reduced from measurement-2026-10-18-proc04-fdelete/method/build-kernels.sh: side wt == head, kernel "plain" only.)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; export SOURCE_DATE_EPOCH=1792152000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"
K="$( (git diff --name-only $HEADREV -- kernel tos-api tos-types; git ls-files --others --exclude-standard -- kernel tos-api tos-types) | sort -u)"
[ -z "$K" ] || { echo "kernel-side working-tree changes present -- this script builds HEAD only:"; echo "$K"; exit 1; }
T="$S/trees/plain"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
git archive $HEADREV | tar -x -C "$T"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-kernel.log" 2>&1 || { tail -30 "$S/logs/build-kernel.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-plain.img"; shasum -a 256 "$S/bin/kernel8-plain.img"
