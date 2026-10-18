#!/usr/bin/env bash
# build-inj.sh <side> -- side=head: `git archive $HEADREV`; side=wt: that + the uncommitted kernel/ tos-api/ files of the
# working tree. Kernel "inj": tools/inject-arm.py launch:P4Z2=con01.script (EmuCON session). Release rpi4, scratch tree only.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SIDE="${1:?head|wt}"; export SOURCE_DATE_EPOCH=1792152000; k=inj
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"; FILES=""
[ "$SIDE" = wt ] && FILES="$( (git diff --name-only $HEADREV -- kernel tos-api; git ls-files --others --exclude-standard -- kernel tos-api) | sort -u)"
echo "files over $HEADREV ($SIDE):"; echo "$FILES" | sed 's/^/  /'
T="$S/trees/$SIDE-$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
(cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
for f in $FILES; do mkdir -p "$T/$(dirname "$f")"; cp "$WS/$f" "$T/$f"; done
python3 "$WS/tools/inject-arm.py" "$T" --tag inj launch:P4Z2="$HERE/con01.script"
(cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$SIDE-$k.log" 2>&1 || { tail -30 "$S/logs/build-$SIDE-$k.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-$SIDE-$k.img"; shasum -a 256 "$S/bin/kernel8-$SIDE-$k.img"
