#!/usr/bin/env bash
# stderr-matrix-aux.sh <cards-root> [modes...] -- the stderr matrix after the AUX follow-up (ba3dccb8).
# Programs: STDERRP (tests/stderrp.c; GEM #Z, argv[0] empty + spawned kid argv[0]="STDERRP") in two crt0
# variants (new = mintlib _main, old = bin/oldcrt0/STDERRP.NAP, no Fforce(2,-1)), and STDERRA (ARGV kid,
# new crt0, handle 2 = AUX inherited).
# Kernels: aux = HEAD 70192a53 (AUX present, no cable); pre = fb84e64a (FS11, AUX absent; red control, hang
# expected for old crt0 + STDERRA); prefs11 = 7bb9579a (before FS11; old behaviour: stderr to console).
set -uo pipefail
C="${1:?}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"; D="$C/$m"; mkdir -p "$D"
  [ -f "$D/serr-newcrt0.img" ] || bash "$HERE/seed-card.sh" "$D/serr-newcrt0.img" 'C:\STDERRP.NAP' >/dev/null
  [ -f "$D/serr-oldcrt0.img" ] || EXTRA="STDERRP.NAP=$B/oldcrt0/STDERRP.NAP" bash "$HERE/seed-card.sh" "$D/serr-oldcrt0.img" 'C:\STDERRP.NAP' >/dev/null
  [ -f "$D/stderra.img" ] || bash "$HERE/seed-card.sh" "$D/stderra.img" 'C:\STDERRA.NAP' >/dev/null
  for k in aux pre prefs11; do
    case $k in aux) K="$B/kernel8-ax.img"; TO=150;; pre) K="$B/kernel8-pre.img"; TO=90;; prefs11) K="$B/kernel8-prefs11.img"; TO=150;; esac
    for c in newcrt0 oldcrt0; do
      bash "$HERE/run-rpi4-drive.sh" "$K" "$D/serr-$c.img" "ac-$m-rpi4-stderr-${k}kernel-$c" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout $TO
    done
    bash "$HERE/run-rpi4-drive.sh" "$K" "$D/stderra.img" "ac-$m-rpi4-stderra-${k}kernel" --end 'STDERRA\[PAR\]: DONE' --end-delay 3 --timeout $TO
  done
done
