#!/usr/bin/env bash
# build-kernels.sh -- fresh release rpi4 kernels from `git archive $HEADREV` (HEAD; SDK01 F changes no kernel-side file, checked)
# in scratch trees with their own target dirs. Variants: plain (product), qfi (+ up:QED=qfi.script), tdv (+ up:DESKTOP=tdv.script).
# Instrumentation (tools/inject-arm.py IKBD injector) on the scratch trees only. Pattern of
# measurement-2026-10-20-sdk01a-test/method/build-kernels.sh.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; export SOURCE_DATE_EPOCH=1792540800
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"
K="$( (git diff --name-only $HEADREV -- kernel tos-api tos-types; git ls-files --others --exclude-standard -- kernel tos-api tos-types) | sort -u)"
[ -z "$K" ] || { echo "kernel-side working-tree changes present:"; echo "$K"; exit 1; }
for k in ${KERNELS:-plain qfi tdv}; do
  T="$S/trees/$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  git archive $HEADREV | tar -x -C "$T"
  [ $k = qfi ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:QED="$HERE/qfi.script"
  [ $k = tdv ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:DESKTOP="$HERE/tdv.script"
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-$k.log" 2>&1 || { tail -30 "$S/logs/build-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$k.img"; shasum -a 256 "$S/bin/kernel8-$k.img"
done
