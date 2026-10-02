#!/usr/bin/env bash
#
# run-port10.sh <scratch-dir> <outdir>
#
# TASK/PORT10, user decision 2026-10-02 (2): the executable bit of
# stat()/access() follows NewTOS's native extensions .NTP/.NAP/.NCP,
# case-insensitively (overlay sdk/libc/overlay/mintlib/quickstat.c).
# Recipe of measurement-2026-10-02-port10c/method/run-port10.sh (that file
# stays as it was), plus three card files for port10.c's section 5:
#   - sdk/examples/port10.c    -> C:\AUTO\P10TEST.NCP
#   - sdk/examples/port10kid.c -> C:\P10\P10KID.NCP and, same file,
#                                 C:\P10\P10KID.NTP, C:\P10\ONLYNCP.NCP,
#                                 C:\P10\LEGACY.PRG, C:\P10\APP.NAP,
#                                 C:\P10\ACC.NAC, C:\P10\PLAIN.TXT
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
    P10/ONLYNCP.NCP="$SCR/P10KID.NCP" P10/LEGACY.PRG="$SCR/P10KID.NCP" \
    P10/APP.NAP="$SCR/P10KID.NCP" P10/ACC.NAC="$SCR/P10KID.NCP" \
    P10/PLAIN.TXT="$SCR/P10KID.NCP"
"$HERE/run-serial.sh" "$KERNEL" "$SCR/p10.img" "$OUT/port10-serial.log" 60
grep -a -E 'port10|p10kid|EXC|FAR' "$OUT/port10-serial.log"
