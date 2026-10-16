#!/usr/bin/env bash
# build-kernels.sh <scratch-dir> <id>... -- DESK43 U10: the rpi4 release kernels of this run (QEMU raspi4b only).
# Each is built from an rsync copy of the shared tree in <scratch-dir>/tree-<id> (never in the shared tree);
# a variant's change is applied to that copy only.  -> <scratch-dir>/kernel8-<id>.img
#   k    the tree as committed for U10, unchanged
#   kc   embedded NEWDESK blob with its ELF header zeroed (file NEWDESK-BAD.NAP, registry name NEWDESK) -- a load error
#        (EPLFMT) of the desktop program: D6-c/D6-d, after U10 the console EmuCON
#   kx   = kc, plus a scratch instrument (U10-TEMP) in shell_loop.rs: on the FIRST fallback only, the IKBD make/break
#        codes of `exit` + Return are fed (crate::ikbd::feed) before EmuCON starts, so EmuCON reads `exit`, returns, and
#        the next sh_main pass shows the DESKTOP arm declining to load again (D6-c). Needed because QEMU serial input
#        does not reach the console on raspi4b and the injector does not run during Cconin.
# (Method of measurements/measurement-2026-10-14-desk43-u9-3-5/method/build-kernels.sh, variant kc, unchanged in substance.)
set -euo pipefail
W="${1:?scratch dir}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"
export SOURCE_DATE_EPOCH=1792152000   # 2026-10-16 12:00 UTC
for id in "$@"; do
  T="$W/tree-$id"; LOG="$W/build-$id.log"; mkdir -p "$T"
  rsync -a --delete --exclude target --exclude .git --exclude measurements --exclude references \
    --exclude Applications --exclude graphify-out --exclude hardware --exclude 'sd-card-backup-*' \
    --exclude tosdump --exclude sdk/apps --exclude docs --exclude kernel8.img "$WS/" "$T/"
  case $id in
    k) ;;
    kc|kx)
      python3 - "$T/kernel/embed.rs" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
old = 'pub const EMBEDDED: &[Embedded] = &[Embedded { name: "NEWDESK", file: "NEWDESK.NAP" }];'
assert s.count(old) == 1
open(p, "w").write(s.replace(old, 'pub const EMBEDDED: &[Embedded] = &[Embedded { name: "NEWDESK", file: "NEWDESK-BAD.NAP" }]; // U10-TEMP'))
PY
      (cd "$T" && cargo xtask build-programs --board rpi4) > "$W/build-$id-programs.log" 2>&1
      python3 - "$T/target/programs/rpi4/NEWDESK.NAP" "$T/target/programs/rpi4/NEWDESK-BAD.NAP" <<'PY'
import sys
b = bytearray(open(sys.argv[1], "rb").read()); b[:64] = bytes(64)   # ELF header gone -> EPLFMT
open(sys.argv[2], "wb").write(b)
PY
      if [ $id = kx ]; then
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
      ;;
    *) echo "unknown kernel id $id"; exit 2;;
  esac
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$LOG" 2>&1 || { tail -30 "$LOG"; exit 1; }
  echo "$id: $(grep -E 'generated [0-9]+ warning' "$LOG" | tr '\n' ' ')"
  cp "$T/kernel8.img" "$W/kernel8-$id.img"
  shasum -a 256 "$W/kernel8-$id.img"
done
