#!/usr/bin/env bash
# make_scratch_head.sh <ws> <scratch-dir> <scripts.rs>
# The BEFORE kernel: the same rsync as make_scratch_x03.sh, then the two
# kernel files X03 touched replaced by their HEAD version (`git show HEAD:`,
# read only -- no checkout/restore on the shared tree), then the injector and
# the desktop-arming patch.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
for f in kernel/src/desk/inf.rs kernel/src/initinfo.rs; do
  git -C "$WS" show "HEAD:$f" > "$SCR/$f"
done
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/desk_arm_patch.py" "$SCR"
echo "HEAD scratch ready: $SCR"
