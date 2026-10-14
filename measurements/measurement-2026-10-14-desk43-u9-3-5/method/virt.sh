#!/usr/bin/env bash
# virt.sh -- DESK43 U9-3/4: qemu-virt (run-virt.sh + drive.py, U9-1-2 copies), product flat image bin/kernel-virt-u935.img,
# memprot on/off/strict (mode.sh AV), EL0 NEWDESK (tag u935-<mode>-virt) and desk=kernel (u935k-<mode>-virt); the driver
# ends 6 s after "desktop up" with <tag>-end.png. cwd /tmp/m5j/u935.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in on off strict; do
  M="$m"; . "$HERE/mode.sh"
  APPEND="$AV" bash "$HERE/run-virt.sh" "$HERE/../bin/kernel-virt-u935.img" "u935-$m-virt-desktop" --timeout 120
  APPEND="$(echo "$AV desk=kernel" | xargs)" bash "$HERE/run-virt.sh" "$HERE/../bin/kernel-virt-u935.img" "u935k-$m-virt-desktop" --timeout 120
done
