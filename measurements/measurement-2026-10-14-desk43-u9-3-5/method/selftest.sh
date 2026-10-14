#!/usr/bin/env bash
# selftest.sh -- DESK43 U9-4: `cargo xtask build --features selftest-desk` now builds NEWDESK with the self-tests
# (xtask PROGRAM_FEATURES); kernel kt (+ launch:NEWDESK=st-launch.script: Return for the DELETE box at +2/+7/+12 s),
# default boot = the EL0 NEWDESK, private copy of the plain card (the tests write to C:). Dumps at 60/90 s.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPEND="serial=on" bash "$HERE/run-rpi4.sh" "$HERE/../bin/kernel8-kt.img" /tmp/m5j/u5b/cards/plain.img st-el0-on-rpi4 60:t60.png 90:t90.png
