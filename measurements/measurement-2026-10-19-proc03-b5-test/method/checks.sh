#!/usr/bin/env bash
# checks.sh <label> -- static checks on the shared working tree (adapted from
# measurements/measurement-2026-10-13-mem05-acceptance/method/checks.sh): host tests (cargo xtask test), fresh release
# builds of both boards (warning counts; images to the scratch dir, not the run dir), clippy both boards, desk drift
# probe, SDK host tests. Output to ../checks/<label>-*.txt; one summary line per check to stdout.
set -uo pipefail
L="${1:?label}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
OUT="$HERE/../checks"; mkdir -p "$OUT" "$S/bin"
cd "$WS"
cargo xtask test > "$OUT/$L-host-tests.txt" 2>&1
echo "host tests (pass fail ignored): $(grep '^test result' "$OUT/$L-host-tests.txt" | awk '{p+=$4; f+=$6; i+=$8} END {print p, f, i}')"
for b in rpi4 qemu-virt; do
  cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat --release > /dev/null 2>&1
  cargo xtask build --board $b --release > "$OUT/$L-build-$b.txt" 2>&1
  echo "build $b: rc=$? $(grep -E 'generated [0-9]+ warning' "$OUT/$L-build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ') $(grep -c '^error' "$OUT/$L-build-$b.txt") errors"
  if [ $b = rpi4 ]; then cp kernel8.img "$S/bin/kernel8-$L.img"; else
    OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
    "$OC" -O binary target/aarch64-unknown-none-softfloat/release/kernel "$S/bin/kernel-virt-$L.img"; fi
done
for b in rpi4 qemu-virt; do
  cargo clippy --package kernel --no-default-features --features board-$b --lib --bins --target aarch64-unknown-none-softfloat > "$OUT/$L-clippy-$b.txt" 2>&1
  echo "clippy $b: $(grep -E 'generated [0-9]+ warning' "$OUT/$L-clippy-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ')"
done
sh tools/desk-drift-probe.sh > "$OUT/$L-drift.txt" 2>&1; echo "drift: $(tail -1 "$OUT/$L-drift.txt")"
sh sdk/run-host-tests.sh > "$OUT/$L-sdk-host.txt" 2>&1; echo "sdk host: $(tail -1 "$OUT/$L-sdk-host.txt")"
