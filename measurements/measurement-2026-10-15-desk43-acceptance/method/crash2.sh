#!/usr/bin/env bash
# crash2.sh -- desktop crash policy D6, marker-keyed (cf. method/crash.sh). kp: NEWDESK panics at start (injector tag kp: one Return 8 s after the
# first launch); kc: blob with zeroed ELF header; km: no registry entry NEWDESK; card f1hg (F1 -> HELLOGEM).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/tmp/m5j/a15/cards; P=/tmp/m5j/u5b/cards/plain.img
for m in on off strict; do
  PATHS=el0 bash "$HERE/pair2.sh" kp "$P" kp $m 'Press any key@1=3:prompt1.png' '\[kp\] launch:NEWDESK feed@1=4:after-key.png' 'Press any key@2=3:prompt2.png' 'Press any key@2=25:end.png'
done
PATHS=el1 bash "$HERE/pair2.sh" kp "$P" kp on '\[desk\] desktop up@1=8:t.png'
bash "$HERE/pair2.sh" kc "$C/f1hg.img" kc on 'falling back@1=6:fallback.png' 'starting C:\\HELLOGEM@1=5:app.png' '\[desk\] desktop up@2=4:end.png'
bash "$HERE/pair2.sh" km "$C/f1hg.img" km on 'falling back@1=6:fallback.png' 'starting C:\\HELLOGEM@1=5:app.png' '\[desk\] desktop up@2=4:end.png'
