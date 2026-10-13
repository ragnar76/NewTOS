#!/usr/bin/env bash
# batch2.sh <label> <cards-root> <scratch-root> <mode...> -- A13 follow-up runs after series-a13.sh:
#  (a) desk3a + ppdesk for the mode (the first series start had a stale injector anchor, see RUNINFO),
#  (b) PPZJ: PPARENT Z with the libc-joined PATH= form accepted (PORT09 fa99b4d8),
#  (c) PPDESKJ: same for the desktop double-click,
#  (d) S2ALLJ / S2ALLSJ: S2ALL(S) with an ENVKID.NAP that checks the libc view (argc/argv/getenv),
#  (e) FSEEKCON: Fseek(0,h,SEEK_CUR) on the standard handles (open item: EmuTOS 0).
# Programs for (b)-(d) come from $BINJ (built by build-ppj.sh), (e) from bin/.
set -uo pipefail
L="$1"; C="$2"; R="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; BINJ="${BINJ:-/private/tmp/claude-501/n3/binj}"
for m in "$@"; do
  M="$m"; . "$HERE/mode.sh"
  T="$L-$m"; mkdir -p "$C/$m"
  rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
  if [ -z "${SKIP_DESK:-}" ]; then
    bash "$HERE/ppdesk-run.sh" $L "$R" "$m"
    bash "$HERE/desk3a-run.sh" $L "$R" "$m"
  fi
  [ -f "$C/$m/ppzj.img" ] || SEED_ACC=1 EXTRA="AUTO/PPAUTO.NCP=$B/PPAUTO.NCP ACC/PPACC.NAC=$B/PPACC.NAC PPZJ.NAP=$BINJ/PPZJ.NAP" \
    bash "$HERE/seed-card.sh" "$C/$m/ppzj.img" 'C:\PPZJ.NAP' > /dev/null
  rd "$B/kernel8-$L.img" "$C/$m/ppzj.img" "$T-rpi4-ppzj" --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 300
  [ -f "$R/cards/ppdeskj.img" ] || INF="$HERE/ppdesk/newdesk-ppdesk.inf" EXTRA="PPD/PPDESKJ.NAP=$BINJ/PPDESKJ.NAP" \
    bash "$HERE/seed-card.sh" "$R/cards/ppdeskj.img" - > /dev/null
  cp "$R/cards/ppdeskj.img" "$R/cards/run-ppdeskj.img"
  bash "$HERE/run-marks.sh" "$B/kernel8-ppdesk-$L.img" "$R/cards/run-ppdeskj.img" "$HERE/../screendumps" "$T-rpi4-ppdeskj" 150 \
    "MARK A_IDLE:$T-ppdeskj-A_IDLE.png:0.5" "MARK B_BACK:$T-ppdeskj-B_BACK.png:0.5" || true
  mv "$HERE/../screendumps/$T-rpi4-ppdeskj-serial.log" "$HERE/../serial/" 2>/dev/null || true
  S2=s2all; K="$B/kernel8-$L.img"; [ "$BASE" = strict ] && { S2=s2alls; K="$B/kernel8-crash-$L.img"; }
  [ -f "$C/$m/${S2}j.img" ] || EXTRA="ENVKID.NAP=$BINJ/ENVKID.NAP" bash "$HERE/seed-card.sh" "$C/$m/${S2}j.img" "C:\\$(echo $S2 | tr a-z A-Z).NAP" > /dev/null
  rd "$K" "$C/$m/${S2}j.img" "$T-rpi4-${S2}j" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  [ -f "$C/$m/fseekcon.img" ] || bash "$HERE/seed-card.sh" "$C/$m/fseekcon.img" 'C:\FSEEKCON.NAP' > /dev/null
  rd "$B/kernel8-$L.img" "$C/$m/fseekcon.img" "$T-rpi4-fseekcon" --end 'FSEEKCON: (PASS|FAIL)' --end-delay 2 --timeout 200
done
echo "=== batch2 done $(date)"
