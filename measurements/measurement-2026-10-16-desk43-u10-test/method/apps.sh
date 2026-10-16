#!/usr/bin/env bash
# apps.sh -- DESK43 U9-5: programs started with F1 from the desktop (kernel kf: desktop=f1.script, Return scripts per
# program), EL0 NEWDESK and desk=kernel, memprot on (HELLOGEM/HGPANIC/CRCHILD also off/strict); the accessory card on
# kernel ka (Desk menu -> first accessory). Cards: make-cards.sh (/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards), /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/acc.img.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards
b() { bash "$HERE/both.sh" "$@"; }
for m in on off strict; do
  b kf "$C/f1hg.img" f1hg $m 22:app.png 32:end.png
  b kf "$C/f1hgp.img" f1hgp $m 22:app.png 32:end.png
  b kf "$C/f1crch.img" f1crch $m 22:app.png 32:end.png
done
b kf "$C/f1crash.img" f1crash on 30:app.png 65:end.png
b kf "$C/f1qed.img" f1qed on 30:app.png
b kf "$C/f1tera.img" f1tera on 35:app.png
b kf "$C/f1api.img" f1api on 30:t30.png 45:t45.png
b kf "$C/f1doom.img" f1doom on 35:t35.png 50:t50.png
for m in on off strict; do b ka /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/acc.img acc $m 15:desk.png 20:open.png 26:after.png; done
