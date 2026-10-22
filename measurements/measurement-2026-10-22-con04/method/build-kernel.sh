#!/usr/bin/env bash
# build-kernel.sh <side> -- fresh release rpi4 kernel from `git archive $HEADREV` into a scratch tree with its own target dir.
#   side=head : the archive as is (before CON04)
#   side=wt   : the archive + the uncommitted kernel/ tos-api/ files of the working tree (CON04)
# Pattern of measurements/measurement-2026-10-18-proc04-fdelete/method/build-kernels.sh (plain kernels only).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SIDE="${1:?head|wt}"; export SOURCE_DATE_EPOCH=1792152000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"; FILES=""
[ "$SIDE" = wt ] && FILES="$( (git diff --name-only $HEADREV -- kernel tos-api; git ls-files --others --exclude-standard -- kernel tos-api) | sort -u)"
echo "working-tree files over $HEADREV ($SIDE):"; echo "$FILES" | sed 's/^/  /'
T="$S/trees/$SIDE"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
(cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
for f in $FILES; do mkdir -p "$T/$(dirname "$f")"; cp "$WS/$f" "$T/$f"; done
(cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$SIDE.log" 2>&1 || { tail -30 "$S/logs/build-$SIDE.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-$SIDE.img"; shasum -a 256 "$S/bin/kernel8-$SIDE.img"
