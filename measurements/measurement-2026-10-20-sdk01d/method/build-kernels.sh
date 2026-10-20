#!/usr/bin/env bash
# build-kernels.sh -- release rpi4 kernels from `git archive $HEADREV` in scratch trees (the
# shared tree has no kernel-side change; checked). plain = product kernel; inj = + the IKBD
# injector armed on up:HELLO=hello.script (tools/inject-arm.py, scratch tree only).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"
K="$( (git diff --name-only $HEADREV -- kernel tos-api tos-types programs; git ls-files --others --exclude-standard -- kernel tos-api tos-types programs) | sort -u)"
[ -z "$K" ] || { echo "kernel-side working-tree changes present:"; echo "$K"; exit 1; }
for k in plain inj; do
  T="$S/trees/$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  git archive $HEADREV | tar -x -C "$T"
  [ $k = inj ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:HELLO="$HERE/hello.script"
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-$k.log" 2>&1 || { tail -30 "$S/logs/build-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$k.img"; shasum -a 256 "$S/bin/kernel8-$k.img"
done
