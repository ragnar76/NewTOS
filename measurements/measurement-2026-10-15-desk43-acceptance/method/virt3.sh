#!/usr/bin/env bash
# virt3.sh -- qemu-virt, mode on-asid, EL0 NEWDESK (tag acc-on-asid-virt-desktop) and desk=kernel (acck-...); as virt.sh.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="on-asid"; . "$HERE/mode.sh"
APPEND="$AV" bash "$HERE/run-virt.sh" "$HERE/../bin/kernel-virt-acc.img" "acc-$M-virt-desktop" --timeout 120
APPEND="$(echo "$AV desk=kernel" | xargs)" bash "$HERE/run-virt.sh" "$HERE/../bin/kernel-virt-acc.img" "acck-$M-virt-desktop" --timeout 120
