#!/usr/bin/env bash
# virt.sh -- DESK43 U9-3/4: qemu-virt (run-virt.sh + drive.py, U9-1-2 copies), product flat image bin/kernel-virt-acc.img,
# memprot on/off/strict (mode.sh AV), EL0 NEWDESK (tag acc-<mode>-virt) and desk=kernel (acck-<mode>-virt); the driver
# ends 6 s after "desktop up" with <tag>-end.png. cwd /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in on off strict; do
  M="$m"; . "$HERE/mode.sh"
  APPEND="$AV" bash "$HERE/run-virt.sh" "$HERE/../bin/kernel-virt-acc.img" "acc-$m-virt-desktop" --timeout 120
  APPEND="$(echo "$AV desk=kernel" | xargs)" bash "$HERE/run-virt.sh" "$HERE/../bin/kernel-virt-acc.img" "acck-$m-virt-desktop" --timeout 120
done
