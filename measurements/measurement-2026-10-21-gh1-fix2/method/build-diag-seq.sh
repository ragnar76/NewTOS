#!/usr/bin/env bash
# build-diag.sh <head|wt> <name> -- diagnostic kernel: like build-armed.sh but up:QED = qedd.script and
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; export SOURCE_DATE_EPOCH=1792368000
bash "$HERE/mktree.sh" "$1" "$2" >/dev/null
T="$S/trees/$2"; rm -rf "$T/target" "$T/kernel8.img"
python3 "$WS/tools/inject-arm.py" "$T" --tag inj desktop="$HERE/nd.script" up:QED="$HERE/qedd.script" | tail -1
(cd "$T" && cargo xtask build --board rpi4 --release --features svc-seq-trace) > "$S/logs/build-$2.log" 2>&1 || { tail -30 "$S/logs/build-$2.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-$2.img"; shasum -a 256 "$S/bin/kernel8-$2.img"
