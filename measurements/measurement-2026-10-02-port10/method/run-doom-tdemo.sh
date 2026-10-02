#!/usr/bin/env bash
#
# run-doom-tdemo.sh <scratch-dir> <outdir>
#
# DOOM `-timedemo demo1` with the PORT10 libc, to see DOOM's own end line
# "timed %i gametics in %i realtics (%f fps)" -- the printf that faulted in
# __printf_fp in PORT06. Uses the PORT06 method scripts UNCHANGED:
#   build-variants.sh -- compiles with sdk/apps/build-doom.sh and links with
#                        PORT06's diag_system.o, because system() still does
#                        not link (vfork, __waitval, __waittime -- see
#                        TASK/PORT10-STATUS-UNFINISHED.MD);
#   seed-doom.sh, run-doom.sh, qmp_timed.py -- card + headless QEMU raspi4b +
#                        QMP screendumps.
# The delivered build-doom.sh was ALSO run on its own; its log is
# method/build-doom-delivered.log (compiles 81/81, link stops on exactly
# vfork/__waitval/__waittime -- spawnvp, the PORT06 blocker, is gone).
set -euo pipefail
SCR="${1:?usage: run-doom-tdemo.sh <scratch-dir> <outdir>}"; OUT="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
P06="$WS/measurements/measurement-2026-10-02-port06/method"
KERNEL="${KERNEL:-$WS/kernel8.img}"
"$P06/build-variants.sh" "$SCR/doomvar"
"$P06/seed-doom.sh" "$SCR/doom.img" "$SCR/doomvar/tdemo/DOOM.NAP" \
    "$WS/Applications/doom-wad/doom1.wad"
"$P06/run-doom.sh" "$KERNEL" "$SCR/doom.img" "$OUT" tdemo \
    45:31-timedemo-t45.png 130:32-timedemo-end-t130.png
grep -a -E 'timed|DIAG|EXC|FAR|exited' "$OUT/tdemo-serial.log"
