#!/usr/bin/env bash
# t16.sh <kernel-id> <tag> -- T16 proof: card f1hg (C:\HELLOGEM.NAP on F1; /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u935/cards/f1hg.img,
# derived by measurement-2026-10-14-desk43-u9-3-5/method/make-cards.sh), EL0 default desktop, mode on. F1 at desktop up +1 s
# (f1.script) -> NEWDESK shel_write + Pterm -> HELLOGEM published in the SAME slot 0 -> launch:HELLOGEM must arm -> Return at +8 s
# (ret.script) -> HELLOGEM quits -> NEWDESK up again. Dumps: 4 s after HELLOGEM's launch, 4 s after the 2nd "desktop up".
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M=on; . "$HERE/mode.sh"
TMO="${TMO:-60}" bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-$1.img" /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u935/cards/f1hg.img "$2" \
  '\[launch\] EL0: slot@2=4:app.png' '\[desk\] desktop up@2=4:end.png'
