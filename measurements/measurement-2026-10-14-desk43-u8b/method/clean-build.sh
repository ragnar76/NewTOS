#!/usr/bin/env bash
# clean-build.sh <workdir> -- DESK43 U8b (part A of measurement-2026-10-14-desk43-u6b/method/embed-proof.sh, u6b -> u8b;
# part B, the embedding mechanism, is untouched by U8b and not repeated): the product build from a CLEAN target
# directory on a SCRATCH copy of the shared tree (make-scratch.sh, plus .git so the build stamp's rev is the shared
# tree's): `cargo xtask build --board rpi4|qemu-virt --release` -- programs first, then the kernel, which now also
# compiles the desktop crate (programs/desktop) as a path dependency. The images are compared with
# bin/kernel8-u8b.img / bin/kernel-virt-u8b.img of checks.sh (expected: byte-identical).
set -uo pipefail
W="${1:?workdir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
OUT="$M/checks"
export SOURCE_DATE_EPOCH=1791979200
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
A="$W/tree-clean"
rm -rf "$A"; bash "$HERE/make-scratch.sh" "$A"
rsync -a "$(cd "$HERE/../../.." && pwd)/.git/" "$A/.git/"
[ ! -e "$A/target" ] && echo "A: fresh copy has no target/ (git rev $(git -C "$A" rev-parse --short=8 HEAD))"
for b in rpi4 qemu-virt; do
  (cd "$A" && cargo xtask build --board $b --release) > "$OUT/u8b-clean-build-$b.txt" 2>&1
  echo "A: clean build $b rc=$? $(grep -E 'xtask: building|Compiling desktop|generated [0-9]+ warning' "$OUT/u8b-clean-build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/; s/^ *//' | tr '\n' ' ')"
  if [ $b = rpi4 ]; then IMG="$A/kernel8.img"; REF="$M/bin/kernel8-u8b.img"; else
    "$OC" -O binary "$A/target/aarch64-unknown-none-softfloat/release/kernel" "$W/kernel-virt-clean.img"; IMG="$W/kernel-virt-clean.img"; REF="$M/bin/kernel-virt-u8b.img"; fi
  echo "A: $b image vs $(basename "$REF"): size $(wc -c < "$IMG" | tr -d ' ')/$(wc -c < "$REF" | tr -d ' '), differing bytes $(cmp -l "$IMG" "$REF" | wc -l | tr -d ' ')"
done
ls "$A/target/programs/"*/HELLOGEM.NAP | sed "s#$A/##"
