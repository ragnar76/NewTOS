#!/usr/bin/env bash
#
# run-port10.sh <scratch-dir> <outdir>
#
# TASK/PORT10 native extension list (user decision 2026-10-02): spawnvp/
# system() resolve a name without extension with { ntp, nap, ncp }.
# Recipe of measurement-2026-10-02-port10b/method/run-port10.sh (that file
# stays as it was), plus two card files. Builds with the CURRENT tree (libc
# overlays incl. mintlib/spawnvp.c's native list in place):
#   - sdk/examples/port10.c    -> C:\AUTO\P10TEST.NCP  (the acceptance program,
#                                 now with the system() checks)
#   - sdk/examples/port10kid.c -> C:\P10\P10KID.NCP and, same file,
#                                 C:\P10\P10KID.NTP, C:\P10\ONLYNCP.NCP and
#                                 C:\P10\LEGACY.PRG (a decoy: if the search
#                                 still took .prg, it would run and return 42)
# The kernel is the unchanged `cargo xtask build --board rpi4 --release`
# image, passed in as $KERNEL (default: <repo>/kernel8.img).
set -euo pipefail
SCR="${1:?usage: run-port10.sh <scratch-dir> <outdir>}"; OUT="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
KERNEL="${KERNEL:-$WS/kernel8.img}"
mkdir -p "$SCR" "$OUT"
B="$WS/sdk/libc/build-libc.sh"
"$B" "$WS/sdk/examples/port10kid.c" "$SCR/P10KID.NCP" | grep -E '^LINK'
"$B" "$WS/sdk/examples/port10.c"    "$SCR/P10TEST.NCP" | grep -E '^LINK'
"$HERE/seed-auto.sh" "$SCR/p10.img" \
    AUTO/P10TEST.NCP="$SCR/P10TEST.NCP" \
    P10/P10KID.NCP="$SCR/P10KID.NCP" P10/P10KID.NTP="$SCR/P10KID.NCP" \
    P10/ONLYNCP.NCP="$SCR/P10KID.NCP" P10/LEGACY.PRG="$SCR/P10KID.NCP"
"$HERE/run-serial.sh" "$KERNEL" "$SCR/p10.img" "$OUT/port10-serial.log" 60
grep -a -E 'port10|p10kid|EXC|FAR' "$OUT/port10-serial.log"
