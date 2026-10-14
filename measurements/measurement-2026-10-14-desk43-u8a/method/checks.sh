#!/usr/bin/env bash
# (copied from measurements/measurement-2026-10-14-desk43-u7/method/checks.sh for DESK43 U8a, unchanged apart from this line)
# checks.sh <label> -- the per-sub-step static checks of ARCH03 stage 1:
# host tests, fresh release builds of both boards (warning counts), clippy for
# both feature sets, the desk drift probe and the SDK host tests. Output goes
# to ../checks/<label>-*.txt; a one-line summary per check to stdout.
set -uo pipefail
L="${1:?label}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="$HERE/../checks"; mkdir -p "$OUT"
cd "$WS"
# 2026-10-14 12:00 UTC: same build-date stamp as the baseline build of the
# unchanged tree (bin/kernel8-base.img), so a byte comparison sees only code.
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1791979200}"
cargo test -p tos-api -p tos-types -p xtask --no-fail-fast > "$OUT/$L-host-tests.txt" 2>&1
echo "host tests (pass fail ignored): $(grep '^test result' "$OUT/$L-host-tests.txt" | awk '{p+=$4; f+=$6; i+=$8} END {print p, f, i}')"
for b in rpi4 qemu-virt; do
  cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat --release > /dev/null 2>&1
  cargo xtask build --board $b --release > "$OUT/$L-build-$b.txt" 2>&1
  echo "build $b: rc=$? $(grep -E 'generated [0-9]+ warning' "$OUT/$L-build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ') $(grep -c '^error' "$OUT/$L-build-$b.txt") errors"
  if [ $b = rpi4 ]; then cp kernel8.img "$HERE/../bin/kernel8-$L.img"; else
    OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
    "$OC" -O binary target/aarch64-unknown-none-softfloat/release/kernel "$HERE/../bin/kernel-virt-$L.img"; fi
done
for b in rpi4 qemu-virt; do
  cargo clippy --package kernel --no-default-features --features board-$b --lib --bins --target aarch64-unknown-none-softfloat > "$OUT/$L-clippy-$b.txt" 2>&1
  echo "clippy $b: $(grep -E 'generated [0-9]+ warning' "$OUT/$L-clippy-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ')"
done
sh tools/desk-drift-probe.sh -v > "$OUT/$L-drift.txt" 2>&1; echo "drift: $(tail -1 "$OUT/$L-drift.txt")"
sh tools/desk-drift-probe.sh -u > "$OUT/$L-drift-unsafe.txt" 2>&1; echo "unsafe: $(tail -1 "$OUT/$L-drift-unsafe.txt")"
cargo xtask emutos-consts -v > "$OUT/$L-emutos-consts.txt" 2>&1; echo "emutos-consts: $(tail -1 "$OUT/$L-emutos-consts.txt")"
sh sdk/run-host-tests.sh > "$OUT/$L-sdk-host.txt" 2>&1; echo "sdk host: $(tail -1 "$OUT/$L-sdk-host.txt")"
