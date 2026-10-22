#!/usr/bin/env bash
# lint.sh <tree> <outdir> -- build both boards + clippy counts (kernel per board, tos-api host, newdesk program per board)
set -u
T="$1"; O="$2"; mkdir -p "$O"; cd "$T"
for B in qemu-virt rpi4; do
  cargo xtask build --board $B > "$O/build-$B.log" 2>&1; echo "build $B exit $?" >> "$O/summary.txt"
  grep -cE "^warning" "$O/build-$B.log" | sed "s/^/build-$B warnings: /" >> "$O/summary.txt"
  touch kernel/src/lib.rs tos-api/src/lib.rs
  cargo clippy -p kernel --no-default-features --features board-$B > "$O/clippy-kernel-$B.log" 2>&1; echo "clippy kernel $B exit $?" >> "$O/summary.txt"
  grep -E "^warning: .* generated [0-9]+ warning" "$O/clippy-kernel-$B.log" >> "$O/summary.txt"
  (cd programs && cargo clippy -p newdesk --release --features board-$B > "$O/clippy-newdesk-$B.log" 2>&1; echo "clippy newdesk $B exit $?" >> "$O/summary.txt")
  grep -E "^warning: .* generated [0-9]+ warning" "$O/clippy-newdesk-$B.log" >> "$O/summary.txt"
done
cargo clippy -p tos-api > "$O/clippy-tosapi-host.log" 2>&1; echo "clippy tos-api host exit $?" >> "$O/summary.txt"
grep -E "^warning: .* generated [0-9]+ warning" "$O/clippy-tosapi-host.log" >> "$O/summary.txt"
echo DONE >> "$O/summary.txt"
