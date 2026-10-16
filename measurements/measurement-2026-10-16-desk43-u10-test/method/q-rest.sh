#!/usr/bin/env bash
# q-rest.sh -- everything after the series, one QEMU at a time. cwd = scratch.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
until grep -q SERIES-DONE /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/series.log; do sleep 5; done
bash "$HERE/virt.sh"; bash "$HERE/virt3.sh"; echo VIRT-DONE
bash "$HERE/apitest109.sh"; echo API-DONE
for m in on off strict verify; do bash "$HERE/views2.sh" $m; done
EXTRA_APPEND=input=aes bash "$HERE/views2.sh" on -ia; echo VIEWS-DONE
bash "$HERE/apps3.sh" 2>&1; echo APPS-DONE
bash "$HERE/acc2.sh"; bash "$HERE/emucon2.sh"; bash "$HERE/cycles2.sh"; echo ACC-EMU-CYC-DONE
bash "$HERE/save2.sh" on
bash "$HERE/derive-card.sh" /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/save/saved-el0-on.img /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/f1crchw.img '#G 03 FF   C:\CRCHILD.NAP@ @ 001 @' > /dev/null 2>&1
bash "$HERE/crchw2.sh"; bash "$HERE/readinf3.sh" on; bash "$HERE/tdqed2.sh" on; bash "$HERE/selftest2.sh"; echo MISC-DONE
bash "$HERE/crash3.sh"; echo ALL-DONE
