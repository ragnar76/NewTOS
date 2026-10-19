#!/usr/bin/env bash
# build-kernels.sh -- fresh release rpi4 kernels from `git archive $HEADREV` (HEAD; no kernel-side working-tree changes, checked)
# in scratch trees with their own target dirs. Variants: plain (product), qsv (+ up:QED=qsv.script), tnav (+ up:DESKTOP=tnav.script).
# Instrumentation (tools/inject-arm.py IKBD injector) on the scratch trees only.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; export SOURCE_DATE_EPOCH=1792368000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"
K="$( (git diff --name-only $HEADREV -- kernel tos-api tos-types; git ls-files --others --exclude-standard -- kernel tos-api tos-types) | sort -u)"
[ -z "$K" ] || { echo "kernel-side working-tree changes present:"; echo "$K"; exit 1; }
for k in ${KERNELS:-plain}; do
  T="$S/trees/$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  git archive $HEADREV | tar -x -C "$T"
  [ $k = qsv ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:QED="$HERE/$QS_SCRIPT"
  [ $k = tnav ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:DESKTOP="$HERE/tnav.script"
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-$k.log" 2>&1 || { tail -30 "$S/logs/build-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$k.img"; shasum -a 256 "$S/bin/kernel8-$k.img"
done
