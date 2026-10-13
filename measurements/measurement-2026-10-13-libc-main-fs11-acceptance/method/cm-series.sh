#!/usr/bin/env bash
# cm-series.sh <label> <cards-root> [modes...] -- QEMU raspi4b series for the
# libc crt0 -> mintlib `_main` change, per protection mode (mode.sh: on | off |
# strict, always serial=on). Every program on the cards is rebuilt with the new
# crt0 (build-all.sh, build-ppj.sh -> bin/, bin/j/). Runs, all copied from
# measurement-2026-10-13-arch03-acceptance (runs-e9.sh, runs-e9-extra.sh,
# runs-poste9.sh, pp-run.sh, batch2.sh, rerun-port10.sh, series-a13.sh) and
# measurement-2026-10-13-fs10 (fs10-series.sh):
#   STDOUTDB  the diagnosis probe (post-E9 diag-libc-main/stdoutdbg.c): stdout
#             fputs/printf through `_main` -> rc >= 0, errno 0 (was -1/EBADF)
#   FSEEKCON, ATEXITP, E9MEM, E9PLAIN, ACCFREE, VFORKT alone, ENVRN
#   PPARENT (one boot: AUTO PPAUTO, libc accessory PPACC.NAC through
#             `_acc_main`'s path + SYSACC.NAC, #Z PPZ) and PPZJ
#   S2ALL / S2ALLJ (ENVKIDJ); strict: S2ALLS / S2ALLSJ on the crash kernel
#   PORT10 (card files verified after seeding, rerun-port10.sh recipe)
#   desktop, TeraDesk, QED, Doom, APITEST, desktop + SYSACC (#Z, screendumps)
#   TeraDesk -> QED (tdkeys kernel), KBDVEC + EmuCON (keys kernel)
set -uo pipefail
L="${1:?}"; CR="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; BJ="$B/j"; K="$B/kernel8-$L.img"
MODES="${*:-on off strict}"
for m in $MODES; do
  echo "=== $m $(date)"
  M="$m"; . "$HERE/mode.sh"
  T="$L-$m"; C="$CR/$m"; mkdir -p "$C"
  sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
  sd stdoutdb 'C:\STDOUTDB.NAP'; sd fseekcon 'C:\FSEEKCON.NAP'; sd atexit 'C:\ATEXITP.NAP'
  sd e9mem 'C:\E9MEM.NAP'; sd e9plain 'C:\E9PLAIN.NCP'; sd vforkt 'C:\VFORKT.NAP'; sd envrn 'C:\ENVRN.NAP'
  sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
  sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd kbdvec 'C:\KBDVEC.NAP'
  [ -f "$C/acc.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/acc.img" - > /dev/null
  [ -f "$C/accfree.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/accfree.img" 'C:\ACCFREE.NAP' > /dev/null
  [ -f "$C/tdqed.img" ] || TDINF="$HERE/TERADESK-QED.INF" bash "$HERE/seed-card.sh" "$C/tdqed.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
  [ -f "$C/pparent.img" ] || SEED_ACC=1 EXTRA="AUTO/PPAUTO.NCP=$B/PPAUTO.NCP ACC/PPACC.NAC=$B/PPACC.NAC" \
    bash "$HERE/seed-card.sh" "$C/pparent.img" 'C:\PPZ.NAP' > /dev/null
  [ -f "$C/ppzj.img" ] || SEED_ACC=1 EXTRA="AUTO/PPAUTO.NCP=$B/PPAUTO.NCP ACC/PPACC.NAC=$B/PPACC.NAC PPZJ.NAP=$BJ/PPZJ.NAP" \
    bash "$HERE/seed-card.sh" "$C/ppzj.img" 'C:\PPZJ.NAP' > /dev/null
  S2=s2all; KS="$K"; [ "$BASE" = strict ] && { S2=s2alls; KS="$B/kernel8-crash-$L.img"; }
  U2="$(echo $S2 | tr a-z A-Z)"
  sd "$S2" "C:\\$U2.NAP"
  [ -f "$C/${S2}j.img" ] || EXTRA="ENVKID.NAP=$BJ/ENVKID.NAP" bash "$HERE/seed-card.sh" "$C/${S2}j.img" "C:\\$U2.NAP" > /dev/null
  export APPEND="$A4"
  r4() { bash "$HERE/run-rpi4.sh" "$@"; }
  rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
  rd "$K" "$C/stdoutdb.img" "$T-rpi4-stdoutdb" --end 'DBG: DONE' --end-delay 2 --timeout 200
  rd "$K" "$C/fseekcon.img" "$T-rpi4-fseekcon" --end 'FSEEKCON: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$K" "$C/atexit.img"   "$T-rpi4-atexit"   --end 'ATEXIT: (PASS|FAIL)' --end-delay 2 --timeout 240
  rd "$K" "$C/e9mem.img"    "$T-rpi4-e9mem"    --end '\[E9MEM\] E9MEM: (PASS|FAIL)' --end-delay 2 --timeout 240
  rd "$K" "$C/e9plain.img"  "$T-rpi4-e9plain"  --end 'E9PLAIN: (PASS|FAIL)' --end-delay 2 --timeout 240
  rd "$K" "$C/accfree.img"  "$T-rpi4-accfree"  --end 'ACCFREE: (PASS|FAIL)' --end-delay 2 --timeout 400
  rd "$K" "$C/vforkt.img"   "$T-rpi4-vforkt-alone" --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
  rd "$K" "$C/envrn.img"    "$T-rpi4-envrn"    --end 'ENVRN: (PASS|FAIL)' --end-delay 2 --timeout 240
  rd "$K" "$C/pparent.img"  "$T-rpi4-pparent"  --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 300
  rd "$K" "$C/ppzj.img"     "$T-rpi4-ppzj"     --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 300
  rd "$KS" "$C/$S2.img"     "$T-rpi4-$S2"      --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  rd "$KS" "$C/${S2}j.img"  "$T-rpi4-${S2}j"   --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  bash "$HERE/rerun-port10.sh" "$L" "$CR" "$m"
  r4 "$K" "$C/plain.img"    "$T-rpi4-desktop"  25:t25.png
  r4 "$K" "$C/acc.img"      "$T-rpi4-acc"      25:t25.png
  r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
  r4 "$K" "$C/qed.img"      "$T-rpi4-qed"      30:t30.png
  r4 "$K" "$C/doom.img"     "$T-rpi4-doom"     30:t30.png 45:t45.png
  r4 "$K" "$C/apitest.img"  "$T-rpi4-apitest"  25:t25.png 40:t40.png
  r4 "$B/kernel8-tdkeys-$L.img" "$C/tdqed.img" "$T-rpi4-tdqed" 30:t30.png 50:t50.png
  r4 "$B/kernel8-keys-$L.img" "$C/kbdvec.img" "$T-rpi4-kbdvec-emucon" 20:t20.png 35:t35.png 60:t60.png
done
echo "=== done $(date)"
