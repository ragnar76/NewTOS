#!/usr/bin/env bash
# build-kernels.sh <id>... -- DESK43 U9-3/4/5: the scratch kernels of this run (rpi4 release, QEMU raspi4b only).
# Each is built from an rsync copy of the shared tree (make-scratch.sh) in /tmp/m5j/a15/tree-<id>; the instrumentation
# (tools/inject-arm.py, inj-desk-seq.py, the NEWDESK variants) is applied to that copy only. -> ../bin/kernel8-<id>.img
#   kv     injector desktop=views.script (tag u7v)                       -- 8 views, EL0 vs desk=kernel
#   ksave  injector desktop=save.script (tag sav)                        -- Options -> Save desktop
#   ka     injector desktop=acc.script (tag acc)                         -- open the first accessory
#   kf     injector desktop=f1.script, launch:HELLOGEM/HGPANIC/CRCHILD=ret.script, launch:CRASHTST=crash-keys.script
#          (tag u6a)                                                     -- programs started by F1 from the desktop
#   kf2    = kf, but with method/inject-arm-u935.py (launch: arms fire at the launcher's slot publication -- the EL0
#          desktop's slot is reused by the program it starts within one tick; kf's launch: arms never fired on EL0)
#   kp     NEWDESK with a panic!() before desktop::run() (U935-PANIC-TEMP), injector launch:NEWDESK=key.script (tag kp)
#   kc     embedded NEWDESK blob with its ELF header zeroed (file NEWDESK-BAD.NAP, registry name NEWDESK) + the kf arms
#          desktop=f1.script launch:HELLOGEM=ret.script (tag kc)
#   km     no registry entry NEWDESK (embedded under the name NEWDESKX) + the same arms (tag km)
#   ks     inj-desk-seq.py (Alt-C at desktop up +200) + --features svc-seq-trace -- the G2 oracle
#   kt     --features selftest-desk (reaches NEWDESK through build-programs, U9-4) + launch:NEWDESK=st-launch.script (tag kt)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/tmp/m5j/a15
export SOURCE_DATE_EPOCH=1791979200
INJ="$WS/tools/inject-arm.py"
for id in "$@"; do
  T="$W/tree-$id"; LOG="$W/build-$id.log"
  bash "$HERE/make-scratch.sh" "$T"
  FEAT=()
  case $id in
    kv)    python3 "$INJ" "$T" --tag u7v desktop="$HERE/views.script";;
    ksave) python3 "$INJ" "$T" --tag sav desktop="$HERE/save.script";;
    ka)    python3 "$INJ" "$T" --tag acc desktop="$HERE/acc.script";;
    kf)    python3 "$INJ" "$T" --tag u6a desktop="$HERE/f1.script" launch:HELLOGEM="$HERE/ret.script" \
             launch:HGPANIC="$HERE/ret.script" launch:CRCHILD="$HERE/ret.script" launch:CRASHTST="$HERE/crash-keys.script";;
    kf2)   python3 "$HERE/inject-arm-u935.py" "$T" --tag u6a desktop="$HERE/f1.script" launch:HELLOGEM="$HERE/ret.script" \
             launch:HGPANIC="$HERE/ret.script" launch:CRCHILD="$HERE/ret.script" launch:CRASHTST="$HERE/crash-keys.script";;
    kp)    python3 "$INJ" "$T" --tag kp launch:NEWDESK="$HERE/key.script"
           python3 - "$T/programs/newdesk/src/lib.rs" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
old = "    desktop::run();\n"
assert s.count(old) == 1
s = s.replace(old, '    panic!("U935-PANIC-TEMP: NEWDESK panics at startup"); // U935-PANIC-TEMP\n' + old)
s = s.replace("#![no_std]", "#![no_std]\n#![allow(unreachable_code)] // U935-PANIC-TEMP", 1)
open(p, "w").write(s)
PY
           ;;
    kc|km) python3 "$INJ" "$T" --tag $id desktop="$HERE/f1.script" launch:HELLOGEM="$HERE/ret.script"
           if [ $id = kc ]; then NEW='Embedded { name: "NEWDESK", file: "NEWDESK-BAD.NAP" }'; else NEW='Embedded { name: "NEWDESKX", file: "NEWDESK.NAP" }'; fi
           python3 - "$T/kernel/embed.rs" "$NEW" <<'PY'
import sys
p, new = sys.argv[1], sys.argv[2]; s = open(p).read()
old = 'pub const EMBEDDED: &[Embedded] = &[Embedded { name: "NEWDESK", file: "NEWDESK.NAP" }];'
assert s.count(old) == 1
open(p, "w").write(s.replace(old, 'pub const EMBEDDED: &[Embedded] = &[' + new + ']; // U935-TEMP'))
PY
           if [ $id = kc ]; then
             (cd "$T" && cargo xtask build-programs --board rpi4) > "$W/build-$id-programs.log" 2>&1
             python3 - "$T/target/programs/rpi4/NEWDESK.NAP" "$T/target/programs/rpi4/NEWDESK-BAD.NAP" <<'PY'
import sys
b = bytearray(open(sys.argv[1], "rb").read()); b[:64] = bytes(64)   # ELF header gone -> EPLFMT
open(sys.argv[2], "wb").write(b)
PY
           fi;;
    ks)    python3 "$HERE/inj-desk-seq.py" "$T"; FEAT=(--features svc-seq-trace);;
    kt)    python3 "$INJ" "$T" --tag kt launch:NEWDESK="$HERE/st-launch.script"; FEAT=(--features selftest-desk);;
    kx*)   # acceptance: generic injector kernel, arms listed in method/arms-<id>.txt ("trigger=script" per line, script path relative to method/)
           ARMS=(); while IFS= read -r l; do [ -n "$l" ] && ARMS+=("${l%%=*}=$HERE/${l#*=}"); done < "$HERE/arms-$id.txt"
           python3 "$HERE/inject-arm-u935.py" "$T" --tag $id "${ARMS[@]}";;
    *) echo "unknown kernel id $id"; exit 2;;
  esac
  (cd "$T" && cargo xtask build --board rpi4 --release ${FEAT[@]+"${FEAT[@]}"}) > "$LOG" 2>&1 || { tail -30 "$LOG"; exit 1; }
  echo "$id: $(grep -E 'generated [0-9]+ warning' "$LOG" | tr '\n' ' ')"
  cp "$T/kernel8.img" "$M/bin/kernel8-$id.img"
  [ $id = kt ] && cp "$T/target/programs/rpi4/NEWDESK.NAP" "$M/bin/NEWDESK-kt-selftest.NAP"
  [ $id = kc ] && cp "$T/target/programs/rpi4/NEWDESK-BAD.NAP" "$M/bin/NEWDESK-BAD.NAP"
  [ $id = kp ] && cp "$T/target/programs/rpi4/NEWDESK.NAP" "$M/bin/NEWDESK-kp-panic.NAP" || true
done
