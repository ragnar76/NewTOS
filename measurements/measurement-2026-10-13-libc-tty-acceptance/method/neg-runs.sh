#!/usr/bin/env bash
# neg-runs.sh <label> <cards-root> -- ARCH03-4 negative controls (kernels from
# asid-kernels.sh, deliberately broken; see asid-instrumentation.py):
#   kernel8-neg-notlbi: ASIDs off, restore_boot_ttbr0 without TLBI,
#     memprot=off, ASIDSTRO + accessory  -> expected FAIL (stale page 0)
#   kernel8-neg-noinval: asid=on, no per-VA invalidation,
#     MPROT3B -> expected FAIL in case 'f' (same ASID);
#     ASIDSTR -> passes on QEMU (inactive-ASID case not observable there)
set -uo pipefail
L="$1"; C="$2/neg"; mkdir -p "$C"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
[ -f "$C/asidstro.img" ] || EXTRA="ACC/ASIDACC.NAC=$B/ASIDACC.NAC" bash "$HERE/seed-card.sh" "$C/asidstro.img" 'C:\ASIDSTRO.NAP' > /dev/null
[ -f "$C/asidstr.img" ] || EXTRA="ACC/ASIDACC.NAC=$B/ASIDACC.NAC" bash "$HERE/seed-card.sh" "$C/asidstr.img" 'C:\ASIDSTR.NAP' > /dev/null
[ -f "$C/mprot.img" ] || bash "$HERE/seed-card.sh" "$C/mprot.img" 'C:\MPROT3B.NAP' > /dev/null
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
APPEND="serial=on memprot=off" rd "$B/kernel8-neg-notlbi-$L.img" "$C/asidstro.img" "neg-notlbi-off-rpi4-asidstr" --end 'ASIDSTR: (PASS|FAIL)' --end-delay 3 --timeout 400
APPEND="serial=on asid=on" rd "$B/kernel8-neg-noinval-$L.img" "$C/mprot.img" "neg-noinval-on-asid-rpi4-mprot" --end 'MPROT3B: (PASS|FAIL)' --end-delay 3 --timeout 400
APPEND="serial=on asid=on" rd "$B/kernel8-neg-noinval-$L.img" "$C/asidstr.img" "neg-noinval-on-asid-rpi4-asidstr" --end 'ASIDSTR: (PASS|FAIL)' --end-delay 3 --timeout 400
