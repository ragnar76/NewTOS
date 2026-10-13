#!/usr/bin/env bash
# fs11-kernels.sh <label> <scratch-root> [git-rev] -- the instrumented "keys"
# kernel for the FS11 probe's part B: KEYS02 injector
# (inj-keys02-apply-instrumentation.py) + the FS11 key schedule
# (fs11-schedule.py), from a scratch copy of the shared tree (make-scratch.sh)
# -- or, with [git-rev], of that commit (git archive; the red run on the
# pre-FS11 code). Never applied to the shared tree.
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"; REV="${3:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
BIN="$HERE/../bin"
D="$R/keys-$L"
if [ -n "$REV" ]; then
  rm -rf "$D"; mkdir -p "$D"
  (cd "$WS" && git archive "$REV" -- . ':!measurements' ':!docs' ':!tosdump') | tar -x -C "$D"
else
  sh "$HERE/make-scratch.sh" "$D"
fi
python3 "$HERE/inj-keys02-apply-instrumentation.py" "$D" > /dev/null
python3 "$HERE/fs11-schedule.py" "$D"
(cd "$D" && cargo xtask build --board rpi4 --release > "$R/keys-$L-rpi4.log" 2>&1) || { tail -20 "$R/keys-$L-rpi4.log"; exit 1; }
cp "$D/kernel8.img" "$BIN/kernel8-keys-$L.img"
echo "built kernel8-keys-$L.img${REV:+ from $REV}"
