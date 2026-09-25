#!/usr/bin/env bash
# make_scratch_fs04.sh <ws> <scratch-dir> <scripts.rs> [driver|gesture]
# The PORT03 scratch method plus the FS04 probe instrumentation.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; MODE="${4:-gesture}"; PROBE="${5:-0}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 "$HERE/a79_patch_desk10.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
# probe_patch prints one serial line per icon on every redraw -- useful to
# find click points, ruinous for a timing run.  Only with PROBE=1.
if [ "$PROBE" = "1" ]; then python3 "$HERE/probe_patch.py" "$SCR"; fi
python3 "$HERE/t10_workaround.py" "$SCR"
python3 "$HERE/fs04_probe_patch.py" "$SCR" "$MODE"
python3 "$HERE/fs04_mark_hooks.py" "$SCR"
echo "scratch ready: $SCR"
