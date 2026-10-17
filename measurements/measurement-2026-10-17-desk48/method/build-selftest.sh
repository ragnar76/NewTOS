#!/usr/bin/env bash
# build-selftest.sh <id> -- (DESK48: copy of ../../measurement-2026-10-17-desk64/method/build-selftest.sh; scratch dir d48,
# injector tag d48, plus the source touch of build.sh.) Scratch kernel (rpi4 release, QEMU raspi4b only) with the desktop
# self-tests (`cargo xtask build --features selftest-desk`) + injector launch:NEWDESK=st-launch.script (Return for the
# self-test's DELETE confirmation box). SRC=head = the committed HEAD as control. -> ../bin/kernel8-<id>.img
set -euo pipefail
id="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/d48
export SOURCE_DATE_EPOCH=1792238400
T="$W/tree"; LOG="$W/build-$id.log"
if [ "${SRC:-tree}" = head ]; then
  rm -rf "$T"; mkdir -p "$T"; (cd "$HERE/../../.." && git archive HEAD) | tar -x -C "$T"
else
  bash "$HERE/make-scratch.sh" "$T"
  find "$T" -path "$T/target" -prune -o \( -name '*.rs' -o -name 'Cargo.toml' \) -exec touch {} +
fi
python3 "$HERE/../../../tools/inject-arm.py" "$T" --tag d48 launch:NEWDESK="$HERE/st-launch.script"
(cd "$T" && cargo xtask build --board rpi4 --release --features selftest-desk) > "$LOG" 2>&1 || { tail -40 "$LOG"; exit 1; }
mkdir -p "$M/bin"; cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
shasum -a 256 "$M/bin/kernel8-$id.img"
