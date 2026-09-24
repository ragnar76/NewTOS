#!/usr/bin/env bash
# run_launch_many.sh <run20-scratch> <method-dir> <first> <last>
# Drive the DESK19 drag-launch script (method/d19-full.rs) N times on one
# built kernel, each run on its own fresh copy of the seeded card image.
# Markers: A_IDLE / B_WIN from the injector, then the launched CMDLINE.NCP's
# own output line, which is the proof the launch completed.
set -uo pipefail
SCR="$1"; M="$2"; FIRST="$3"; LAST="$4"
for i in $(seq -w "$FIRST" "$LAST"); do
  cp "$SCR/tmp/base.img" "$SCR/tmp/r$i.img"
  bash "$M/run.sh" "$SCR/tree/kernel8.img" "$SCR/tmp/r$i.img" "$SCR/tmp/out" "run$i" 150 \
      "MARK A_IDLE:run$i-a-idle.png:1" \
      "MARK B_WIN:run$i-b-win.png:1" \
      "len 13 arg:run$i-c-cmdline.png:1"
  rm -f "$SCR/tmp/r$i.img"
  n=$(/usr/bin/grep -cE '\[EXC\]|\[PANIC\]' "$SCR/tmp/out/run$i-serial.log")
  echo "run$i faults=$n"
done
