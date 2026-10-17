#!/usr/bin/env bash
# matrix.sh -- the interactive regression matrix: kernels kd-head (HEAD d21a6b21) and kd-green (working tree), cards desk / acc / accA / td / qed.
#   desk, acc, accA: desktop=s1.script (window, folder, select, View/Options/Desk menus, dialogs, close, accessory, 15 s menu hold)
#   td : TeraDesk by #Z, launch:DESKTOP=td.script, then (after Quit) the desktop arm s1.script
#   qed: QED by #Z, launch:QED=q.script,           then (after Ctrl-Q) the desktop arm s1.script
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SC="$HERE/../scripts"
for card in ${CARDS:-desk acc accA td qed}; do
  for side in ${SIDES:-head green}; do
    case $card in
      td)  A=(launch:DESKTOP=$SC/td.script desktop=$SC/s1.script);;
      qed) A=(launch:QED=$SC/q.script desktop=$SC/s1.script);;
      *)   A=(desktop=$SC/s1.script);;
    esac
    TMO=150 bash "$HERE/drive.sh" "$S/bin/kernel8-kd-$side.img" "$S/img/card-$card.img" "$card-$side" kd "${A[@]}"
  done
done
