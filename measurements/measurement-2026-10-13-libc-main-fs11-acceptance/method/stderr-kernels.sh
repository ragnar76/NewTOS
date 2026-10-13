#!/usr/bin/env bash
# stderr-kernels.sh <scratch-root> -- keys kernels with emucon-stderr-schedule.py
# from scratch copies of (a) the shared tree (HEAD) and (b) git archive 7bb9579a
# (pre-FS11 code). Never applied to the shared tree.
set -euo pipefail
R="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"; BIN="$HERE/../bin"
sh "$HERE/make-scratch.sh" "$R/ks-new"
rm -rf "$R/ks-old"; mkdir -p "$R/ks-old"; (cd "$WS" && git archive 7bb9579a -- . ':!measurements' ':!docs' ':!tosdump') | tar -x -C "$R/ks-old"
for d in ks-new ks-old; do
  python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/$d" > /dev/null
  python3 "$HERE/emucon-stderr-schedule.py" "$R/$d"
  (cd "$R/$d" && cargo xtask build --board rpi4 --release > "$R/$d.log" 2>&1) || { tail -20 "$R/$d.log"; exit 1; }
done
cp "$R/ks-new/kernel8.img" "$BIN/kernel8-keys-stderr-new.img"; cp "$R/ks-old/kernel8.img" "$BIN/kernel8-keys-stderr-old.img"
