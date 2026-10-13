#!/usr/bin/env bash
# rerun-j.sh <label> <cards-root> [modes...] -- the runs of cm-series.sh that
# needed bin/j (PPZJ, ENVKIDJ), which was not built yet when the first series
# started (build-ppj.sh/build-envkidm.sh had been called without <outdir>):
# PPZJ, S2ALLJ / S2ALLSJ (+ cm-batch2.sh for ENVKIDM). Cards are re-seeded.
set -uo pipefail
L="${1:?}"; CR="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; BJ="$B/j"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"
  T="$L-$m"; C="$CR/$m"; mkdir -p "$C"
  S2=s2all; KS="$B/kernel8-$L.img"; [ "$BASE" = strict ] && { S2=s2alls; KS="$B/kernel8-crash-$L.img"; }
  U2="$(echo $S2 | tr a-z A-Z)"
  rm -f "$C/ppzj.img" "$C/${S2}j.img" "$C/${S2}m.img"
  SEED_ACC=1 EXTRA="AUTO/PPAUTO.NCP=$B/PPAUTO.NCP ACC/PPACC.NAC=$B/PPACC.NAC PPZJ.NAP=$BJ/PPZJ.NAP" \
    bash "$HERE/seed-card.sh" "$C/ppzj.img" 'C:\PPZJ.NAP' > /dev/null
  EXTRA="ENVKID.NAP=$BJ/ENVKID.NAP" bash "$HERE/seed-card.sh" "$C/${S2}j.img" "C:\\$U2.NAP" > /dev/null
  bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-$L.img" "$C/ppzj.img" "$T-rpi4-ppzj" --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 300
  bash "$HERE/run-rpi4-drive.sh" "$KS" "$C/${S2}j.img" "$T-rpi4-${S2}j" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
done
bash "$HERE/cm-batch2.sh" "$L" "$CR" ${*:-on off strict}
