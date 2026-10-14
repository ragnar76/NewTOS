#!/usr/bin/env bash
# embed-proof.sh <workdir> -- DESK43 U6b: the build coupling from a clean target directory, and the embedding mechanism
# end to end, on SCRATCH copies of the shared tree (make-scratch.sh: no target/, no .git). Nothing here touches the
# shared tree.
#   A  clean-target product builds: `cargo xtask build --board rpi4|qemu-virt --release` in a fresh copy (programs are
#      built first by xtask, then the kernel); the images are compared with bin/kernel8-u6b.img / kernel-virt-u6b.img
#      (expected: only the build stamp's git rev differs -- the copy has no .git, "unknown").
#   B  hard error: a second copy whose kernel/embed.rs lists one program (HELLOGEM.NAP under the registry name HGEMBED,
#      placeholder, NOT NEWDESK) -- `cargo build -p kernel` WITHOUT build-programs must fail with the build script's
#      message; then `cargo xtask build --board rpi4 --release` (programs first) must succeed, the generated table must
#      name the blob, and kernel8.img must contain the NAP byte for byte.
set -uo pipefail
W="${1:?workdir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
OUT="$M/checks"
export SOURCE_DATE_EPOCH=1791979200
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"

# ── A ──
A="$W/tree-clean"
rm -rf "$A"; bash "$HERE/make-scratch.sh" "$A"
# The build stamp carries `git rev-parse --short=8 HEAD` (tos-api/build.rs); without .git it is "unknown" and the
# shorter string shifts .rodata. Copy .git so the stamp equals the shared tree's and the images can be compared.
rsync -a "$(cd "$HERE/../../.." && pwd)/.git/" "$A/.git/"
[ ! -e "$A/target" ] && echo "A: fresh copy has no target/ (git rev $(git -C "$A" rev-parse --short=8 HEAD))"
for b in rpi4 qemu-virt; do
  (cd "$A" && cargo xtask build --board $b --release) > "$OUT/u6b-clean-build-$b.txt" 2>&1
  echo "A: clean build $b rc=$? $(grep -E 'xtask: building|generated [0-9]+ warning' "$OUT/u6b-clean-build-$b.txt" | sed -E 's/.*`([a-z-]+)` \(lib\) generated ([0-9]+) warnings?.*/\1=\2/' | tr '\n' ' ')"
  if [ $b = rpi4 ]; then IMG="$A/kernel8.img"; REF="$M/bin/kernel8-u6b.img"; else
    "$OC" -O binary "$A/target/aarch64-unknown-none-softfloat/release/kernel" "$W/kernel-virt-clean.img"; IMG="$W/kernel-virt-clean.img"; REF="$M/bin/kernel-virt-u6b.img"; fi
  echo "A: $b image vs $(basename "$REF"): size $(wc -c < "$IMG" | tr -d ' ')/$(wc -c < "$REF" | tr -d ' '), differing bytes $(cmp -l "$IMG" "$REF" | wc -l | tr -d ' ')"
  python3 - "$IMG" "$REF" <<'PY'
import sys
a=open(sys.argv[1],'rb').read(); b=open(sys.argv[2],'rb').read()
d=[i for i in range(min(len(a),len(b))) if a[i]!=b[i]]
print("   differing byte offsets (first 8): %s" % [hex(i) for i in d[:8]])
PY
done
ls "$A/target/programs/"*/HELLOGEM.NAP | sed "s#$A/##"

# ── B ──
B="$W/tree-embed"
rm -rf "$B"; bash "$HERE/make-scratch.sh" "$B"
python3 - "$B/kernel/embed.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text()
old = "pub const EMBEDDED: &[Embedded] = &[];"
assert s.count(old) == 1
s = s.replace(old, 'pub const EMBEDDED: &[Embedded] = &[Embedded { name: "HGEMBED", file: "HELLOGEM.NAP" }]; // U6B-PROOF')
p.write_text(s)
print("B: kernel/embed.rs patched (scratch copy): HGEMBED <- HELLOGEM.NAP")
PY
(cd "$B" && cargo build -p kernel --no-default-features --features board-rpi4 --release) > "$OUT/u6b-embed-missing.txt" 2>&1
echo "B: kernel build without build-programs: rc=$? (must be non-zero)"
grep -A4 "kernel build: the kernel image embeds" "$OUT/u6b-embed-missing.txt" | sed 's/^/   /'
(cd "$B" && cargo xtask build --board rpi4 --release) > "$OUT/u6b-embed-build.txt" 2>&1
echo "B: cargo xtask build --board rpi4 --release: rc=$?"
GEN=$(ls "$B"/target/aarch64-unknown-none-softfloat/release/build/kernel-*/out/embedded_programs.rs | head -1)
sed 's/^/   /' "$GEN" | sed "s#$B#<tree>#g"
python3 - "$B/kernel8.img" "$B/target/programs/rpi4/HELLOGEM.NAP" <<'PY'
import sys, hashlib
img=open(sys.argv[1],'rb').read(); nap=open(sys.argv[2],'rb').read()
at=img.find(nap)
print("B: NAP %d bytes sha256 %s; found in kernel8.img (%d bytes) at offset %s; second occurrence %s"
      % (len(nap), hashlib.sha256(nap).hexdigest()[:16], len(img), hex(at) if at>=0 else "NOT FOUND",
         img.find(nap, at+1) if at>=0 else "-"))
PY
