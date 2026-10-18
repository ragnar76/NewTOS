#!/usr/bin/env bash
# series-n.sh -- interactive scenarios, head vs wt (one QEMU at a time): dn = NEWDESK folder navigation + launch from folders (c-nd),
# tn = TeraDesk folder navigation + launch (c-nt), qn = QED fsel navigation (c-nq); m = kd matrix (desk/acc/td/qed, as proc03-s1-test).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SC="$HERE/../scripts/n"; SM="$HERE/../scripts/m113"
export OUTDIR="$S/shots-n"; mkdir -p "$OUTDIR"
for side in head wt; do
  DL=1.0 TMO=150 bash "$HERE/m113/drive.sh" "$S/bin/kernel8-$side-kdn.img" "$S/img/c-nd.img" "dn-$side" kn desktop="$SC/dn.script"
  DL=1.0 TMO=150 bash "$HERE/m113/drive.sh" "$S/bin/kernel8-$side-ktn.img" "$S/img/c-nt.img" "tn-$side" kn up:DESKTOP="$SC/tn.script"
  DL=1.0 TMO=150 bash "$HERE/m113/drive.sh" "$S/bin/kernel8-$side-kqn.img" "$S/img/c-nq.img" "qn-$side" kn up:QED="$SC/qn.script"
done
