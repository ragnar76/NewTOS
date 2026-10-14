#!/usr/bin/env bash
# (DESK43 U9-0 copy of measurement-2026-10-14-desk43-u8b/method/checks.sh, unchanged below this line; label u90.)
# (copied from measurements/measurement-2026-10-14-desk43-u6b/method/checks.sh for DESK43 U8b. Changed: the warning
#  summaries list every crate (`desktop` is its own crate now); kernel clippy also with selftest-desk; the programs/
#  clippy block names its packages (a bare `cargo clippy` over programs/ stops at the desktop's missing-board
#  compile_error!) and adds the desktop crate per board, with selftest-desk; the dependency graph (cargo tree) of the
#  desktop and the kernel; desk-cite-list.)
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
  cargo clean -p kernel -p tos-api -p desktop -p newtos-gem --target aarch64-unknown-none-softfloat --release > /dev/null 2>&1
  cargo xtask build --board $b --release > "$OUT/$L-build-$b.txt" 2>&1
  echo "build $b: rc=$? $(grep -E 'generated [0-9]+ warning' "$OUT/$L-build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ') $(grep -c '^error' "$OUT/$L-build-$b.txt") errors; desktop crate compiled: $(grep -c 'Compiling desktop' "$OUT/$L-build-$b.txt")"
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
# DESK43 U6b: the program side. `cargo xtask build` above already ran build-programs for each board; here explicitly,
# then clippy of the programs workspace (default features = the runtime, and newtos-gem without `rt` = what the kernel
# links), and the sha256 of the per-board NAPs.
for b in rpi4 qemu-virt; do
  cargo xtask build-programs --board $b > "$OUT/$L-build-programs-$b.txt" 2>&1
  echo "build-programs $b: rc=$? $(grep -cE '^(warning|error)' "$OUT/$L-build-programs-$b.txt") warning/error lines"
done
(cd programs && touch newtos-gem/src/lib.rs hello-gem/src/lib.rs desktop/src/lib.rs && \
 cargo clippy -p hello-gem -p newtos-gem --release > "$OUT/$L-clippy-programs.txt" 2>&1; \
 cargo clippy -p newtos-gem --no-default-features --release > "$OUT/$L-clippy-newtos-gem-nort.txt" 2>&1; \
 for b in rpi4 qemu-virt; do touch desktop/src/lib.rs; cargo clippy -p desktop --features board-$b --release > "$OUT/$L-clippy-desktop-$b.txt" 2>&1; done; \
 touch desktop/src/lib.rs; cargo clippy -p desktop --features board-rpi4,selftest-desk --release > "$OUT/$L-clippy-desktop-rpi4-selftest-desk.txt" 2>&1; \
 cargo clippy -p desktop --release > "$OUT/$L-clippy-desktop-noboard.txt" 2>&1)
echo "clippy programs (hello-gem+newtos-gem): $(grep -cE '^(warning|error)' "$OUT/$L-clippy-programs.txt") / newtos-gem no rt: $(grep -cE '^(warning|error)' "$OUT/$L-clippy-newtos-gem-nort.txt") warning/error lines"
for f in desktop-rpi4 desktop-qemu-virt desktop-rpi4-selftest-desk; do
  echo "clippy $f: $(grep -cE '^(warning|error)' "$OUT/$L-clippy-$f.txt") warning/error lines ($(grep -c 'Checking desktop' "$OUT/$L-clippy-$f.txt") x 'Checking desktop')"
done
echo "clippy desktop without board (must fail at the compile_error!): $(grep -c 'no About-box machine line' "$OUT/$L-clippy-desktop-noboard.txt") hit(s)"
cargo clippy --package kernel --no-default-features --features board-rpi4,selftest-desk --lib --bins --target aarch64-unknown-none-softfloat > "$OUT/$L-clippy-rpi4-selftest-desk.txt" 2>&1
echo "clippy rpi4 selftest-desk: $(grep -E 'generated [0-9]+ warning' "$OUT/$L-clippy-rpi4-selftest-desk.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ')"
(cd programs && cargo tree -p desktop --features board-rpi4 -e normal,build,dev) > "$OUT/$L-deptree-desktop.txt" 2>&1
cargo tree -p kernel --features board-rpi4 -e normal,build --depth 1 > "$OUT/$L-deptree-kernel.txt" 2>&1
cargo tree -p kernel --features board-rpi4 -e normal -i desktop > "$OUT/$L-deptree-kernel-inv-desktop.txt" 2>&1
echo "deptree desktop: $(grep -oE '(newtos-gem|tos-types|tos-api|kernel|desktop) v' "$OUT/$L-deptree-desktop.txt" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
bash tools/desk-cite-list.sh > "$OUT/$L-cite-list.txt" 2>&1; echo "cite: $(tail -1 "$OUT/$L-cite-list.txt")"
shasum -a 256 target/programs/rpi4/HELLOGEM.NAP target/programs/qemu-virt/HELLOGEM.NAP > "$OUT/$L-nap-sha256.txt"
echo "naps: $(awk '{print substr($1,1,12)}' "$OUT/$L-nap-sha256.txt" | tr '\n' ' ')"
