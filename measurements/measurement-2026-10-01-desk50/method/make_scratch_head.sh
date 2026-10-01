#!/usr/bin/env bash
# make_scratch_head.sh <ws> <scratch-dir> <scripts.rs>
# The BEFORE kernel: the same rsync, then every file DESK50 touched under
# kernel/src/desk/ replaced by its HEAD version (`git show HEAD:<path>`, read
# only -- no checkout/restore on the shared tree), then the injector and the
# desktop-arming patch. desk50_patch.py is NOT applied (its anchors are DESK50
# code), so this kernel has no [d50] lines and no zoom hold.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
for f in dir main mod model pro win inf; do
  git -C "$WS" show "HEAD:kernel/src/desk/$f.rs" > "$SCR/kernel/src/desk/$f.rs"
done
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/desk_arm_patch.py" "$SCR"
echo "HEAD scratch ready: $SCR"
