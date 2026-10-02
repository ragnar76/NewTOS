#!/usr/bin/env bash
#
# run-doom.sh <scratch-dir> <outdir>
#
# DOOM built by the DELIVERED sdk/apps/build-doom.sh alone -- no diagnostic
# system() substitute (PORT06's diag_system.o is gone from the link) -- in
# two runs on headless QEMU raspi4b:
#   tdemo   -timedemo demo1 with doom1.wad: the end line
#           "timed ... gametics in ... realtics (... fps)", then I_Quit; the
#           desktop comes back.
#   noiwad  the same binary on a card WITHOUT the WAD: D_FindIWAD fails,
#           I_Error -> ZenityErrorBox -> system("/usr/bin/zenity --help
#           >/dev/null 2>&1") through the real libc system(); it must come
#           back with an error, and DOOM must exit, not hang or fault.
# Card + QEMU + QMP: PORT06's run-doom.sh and qmp_timed.py, unchanged.
set -euo pipefail
SCR="${1:?usage: run-doom.sh <scratch-dir> <outdir>}"; OUT="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
P06="$WS/measurements/measurement-2026-10-02-port06/method"
KERNEL="${KERNEL:-$WS/kernel8.img}"
mkdir -p "$SCR" "$OUT"
BUILD="$SCR/doom-tdemo" DOOM_STATS=5 DOOM_ARGS='"-mb","2","-timedemo","demo1"' \
    "$WS/sdk/apps/build-doom.sh" > "$OUT/build-doom.log" 2>&1
grep -E "compiled OK|LINK" "$OUT/build-doom.log"
NAP="$SCR/doom-tdemo/DOOM.NAP"
"$HERE/seed-doom.sh" "$SCR/doom.img" "$NAP" "$WS/Applications/doom-wad/doom1.wad"
"$P06/run-doom.sh" "$KERNEL" "$SCR/doom.img" "$OUT" tdemo \
    45:41-timedemo-t45.png 130:42-timedemo-end-t130.png
grep -a -E 'timed|DIAG|EXC|FAR|exited' "$OUT/tdemo-serial.log" || true
"$HERE/seed-doom.sh" "$SCR/noiwad.img" "$NAP"
"$P06/run-doom.sh" "$KERNEL" "$SCR/noiwad.img" "$OUT" noiwad \
    20:43-noiwad-t20.png 45:44-noiwad-t45.png
grep -a -E 'IWAD|zenity|null|Error|EXC|FAR|exited|desk' "$OUT/noiwad-serial.log" || true
