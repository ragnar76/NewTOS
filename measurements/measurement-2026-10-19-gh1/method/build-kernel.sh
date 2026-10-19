#!/usr/bin/env bash
# build-kernel.sh <rev> <name> [features] -- fresh release rpi4 kernel from `git archive <rev>` in a scratch tree,
# injector armed on "desktop" with gh1.script (tools/inject-arm.py of the WORKSPACE, applied to the scratch tree only),
# optional cargo features (e.g. aes-trace); GH1LOG=1 adds the [gh1] serial instrumentation (instrument.py);
# EXP=<name> applies the scratch-only experiment experiment-<name>.py; SCRIPT=<file> picks the injector script. Output $S/bin/kernel8-<name>.img.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
REV="$1"; NAME="$2"; FEAT="${3:-}"; export SOURCE_DATE_EPOCH=1792368000
T="$S/trees/$NAME"; rm -rf "$T"; mkdir -p "$T" "$S/bin" "$S/logs"
(cd "$WS" && git archive "$REV") | tar -x -C "$T"
[ -n "${GH1LOG:-}" ] && python3 "$HERE/instrument.py" "$T"
[ -n "${EXP:-}" ] && python3 "$HERE/experiment-$EXP.py" "$T"
python3 "$WS/tools/inject-arm.py" "$T" --tag inj desktop="$HERE/${SCRIPT:-gh1.script}"
F=""; [ -n "$FEAT" ] && F="--features $FEAT"
# aes-trace at CAPACITY 8192 overruns STRAM_OS_RESERVE at HEAD: shrink the ring in the SCRATCH tree only.
if [[ "$FEAT" == *aes-trace* ]]; then
  sed -i '' -e 's/^pub const CAPACITY: usize = 8192;/pub const CAPACITY: usize = 4096;/' -e 's/^pub const PROLOGUE: usize = 2048;/pub const PROLOGUE: usize = 512;/' "$T/tos-api/src/aes/trace.rs"
  grep -n '^pub const CAPACITY\|^pub const PROLOGUE' "$T/tos-api/src/aes/trace.rs"
fi
(cd "$T" && cargo xtask build --board rpi4 --release $F) > "$S/logs/build-$NAME.log" 2>&1 || { tail -30 "$S/logs/build-$NAME.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-$NAME.img"; shasum -a 256 "$S/bin/kernel8-$NAME.img"
