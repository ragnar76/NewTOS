#!/usr/bin/env bash
# checks.sh <label> -- MEM02 close-out static checks (subset of
# measurements/measurement-2026-10-16-desk43-u10-test/method/checks.sh, same commands):
# host tests, fresh release builds of both boards (warning counts), kernel clippy for both
# boards, the desk drift probe and the SDK host tests. Output: ../checks/<label>-*.txt.
set -uo pipefail
L="${1:?label}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="$HERE/../checks"; mkdir -p "$OUT"
BIN="${BIN_DIR:?BIN_DIR (scratch dir for kernel images)}"; mkdir -p "$BIN"
cd "$WS"
cargo test -p tos-api -p tos-types -p xtask --no-fail-fast > "$OUT/$L-host-tests.txt" 2>&1
echo "host tests (pass fail ignored): $(grep '^test result' "$OUT/$L-host-tests.txt" | awk '{p+=$4; f+=$6; i+=$8} END {print p, f, i}')"
for b in rpi4 qemu-virt; do
  cargo clean -p kernel -p tos-api -p desktop -p newtos-gem --target aarch64-unknown-none-softfloat --release > /dev/null 2>&1
  cargo xtask build --board $b --release > "$OUT/$L-build-$b.txt" 2>&1
  echo "build $b: rc=$? $(grep -E 'generated [0-9]+ warning' "$OUT/$L-build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ') $(grep -c '^error' "$OUT/$L-build-$b.txt") errors; compiled: $(grep -oE 'Compiling (kernel|tos-api) ' "$OUT/$L-build-$b.txt" | tr '\n' ' ')"
  if [ $b = rpi4 ]; then cp kernel8.img "$BIN/kernel8-$L.img"; fi
done
for b in rpi4 qemu-virt; do
  cargo clippy --package kernel --no-default-features --features board-$b --lib --bins --target aarch64-unknown-none-softfloat > "$OUT/$L-clippy-$b.txt" 2>&1
  echo "clippy $b: $(grep -E 'generated [0-9]+ warning' "$OUT/$L-clippy-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ')"
done
sh tools/desk-drift-probe.sh -v > "$OUT/$L-drift.txt" 2>&1; echo "drift: $(tail -1 "$OUT/$L-drift.txt")"
sh sdk/run-host-tests.sh > "$OUT/$L-sdk-host.txt" 2>&1; echo "sdk host: $(tail -1 "$OUT/$L-sdk-host.txt")"
