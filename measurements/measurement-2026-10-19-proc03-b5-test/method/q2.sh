#!/usr/bin/env bash
# q2.sh -- interactive batch (one QEMU at a time), head vs wt:
#  acc-menu: SYSACC opened from the Desk menu by an injected mouse click (acc.script) while idle (NEWDESK), while TeraDesk runs, while QED runs
#  (kernels acd/act/acq, cards dacc/tdqa), modes on/strict/off; marker-keyed screendumps (fd/inter/run.sh -> mdrive.py)
#  inter: marker-keyed delete/copy/save matrices of measurement-2026-10-19-proc04-fdelete-test (dsk tdd qsv dcm tcp), memprot default.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"; C="$S/cards"; I="$HERE/fd/inter"
read -ra NS <<< "$(python3 "$HERE/fx.py" "$HERE/acc.script" 510 1310)"
for side in ${SIDES:-head wt}; do for m in ${MODES:-on strict off}; do
  case $m in on) A="";; strict) A="memprot=strict";; off) A="memprot=off";; esac
  export APPEND_EXTRA="$A" TMO=150 QDELAY=2
  bash "$I/run.sh" "$B/kernel8-$side-acd.img" "$C/dacc.img" "am-$side-$m-idle" '\[desk\] desktop up@1=3:desk.png' "\\[inj\\] desktop feed@${NS[0]}=3:open.png" "\\[inj\\] desktop feed@${NS[1]}=3:after.png"
  bash "$I/run.sh" "$B/kernel8-$side-act.img" "$C/tdqa.img" "am-$side-$m-td" '\[inj\] up:DESKTOP armed@1=3:app.png' "\\[inj\\] up:DESKTOP feed@${NS[0]}=3:open.png" "\\[inj\\] up:DESKTOP feed@${NS[1]}=3:after.png"
  bash "$I/run.sh" "$B/kernel8-$side-acq.img" "$C/tdqa.img" "am-$side-$m-qed" '\[inj\] up:QED armed@1=3:app.png' "\\[inj\\] up:QED feed@${NS[0]}=3:open.png" "\\[inj\\] up:QED feed@${NS[1]}=3:after.png"
done; done
unset APPEND_EXTRA
for side in ${SIDES:-head wt}; do
  DL=0.8 TMO=150 bash "$I/drive.sh" "$B/kernel8-$side-dsk.img" "$C/dsk.img" "dsk-$side" inj desktop="$I/dsk.script"
  DL=0.8 TMO=150 bash "$I/drive.sh" "$B/kernel8-$side-tdd.img" "$C/tdd.img" "tdd-$side" inj up:DESKTOP="$I/tdd.script"
  LSDIRS="/ /APPS/QED" MTYPE="APPS/QED/QS.TXT APPS/QED/QS.BAK" DL=0.8 TMO=150 bash "$I/drive.sh" "$B/kernel8-$side-qsv.img" "$C/qsv.img" "qsv-$side" inj up:QED="$I/qsv.script"
  LSDIRS="/ /SUB /FOLD" DL=0.8 TMO=200 bash "$I/drive.sh" "$B/kernel8-$side-dcm.img" "$C/dcm.img" "dcm-$side" inj desktop="$I/dcm.script"
  LSDIRS="/ /SUB /FOLD" DL=0.8 TMO=200 bash "$I/drive.sh" "$B/kernel8-$side-tcp.img" "$C/tcp.img" "tcp-$side" inj up:DESKTOP="$I/tcp.script"
done
echo Q2-DONE
