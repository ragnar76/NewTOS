#!/usr/bin/env bash
# checks.sh <tree> <outdir> -- host tests, fresh release builds (warnings), clippy both boards, drift probe, sdk host tests.
set -uo pipefail
WS="${1:?}"; OUT="${2:?}"; mkdir -p "$OUT"; cd "$WS"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1791979200}"
cargo test -p tos-api -p tos-types -p xtask --no-fail-fast > "$OUT/host-tests.txt" 2>&1
echo "host tests (pass fail ignored): $(grep '^test result' "$OUT/host-tests.txt" | awk '{p+=$4; f+=$6; i+=$8} END {print p, f, i}')"
for b in rpi4 qemu-virt; do
  cargo clean -p kernel -p tos-api -p desktop -p newtos-gem --target aarch64-unknown-none-softfloat --release > /dev/null 2>&1
  cargo xtask build --board $b --release > "$OUT/build-$b.txt" 2>&1
  echo "build $b: rc=$? $(grep -E 'generated [0-9]+ warning' "$OUT/build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ') $(grep -c '^error' "$OUT/build-$b.txt") errors; desktop compiled: $(grep -c 'Compiling desktop' "$OUT/build-$b.txt")"
done
for b in rpi4 qemu-virt; do
  cargo clippy --package kernel --no-default-features --features board-$b --lib --bins --target aarch64-unknown-none-softfloat > "$OUT/clippy-$b.txt" 2>&1
  echo "clippy $b: $(grep -E 'generated [0-9]+ warning' "$OUT/clippy-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ')"
done
cargo clippy --package kernel --no-default-features --features board-rpi4,selftest-desk --lib --bins --target aarch64-unknown-none-softfloat > "$OUT/clippy-rpi4-st.txt" 2>&1
echo "clippy rpi4+selftest-desk: $(grep -E 'generated [0-9]+ warning' "$OUT/clippy-rpi4-st.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ')"
sh tools/desk-drift-probe.sh -v > "$OUT/drift.txt" 2>&1; echo "drift: $(tail -1 "$OUT/drift.txt")"
sh sdk/run-host-tests.sh > "$OUT/sdk-host.txt" 2>&1; echo "sdk host: $(tail -1 "$OUT/sdk-host.txt")"
