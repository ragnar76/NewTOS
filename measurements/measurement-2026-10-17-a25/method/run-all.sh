#!/usr/bin/env bash
# run-all.sh <scratch-dir> -- A25: probe (re-call) under memprot on/off/strict + control (no re-call, on), QEMU raspi4b.
# Needs bin/kernel8-a25.img (cargo xtask build --board rpi4 --release in a scratch copy made by make-scratch.sh) and
# bin/A25REINI.NAP (sdk/build.sh sdk/examples/a25reinit.c). One QEMU at a time, per-run card = clone (cp -c) of the base, deleted after.
set -uo pipefail
W="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; BIN="$HERE/../bin"
mkdir -p "$W/s"; cd "$W"
sed 's|id2 = aes_call(10, 0, 1, 0); /\* appl_init, the re-call \*/|id2 = id1; /* CONTROL: no re-call */|' \
  "$HERE/../../../sdk/examples/a25reinit.c" > "$W/a25ctl.c"; grep -c CONTROL "$W/a25ctl.c"
bash "$HERE/../../../sdk/build.sh" "$W/a25ctl.c" "$W/A25CTL.NAP" >/dev/null
cp "$BIN/A25REINI.NAP" "$W/orig.NAP"
bash "$HERE/make-card.sh" "$W/a25base.img" >/dev/null
cp "$W/A25CTL.NAP" "$BIN/A25REINI.NAP"; bash "$HERE/make-card.sh" "$W/a25ctl.img" >/dev/null; cp "$W/orig.NAP" "$BIN/A25REINI.NAP"
for m in on off strict ctl-on; do
  case $m in ctl-on) M=on; card="$W/a25ctl.img";; *) M=$m; card="$W/a25base.img";; esac
  export M; . "$HERE/mode.sh"; echo "== $m append=$APPEND"
  KEEP=1 bash "$HERE/run-rpi4.sh" "$BIN/kernel8-a25.img" "$card" a25-$m 25:shot
done
