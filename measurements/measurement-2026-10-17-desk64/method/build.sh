#!/usr/bin/env bash
# build.sh <id> <desktop-script> -- scratch kernel (rpi4 release, QEMU raspi4b only): rsync copy of the shared tree
# (make-scratch.sh), tools/inject-arm.py --tag d64 desktop=<script>. -> ../bin/kernel8-<id>.img (gitignored).
# Pattern of ../../measurement-2026-10-16-desk62-fix/method/build.sh without the d62 trace.
set -euo pipefail
id="${1:?}"; SCR="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"
W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/d64
export SOURCE_DATE_EPOCH=1792238400
T="$W/tree"; LOG="$W/build-$id.log"
bash "$HERE/make-scratch.sh" "$T"
python3 "$WS/tools/inject-arm.py" "$T" --tag d64 desktop="$SCR"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$LOG" 2>&1 || { tail -40 "$LOG"; exit 1; }
mkdir -p "$M/bin"; cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
shasum -a 256 "$M/bin/kernel8-$id.img"
