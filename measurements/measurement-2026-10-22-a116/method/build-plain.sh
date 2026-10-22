#!/usr/bin/env bash
# build-plain.sh <head|wt> <name> -- fresh scratch tree (mktree.sh), release rpi4 kernel WITHOUT injector -> $S/bin/kernel8-<name>.img
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; export SOURCE_DATE_EPOCH=1792368000
mkdir -p "$S/bin" "$S/logs"
bash "$HERE/mktree.sh" "$1" "$2" >/dev/null
T="$S/trees/$2"; rm -f "$T/kernel8.img"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-$2.log" 2>&1 || { tail -30 "$S/logs/build-$2.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-$2.img"; shasum -a 256 "$S/bin/kernel8-$2.img"
