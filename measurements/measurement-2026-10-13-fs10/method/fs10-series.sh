#!/usr/bin/env bash
# fs10-series.sh <label> <cards-root> [modes...] -- TASK/FS10 QEMU raspi4b runs
# per protection mode (mode.sh: on | off | strict, always serial=on):
#   FSEEKCON  Fseek(0,h,SEEK_CUR) on the standard handles 0..2 -> PASS = all 0
#   regression sample (scripts and #Z cards as in
#   measurement-2026-10-13-arch03-acceptance, runs-e9.sh / runs-poste9.sh):
#   desktop, TeraDesk, QED, Doom, APITEST (#Z, screendumps), S2ALL and
#   ATEXITP (serial verdicts), KBDVEC + EmuCON (keys kernel, screendumps).
set -uo pipefail
L="${1:?}"; C="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; K="$B/kernel8-$L.img"
MODES="${*:-on off strict}"
for m in $MODES; do
  echo "=== $m $(date)"
  M="$m"; . "$HERE/mode.sh"
  T="$L-$m"; D="$C/$m"; mkdir -p "$D"
  sd() { [ -f "$D/$1.img" ] || bash "$HERE/seed-card.sh" "$D/$1.img" "$2" > /dev/null; }
  sd fseekcon 'C:\FSEEKCON.NAP'
  sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
  sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd s2all 'C:\S2ALL.NAP'
  sd atexit 'C:\ATEXITP.NAP'; sd kbdvec 'C:\KBDVEC.NAP'
  export APPEND="$A4"
  r4() { bash "$HERE/run-rpi4.sh" "$@"; }
  rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
  rd "$K" "$D/fseekcon.img" "$T-rpi4-fseekcon" --end 'FSEEKCON: (PASS|FAIL)' --end-delay 2 --timeout 200
  r4 "$K" "$D/plain.img"    "$T-rpi4-desktop"  25:t25.png
  r4 "$K" "$D/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
  r4 "$K" "$D/qed.img"      "$T-rpi4-qed"      30:t30.png
  r4 "$K" "$D/doom.img"     "$T-rpi4-doom"     30:t30.png 45:t45.png
  r4 "$K" "$D/apitest.img"  "$T-rpi4-apitest"  25:t25.png 40:t40.png
  rd "$K" "$D/s2all.img"    "$T-rpi4-s2all"  --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 240
  rd "$K" "$D/atexit.img"   "$T-rpi4-atexit" --end 'ATEXIT: (PASS|FAIL)' --end-delay 2 --timeout 240
  r4 "$B/kernel8-keys-$L.img" "$D/kbdvec.img" "$T-rpi4-kbdvec-emucon" 20:t20.png 35:t35.png 60:t60.png
done
echo "=== done $(date)"
