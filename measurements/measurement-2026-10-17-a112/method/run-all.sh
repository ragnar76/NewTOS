#!/usr/bin/env bash
# run-all.sh <scratch-dir> -- A112: probe A112MESG.NAP without and with the companion accessory A112ACC.NAC,
# memprot on and off, QEMU raspi4b. One QEMU at a time; per-run card = APFS clone (cp -c) of a base, deleted after.
set -uo pipefail
W="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; BIN="$HERE/../bin"
mkdir -p "$W/s"; cd "$W"
# probe builds: default (no accessory) and -DA112_ACC (one-line wrapper); the accessory
SDK="$HERE/../../../sdk"
bash "$SDK/build.sh" "$SDK/examples/a112mesg.c" "$BIN/A112MESG.NAP" >/dev/null
printf '#define A112_ACC 1\n#include "%s"\n' "$SDK/examples/a112mesg.c" > "$W/a112mesga.c"
bash "$SDK/build.sh" "$W/a112mesga.c" "$BIN/A112MSGA.NAP" >/dev/null
bash "$SDK/build.sh" "$SDK/examples/a112acc.c" "$BIN/A112ACC.NAC" >/dev/null
bash "$HERE/make-card.sh" "$W/a112noacc.img" >/dev/null
bash "$HERE/make-card.sh" "$W/a112acc.img" acc >/dev/null
for run in noacc-on noacc-off acc-on acc-off; do
  M="${run#*-}"; card="$W/a112${run%%-*}.img"
  export M; . "$HERE/mode.sh"; echo "== $run append=$APPEND"
  bash "$HERE/run-rpi4.sh" "$BIN/kernel8-a112.img" "$card" a112-$run 40:shot
done
rm -f "$W/a112noacc.img" "$W/a112acc.img"
