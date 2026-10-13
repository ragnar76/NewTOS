#!/usr/bin/env bash
# stderr-matrix.sh <cards-root> [modes...] -- does stderr of libc programs still
# reach the console? STDERRP (tests/stderrp.c) as GEM #Z program (argv[0] empty)
# that also spawns itself as a child via spawnl (argv[0]="STDERRP"). 2x2:
#   crt0: new (mintlib _main, HEAD) / old (crt0-before-libc-main.S)
#   kernel: new (HEAD, FS11) / old (7bb9579a = before FS11 code)
set -uo pipefail
C="${1:?}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"; D="$C/$m"; mkdir -p "$D"
  [ -f "$D/serr-newcrt0.img" ] || bash "$HERE/seed-card.sh" "$D/serr-newcrt0.img" 'C:\STDERRP.NAP' >/dev/null
  [ -f "$D/serr-oldcrt0.img" ] || EXTRA="STDERRP.NAP=$B/oldcrt0/STDERRP.NAP" bash "$HERE/seed-card.sh" "$D/serr-oldcrt0.img" 'C:\STDERRP.NAP' >/dev/null
  for k in new old; do
    if [ $k = new ]; then K="$B/kernel8-lm.img"; else K="$B/kernel8-prefs11-lm.img"; fi
    for c in newcrt0 oldcrt0; do
      bash "$HERE/run-rpi4-drive.sh" "$K" "$D/serr-$c.img" "lm-$m-rpi4-stderr-${k}kernel-$c" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
    done
  done
done
