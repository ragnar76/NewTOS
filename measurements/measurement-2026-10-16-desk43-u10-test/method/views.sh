#!/usr/bin/env bash
# views.sh <mode> -- DESK43 U9-5: the 8 desktop views (views.script, U8b/U9-0 schedule) on ONE kernel
# (bin/kernel8-kv.img), once with the EL0 NEWDESK (default) and once with `desk=kernel` (the built-in EL1 desktop).
# Plain MEM05 card; screendumps at the phase midpoints of measurement-2026-10-14-desk43-u9-0/method/views-run.sh.
# Tags views-el0-<mode>-rpi4 / views-el1-<mode>-rpi4. cwd /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad (short QMP socket dir s/).
set -uo pipefail
m="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="$HERE/../bin/kernel8-kv.img"; C=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards
M="$m"; . "$HERE/mode.sh"
BASEARGS="$APPEND"
for p in el0 el1; do
  if [ $p = el1 ]; then export APPEND="$BASEARGS desk=kernel"; else export APPEND="$BASEARGS"; fi
  bash "$HERE/run-rpi4.sh" "$K" "$C/plain.img" "views-$p-$m-rpi4" 18:1-text.png 21:2-viewmenu.png 23.5:3-icons.png \
    28.5:4-about.png 35.5:5-prefs.png 42.5:6-background.png 49.5:7-config.png 53:8-end.png
done
