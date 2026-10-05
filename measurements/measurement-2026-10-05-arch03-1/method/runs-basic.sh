#!/usr/bin/env bash
# runs-basic.sh <label> <cards-dir> -- the product-kernel QEMU runs every
# sub-step repeats: raspi4b desktop / TeraDesk / QED / Doom / APITEST (#Z
# autorun from private card images, method/seed-card.sh) and the virt desktop.
set -uo pipefail
L="$1"; C="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="$HERE/../bin/kernel8-$L.img"; KV="$HERE/../bin/kernel-virt-$L.img"
sh "$HERE/run-rpi4.sh" "$K" "$C/plain.img"    "$L-rpi4-desktop"  25:t25.png
sh "$HERE/run-rpi4.sh" "$K" "$C/teradesk.img" "$L-rpi4-teradesk" 35:t35.png
sh "$HERE/run-rpi4.sh" "$K" "$C/qed.img"      "$L-rpi4-qed"      30:t30.png
sh "$HERE/run-rpi4.sh" "$K" "$C/doom.img"     "$L-rpi4-doom"     30:t30.png 45:t45.png
sh "$HERE/run-rpi4.sh" "$K" "$C/apitest.img"  "$L-rpi4-apitest"  25:t25.png 40:t40.png
sh "$HERE/run-virt.sh" "$KV" "$L-virt-desktop" --timeout 120
