#!/usr/bin/env bash
# matrix.sh -- interactive regression matrix (copied from measurement-2026-10-17-a113-test/method/matrix.sh, paths adapted):
# kernels kd-head (b8245e5c) and kd-wt (+ uncommitted RST01), cards c-desk / c-acc / c-td / c-qed.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SC="$HERE/../../scripts/m113"
for card in ${CARDS:-desk acc td qed}; do
  for side in ${SIDES:-head wt}; do
    case $card in
      td)  A=(launch:DESKTOP=$SC/td.script desktop=$SC/s1.script);;
      qed) A=(launch:QED=$SC/q.script desktop=$SC/s1.script);;
      *)   A=(desktop=$SC/s1.script);;
    esac
    TMO=150 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-kd.img" "$S/img/c-$card.img" "$card-$side" kd "${A[@]}"
  done
done
