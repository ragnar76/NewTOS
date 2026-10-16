#!/usr/bin/env bash
# probes.sh <mode>... -- LIBC-FSTAT serial-verdict runs on the product kernel bin/kernel8-lf.img (checks.sh lf), EL0 default
# desktop, QEMU raspi4b, one QEMU at a time, marker-keyed dumps (mrun.sh/mdrive.py). Cards from cards.sh: B = relinked against the
# LIBC-FSTAT libc.a, A (tag prefix a-) = the pre-LIBC-FSTAT programs. Tags p-<mode>-<card>.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf/cards; K="${KERNEL:-$HERE/../bin/kernel8-lf.img}"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"
  for side in "" a-; do
    bash "$HERE/mrun.sh" "$K" "$C/${side}fstatp.img"       "p-$m-${side}fstatp"       'FSTATP\[PAR\]: DONE@1=3:t.png'
    bash "$HERE/mrun.sh" "$K" "$C/${side}stdoutdb.img"     "p-$m-${side}stdoutdb"     'DBG: DONE@1=2:t.png'
    bash "$HERE/mrun.sh" "$K" "$C/${side}serr-newcrt0.img" "p-$m-${side}serr-newcrt0" 'STDERRP\[PAR\]: DONE@1=3:t.png'
    bash "$HERE/mrun.sh" "$K" "$C/${side}serr-oldcrt0.img" "p-$m-${side}serr-oldcrt0" 'STDERRP\[PAR\]: DONE@1=3:t.png'
    bash "$HERE/mrun.sh" "$K" "$C/${side}stderra.img"      "p-$m-${side}stderra"      'STDERRA\[PAR\]: DONE@1=3:t.png'
    bash "$HERE/mrun.sh" "$K" "$C/${side}ttya.img"         "p-$m-${side}ttya"         'TTYPROBE: (PASS|FAIL)@1=2:t.png'
  done
  bash "$HERE/mrun.sh" "$K" "$C/doom.img" "p-$m-doom" 'sh_ldapp: starting C:.DOOM@1=30:t30.png'
done
