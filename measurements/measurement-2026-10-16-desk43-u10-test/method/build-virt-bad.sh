#!/usr/bin/env bash
# build-virt-bad.sh <id>... -- U10-TEST item 5: qemu-virt scratch kernels (release), flat image bin/kernel-virt-<id>.img.
#   kcv   embedded NEWDESK blob with the ELF header zeroed (NEWDESK-BAD.NAP), registry name NEWDESK (load error)
#   kxv   = kcv + scratch instrument U10-TEMP (as the developers' kx): at the FIRST fallback only the IKBD codes of `exit`+Return are fed
#   kmv   no registry entry NEWDESK (embedded as NEWDESKX) -> NotFound
# Scratch copies of the tree only (make-scratch.sh); the shared tree is untouched.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad
export SOURCE_DATE_EPOCH=1792152000
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
for id in "$@"; do
  T="$W/tree-$id"; LOG="$W/build-$id.log"
  bash "$HERE/make-scratch.sh" "$T"
  python3 - "$T/kernel/embed.rs" $id <<'PY'
import sys
p, id = sys.argv[1], sys.argv[2]; s = open(p).read()
old = 'pub const EMBEDDED: &[Embedded] = &[Embedded { name: "NEWDESK", file: "NEWDESK.NAP" }];'
assert s.count(old) == 1
new = 'Embedded { name: "NEWDESKX", file: "NEWDESK.NAP" }' if id == 'kmv' else 'Embedded { name: "NEWDESK", file: "NEWDESK-BAD.NAP" }'
open(p, "w").write(s.replace(old, 'pub const EMBEDDED: &[Embedded] = &[' + new + ']; // U10-TEST-TEMP'))
PY
  if [ $id != kmv ]; then
    (cd "$T" && cargo xtask build-programs --board qemu-virt) > "$W/build-$id-programs.log" 2>&1
    python3 - "$T/target/programs/qemu-virt/NEWDESK.NAP" "$T/target/programs/qemu-virt/NEWDESK-BAD.NAP" <<'PY'
import sys
b = bytearray(open(sys.argv[1], "rb").read()); b[:64] = bytes(64)
open(sys.argv[2], "wb").write(b)
PY
  fi
  if [ $id = kxv ]; then
    python3 - "$T/kernel/src/shell_loop.rs" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
old = "    shell::set_sh_isgem(false);\n    sh_chgrf();\n"
assert s.count(old) == 1
inst = ('    { // U10-TEMP: type `exit` + Return into EmuCON once (scratch instrument only)\n'
        '        static ONCE: AtomicBool = AtomicBool::new(false);\n'
        '        if !ONCE.swap(true, Ordering::AcqRel) {\n'
        '            for sc in [0x12u8, 0x2D, 0x17, 0x14, 0x1C] { crate::ikbd::feed(sc); crate::ikbd::feed(sc | 0x80); }\n'
        '            kprintln!("[u10] fed `exit` + Return for EmuCON");\n'
        '        }\n'
        '    }\n')
open(p, "w").write(s.replace(old, old + inst))
PY
  fi
  (cd "$T" && cargo xtask build --board qemu-virt --release) > "$LOG" 2>&1 || { tail -30 "$LOG"; exit 1; }
  "$OC" -O binary "$T/target/aarch64-unknown-none-softfloat/release/kernel" "$M/bin/kernel-virt-$id.img"
  echo "$id: $(grep -E 'generated [0-9]+ warning' "$LOG" | tr '\n' ' ') $(shasum -a 256 "$M/bin/kernel-virt-$id.img" | cut -c1-16)"
done
