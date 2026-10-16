#!/usr/bin/env bash
# selftest2.sh -- kernel kt (--features selftest-desk -> NEWDESK built with the self-tests; launch:NEWDESK=st-launch.script), default boot (EL0 NEWDESK), plain card, marker "[desk-selftest] done".
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cp /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/plain.img /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/st-plain.img
APPEND="serial=on" TMO=300 bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-kt.img" /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/st-plain.img st-el0-on '\[desk-selftest\] done@1=3:t.png'
