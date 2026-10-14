#!/usr/bin/env bash
# apps2.sh -- DESK43 U9-5: the F1 runs whose program needs a key, again on kernel kf2 (inject-arm-u935.py: launch: arms
# fire at the launcher's slot publication). On kf the EL0-path runs never got their key: NEWDESK (slot 0) ends and the
# program starts in slot 0 within one injector tick, the tick-side poll of tools/inject-arm.py saw no new child (RUNINFO).
# Tags f1hg2/f1hgp2/f1crch2 (on/off/strict) and f1crash2 (on), EL0 NEWDESK and desk=kernel.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/tmp/m5j/u935/cards
b() { bash "$HERE/both.sh" "$@"; }
for m in on off strict; do
  b kf2 "$C/f1hg.img" f1hg2 $m 22:app.png 32:end.png
  b kf2 "$C/f1hgp.img" f1hgp2 $m 22:app.png 32:end.png
  b kf2 "$C/f1crch.img" f1crch2 $m 22:app.png 32:end.png
done
b kf2 "$C/f1crash.img" f1crash2 on 30:app.png 65:end.png
