#!/usr/bin/env bash
# stderr-emucon.sh <cards-root> [modes...] -- EmuCON session typing "run stderrq.nap"
# (new crt0) and "run stderro.nap" (old crt0), screendumps at +35 s and +60 s, on the
# keys kernel of HEAD (new) and of 7bb9579a (old = before FS11).
set -uo pipefail
C="${1:?}"; shift; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"; mkdir -p "$C/$m"
  [ -f "$C/$m/kbd.img" ] || bash "$HERE/seed-card.sh" "$C/$m/kbd.img" 'C:\KBDVEC.NAP' > /dev/null
  for k in new old; do
    bash "$HERE/run-rpi4.sh" "$B/kernel8-keys-stderr-$k.img" "$C/$m/kbd.img" "lm-$m-rpi4-stderr-emucon-${k}kernel" 25:t25.png 35:t35.png 60:t60.png
  done
done
