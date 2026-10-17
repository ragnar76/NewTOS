#!/usr/bin/env bash
# build.sh <id> <desktop-script> [ARM ...] -- DESK48 scratch kernel (rpi4 release, QEMU raspi4b only): rsync copy of the
# shared tree (make-scratch.sh) or, with SRC=head, `git archive HEAD` (the pre-change "red" control), d48-trace.py (AES trace),
# tools/inject-arm.py --tag <id> desktop=<script>. -> ../bin/kernel8-<id>.img (gitignored). Copy of the desk62-fix build.sh.
set -euo pipefail
id="${1:?}"; SCR="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"
W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/d48
export SOURCE_DATE_EPOCH=1792152000
T="$W/tree"; LOG="$W/build-$id.log"
if [ "${SRC:-tree}" = head ]; then
  rm -rf "$T"; mkdir -p "$T"; (cd "$WS" && git archive HEAD) | tar -x -C "$T"
else
  bash "$HERE/make-scratch.sh" "$T"
  # rsync -a keeps the shared tree's mtimes and make-scratch.sh keeps target/: after a SRC=head build the head artefacts
  # are newer than the tree's sources and cargo would reuse them (it did once in this run). Touch every source so the
  # tree build always recompiles what differs.
  find "$T" -path "$T/target" -prune -o \( -name '*.rs' -o -name 'Cargo.toml' \) -exec touch {} +
fi
python3 "$HERE/d48-trace.py" "$T"
python3 "$WS/tools/inject-arm.py" "$T" --tag "$id" desktop="$SCR" "$@"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$LOG" 2>&1 || { tail -40 "$LOG"; exit 1; }
mkdir -p "$M/bin"; cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
shasum -a 256 "$M/bin/kernel8-$id.img"
