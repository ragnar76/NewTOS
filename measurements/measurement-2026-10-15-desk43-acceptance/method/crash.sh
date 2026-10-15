#!/usr/bin/env bash
# crash.sh -- DESK43 U9-4 crash policy D6 (prep §1.3, §7 item 4), QEMU raspi4b:
#   kp  NEWDESK panics at startup; launch:NEWDESK=key.script feeds ONE Return 8 s after the first launch.
#       Expected: report + "Press any key" prompt, key -> rc=65535 -> wm_new -> NEWDESK started again -> panics -> prompt
#       again and WAITS (no second key): two launches in ~45 s, no tight loop. memprot on/off/strict; desk=kernel control.
#   kc  embedded blob with a zeroed ELF header (EPLFMT), card f1hg: ONE load attempt, the fallback line, the built-in
#       desktop; F1 -> HELLOGEM -> Return -> back to the built-in desktop WITHOUT a second load attempt.
#   km  no registry entry NEWDESK: the shell's name falls through to the FAT path (no C:\NEWDESK) -> EFILNF, same checks.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/tmp/m5j/a15/cards; P=/tmp/m5j/u5b/cards/plain.img
for m in on off strict; do PATHS=el0 bash "$HERE/both.sh" kp "$P" kp $m 20:prompt1.png 30:after-key.png 45:end.png; done
PATHS=el1 bash "$HERE/both.sh" kp "$P" kp on 25:t25.png
for id in kc km; do bash "$HERE/both.sh" $id "$C/f1hg.img" $id on 22:app.png 34:end.png; done
