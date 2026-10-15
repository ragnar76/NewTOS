#!/usr/bin/env bash
# (DESK59 copy of measurement-2026-10-15-desk43-acceptance/method/selftest2.sh; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/d59; kernel id as $1 (kt-red / kt), tag st-<id>-el0-on.)
# selftest2.sh <kid> -- kernel bin/kernel8-<kid>.img (--features selftest-desk -> NEWDESK built with the self-tests; launch:NEWDESK=st-launch.script),
# default boot (EL0 NEWDESK), plain card, marker "[desk-selftest] done".
set -uo pipefail
K="${1:?kernel id}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cp /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards/plain.img /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/d59/st-plain.img
APPEND="serial=on" TMO=300 bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-$K.img" /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/d59/st-plain.img "st-$K-el0-on" '\[desk-selftest\] done@1=3:t.png'
