#!/usr/bin/env bash
# build-kernels.sh -- fresh release rpi4 kernels from `git archive HEAD` into scratch trees with their own target dirs:
#   plain : no instrumentation            -> $S/bin/kernel8-plain.img
#   inj   : + tools/inject-arm.py (launch:DESKTOP=td.script, desktop=s1.script) -> $S/bin/kernel8-inj.img
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SC="$HERE/../scripts"
export SOURCE_DATE_EPOCH=1792152000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
for k in plain inj; do
  T="$S/trees/$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
  [ $k = inj ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kd launch:DESKTOP="$SC/td.script" desktop="$SC/s1.script"
  (cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$k.log" 2>&1 || { tail -30 "$S/logs/build-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$k.img"; shasum -a 256 "$S/bin/kernel8-$k.img"
done
