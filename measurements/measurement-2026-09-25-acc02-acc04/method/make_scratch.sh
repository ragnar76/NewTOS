#!/usr/bin/env bash
# make_scratch.sh <scratch-tree> <scripts.rs> [probe]
#
# A third argument `probe` adds probe_patch.py (the serial icon/menu dump),
# which is how every click coordinate in the action lists was found.
#
# ACC02/ACC03/ACC04 scratch method: rsync of the crates (never the shared tree)
# + the standard injector scaffolding + probe_patch.py.
#
# No "before" mode: this batch adds a capability that did not exist, so the
# before-state is "no accessory is loaded and no title appears under Desk",
# which the plain (un-injected) boot run already shows.
set -euo pipefail
TREE="$1"; SCRIPTS="$2"; PROBE="${3:-noprobe}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS=/Volumes/Daten/Development/PiRusTOS
rm -rf "$TREE"; mkdir -p "$TREE"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk xtask; do
  [ -e "$WS/$d" ] && rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$TREE/"
done
python3 "$HERE/a79_patch_desk10.py" "$TREE"
python3 "$HERE/apply_injector.py"   "$TREE" "$SCRIPTS"
python3 "$HERE/a78_patch2.py"       "$TREE"
python3 "$HERE/key_step_patch.py"   "$TREE"
python3 "$HERE/acc_inject_patch.py" "$TREE"
if [ "${ACC_IDENT_TRACE:-0}" = "1" ]; then python3 "$HERE/ident_trace_patch.py" "$TREE"; fi
if [ "${ACC_ELR_PROBE:-0}" = "1" ]; then python3 "$HERE/elr_probe_patch.py" "$TREE"; fi
if [ "$PROBE" = "probe" ]; then python3 "$HERE/probe_patch.py" "$TREE"; fi
echo "scratch ready: $TREE"
