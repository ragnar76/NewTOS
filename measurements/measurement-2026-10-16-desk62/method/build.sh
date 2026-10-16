#!/usr/bin/env bash
# build.sh <id> <desktop-script> [ARM ...] -- scratch kernel (rpi4 release, QEMU raspi4b only): rsync copy of the shared tree
# (make-scratch.sh), d62-trace.py, tools/inject-arm.py --tag <id> desktop=<script>. -> ../bin/kernel8-<id>.img (gitignored)
set -euo pipefail
id="${1:?}"; SCR="${2:?}"; shift 2   # further ARMs (trigger=script) go to inject-arm.py as given
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"
W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/d62
export SOURCE_DATE_EPOCH=1792152000
T="$W/tree"; LOG="$W/build-$id.log"
bash "$HERE/make-scratch.sh" "$T"
python3 "$HERE/d62-trace.py" "$T"
python3 "$WS/tools/inject-arm.py" "$T" --tag "$id" desktop="$SCR" "$@"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$LOG" 2>&1 || { tail -40 "$LOG"; exit 1; }
cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
shasum -a 256 "$M/bin/kernel8-$id.img"
