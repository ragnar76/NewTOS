#!/usr/bin/env bash
# build-inj.sh <head|base> <actions> <tag> -- fresh release rpi4 scratch kernel with the injector script; the scratch worktree
# (git worktree of HEAD / 1dec9058 in $S) gets host_env.rs patched and then restored by hand (copy kept as host_env.rs.orig).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SIDE="$1"; ACT="$2"; TAG="$3"; W="$S/wt-$SIDE"; export SOURCE_DATE_EPOCH=1792152000
H="$W/kernel/src/host_env.rs"; cp "$H" "$S/host_env.rs.$SIDE.orig"
python3 -I "$HERE/mk-injector.py" "$W" "$ACT"
(cd "$W" && touch kernel/src/main.rs tos-api/src/lib.rs && cargo xtask build --board rpi4 --release > "$S/logs/build-inj-$TAG.log" 2>&1) || { tail -20 "$S/logs/build-inj-$TAG.log"; cp "$S/host_env.rs.$SIDE.orig" "$H"; exit 1; }
cp "$W/kernel8.img" "$S/bin/kernel8-$TAG.img"; cp "$S/host_env.rs.$SIDE.orig" "$H"
cmp "$H" "$S/host_env.rs.$SIDE.orig" && shasum -a 256 "$S/bin/kernel8-$TAG.img"
