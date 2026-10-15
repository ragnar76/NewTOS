#!/usr/bin/env bash
# selftest2.sh -- kernel kt (--features selftest-desk -> NEWDESK built with the self-tests; launch:NEWDESK=st-launch.script), default boot (EL0 NEWDESK), plain card, marker "[desk-selftest] done".
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cp /tmp/m5j/u5b/cards/plain.img /tmp/m5j/a15/st-plain.img
APPEND="serial=on" TMO=300 bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-kt.img" /tmp/m5j/a15/st-plain.img st-el0-on '\[desk-selftest\] done@1=3:t.png'
