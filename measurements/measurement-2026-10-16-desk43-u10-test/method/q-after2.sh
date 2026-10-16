#!/usr/bin/env bash
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
until grep -q OLD-DONE /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/after.log; do sleep 5; done
bash "$HERE/old-controls.sh"; echo OLD2-DONE
