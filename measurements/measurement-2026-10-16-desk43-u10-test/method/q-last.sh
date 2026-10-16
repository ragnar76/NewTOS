#!/usr/bin/env bash
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
until grep -q ALL-DONE /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/rest.log; do sleep 5; done
bash "$HERE/apitest109b.sh"; echo API2-DONE
bash "$HERE/virt-bad.sh"; echo LAST-DONE
