#!/usr/bin/env bash
# build-selftest.sh <id> -- scratch kernel (rpi4 release, QEMU raspi4b only) with the desktop self-tests
# (`cargo xtask build --features selftest-desk`, the NEWDESK program feature) + injector launch:NEWDESK=st-launch.script
# (Return for the self-test's DELETE confirmation box, as measurement-2026-10-16-desk43-u10-test kt). -> ../bin/kernel8-<id>.img
set -euo pipefail
id="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/d64
export SOURCE_DATE_EPOCH=1792238400
T="$W/tree"; LOG="$W/build-$id.log"
if [ "${SRC:-tree}" = head ]; then
  # SRC=head: the committed HEAD (git archive, without the DESK64 change) -- the control kernel.
  rm -rf "$T"; mkdir -p "$T"; (cd "$HERE/../../.." && git archive HEAD) | tar -x -C "$T"
else
  bash "$HERE/make-scratch.sh" "$T"
fi
python3 "$HERE/../../../tools/inject-arm.py" "$T" --tag d64 launch:NEWDESK="$HERE/st-launch.script"
(cd "$T" && cargo xtask build --board rpi4 --release --features selftest-desk) > "$LOG" 2>&1 || { tail -40 "$LOG"; exit 1; }
mkdir -p "$M/bin"; cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
shasum -a 256 "$M/bin/kernel8-$id.img"
