#!/usr/bin/env bash
# crash3.sh -- U10 test: desktop crash policy. kp: NEWDESK panics at start (EL0 only path left), on/off/strict;
# kc / km (rpi4): NEWDESK blob with zeroed ELF header / no registry entry -> D6-d: EmuCON, exactly one load attempt. Plain card.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; P=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/plain.img
for m in on off strict; do
  bash "$HERE/pair2.sh" kp "$P" kp $m 'Press any key@1=3:prompt1.png' '\[kp\] launch:NEWDESK feed@1=4:after-key.png' 'Press any key@2=3:prompt2.png' 'Press any key@2=25:end.png'
done
bash "$HERE/pair2.sh" kc "$P" kc on 'No desktop: NEWDESK@1=6:fallback.png' 'No desktop: NEWDESK@1=25:end.png'
bash "$HERE/pair2.sh" km "$P" km on 'No desktop: NEWDESK@1=6:fallback.png' 'No desktop: NEWDESK@1=25:end.png'
