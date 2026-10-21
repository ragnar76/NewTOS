#!/usr/bin/env bash
# run-coremark.sh -- CoreMark's validation run (sdk/apps/coremark/build/COREVAL.NCP as rebuilt by app-builds.sh against the
# SDK01 J libc) from C:\AUTO on kernel8-plain, memprot on, 240 s max, ends on "desktop up"; serial -> ../serial/coreval.log.
# Its report goes through mintlib's printf (ee_printf = printf, core_portme.h), incl. a double "%f" (Iterations/Sec).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
DIRS="" bash "$HERE/mkcard.sh" "$S/cards/cv.img" "$HERE/ldp.inf" KERNEL8.IMG="$S/mk/KERNEL8.IMG@202610211000" \
  AUTO/COREVAL.NCP="$WS/sdk/apps/coremark/build/COREVAL.NCP" > "$S/cards/cv.log" 2>&1
FSCK=1 bash "$HERE/run-probe-qemu.sh" "$S/bin/kernel8-plain.img" "$S/cards/cv.img" coreval "serial=on" 'desktop up' 240
grep -aE "CoreMark|Correct|Errors|Iterations|seedcrc|crc|Total ticks|Total time" "$HERE/../serial/coreval.log" | tr -d '\r'
