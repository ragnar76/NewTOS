#!/usr/bin/env bash
# build-kernel.sh <src> <name> -- fresh release rpi4 kernel in a scratch tree, injector armed on "desktop"
# (tools/inject-arm.py of the WORKSPACE, applied to the scratch tree only).
#   <src> = a git revision  -> `git archive <rev>` (the "before" kernel, HEAD f1c225fc)
#   <src> = WORKTREE        -> the workspace's tracked + untracked-not-ignored files as they are now
#                              (`git ls-files -co --exclude-standard`): the uncommitted GH1 F1-F3 change
# SCRIPT=<file> picks the injector script (default gh1.script). Output $S/bin/kernel8-<name>.img.
# Adapted from measurement-2026-10-19-gh1/method/build-kernel.sh (no instrumentation/experiment variants).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SRC="$1"; NAME="$2"; export SOURCE_DATE_EPOCH=1792368000
T="$S/trees/$NAME"; rm -rf "$T"; mkdir -p "$T" "$S/bin" "$S/logs"
if [ "$SRC" = WORKTREE ]; then
  (cd "$WS" && git ls-files -z -co --exclude-standard | tar --null -T - -cf -) | tar -x -C "$T"
else
  (cd "$WS" && git archive "$SRC") | tar -x -C "$T"
fi
python3 "$WS/tools/inject-arm.py" "$T" --tag inj desktop="$HERE/${SCRIPT:-gh1plain.script}"
(cd "$T" && cargo xtask build --board rpi4 --release) > "$S/logs/build-$NAME.log" 2>&1 || { tail -30 "$S/logs/build-$NAME.log"; exit 1; }
cp "$T/kernel8.img" "$S/bin/kernel8-$NAME.img"; shasum -a 256 "$S/bin/kernel8-$NAME.img"
