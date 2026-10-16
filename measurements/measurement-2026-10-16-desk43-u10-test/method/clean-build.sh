#!/usr/bin/env bash
# clean-build.sh <workdir> -- DESK43 U9-3/4/5 copy of measurement-2026-10-14-desk43-u9-1-2/method/clean-build.sh (u912 -> acc, nothing else).
# Its header: DESK43 U9-2 (copy of measurement-2026-10-14-desk43-u8b/method/clean-build.sh, u8b -> u912;
# added: the NEWDESK.NAP / HELLOGEM.NAP of both boards are compared with the shared tree's): the product build from a
# CLEAN target directory on a SCRATCH copy of the shared tree at ANOTHER PATH (make-scratch.sh, plus .git so the
# build stamp's rev is the shared tree's): `cargo xtask build --board rpi4|qemu-virt --release` -- programs first
# (NEWDESK.NAP is embedded), then the kernel. Images compared with bin/kernel8-acc.img / bin/kernel-virt-acc.img
# of checks.sh (expected: byte-identical = the kernel image, with the embedded blob, is path-reproducible).
set -uo pipefail
W="${1:?workdir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"
OUT="$M/checks"
export SOURCE_DATE_EPOCH=1791979200
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
A="$W/tree-clean"
rm -rf "$A"; bash "$HERE/make-scratch.sh" "$A"
rsync -a "$WS/.git/" "$A/.git/"
[ ! -e "$A/target" ] && echo "A: $A -- fresh copy has no target/ (git rev $(git -C "$A" rev-parse --short=8 HEAD))"
for b in rpi4 qemu-virt; do
  (cd "$A" && cargo xtask build --board $b --release) > "$OUT/acc-clean-build-$b.txt" 2>&1
  echo "A: clean build $b rc=$? $(grep -E 'xtask: building|generated [0-9]+ warning' "$OUT/acc-clean-build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/; s/^ *//; s#'"$A"'/##' | tr '\n' ' ')"
  if [ $b = rpi4 ]; then IMG="$A/kernel8.img"; REF="$M/bin/kernel8-acc.img"; else
    "$OC" -O binary "$A/target/aarch64-unknown-none-softfloat/release/kernel" "$W/kernel-virt-clean.img"; IMG="$W/kernel-virt-clean.img"; REF="$M/bin/kernel-virt-acc.img"; fi
  echo "A: $b image vs $(basename "$REF"): size $(wc -c < "$IMG" | tr -d ' ')/$(wc -c < "$REF" | tr -d ' '), differing bytes $(cmp -l "$IMG" "$REF" | wc -l | tr -d ' ')"
  for n in NEWDESK HELLOGEM; do
    echo "A: $b $n.NAP vs shared tree: differing bytes $(cmp -l "$A/target/programs/$b/$n.NAP" "$WS/target/programs/$b/$n.NAP" | wc -l | tr -d ' ')"
  done
done
