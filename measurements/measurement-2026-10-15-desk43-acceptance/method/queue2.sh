#!/usr/bin/env bash
cd /tmp/m5j/a15
M=/Volumes/Daten/Development/PiRusTOS/measurements/measurement-2026-10-15-desk43-acceptance/method
while pgrep -f rest.sh > /dev/null; do sleep 10; done
for m in on off strict verify; do bash $M/views2.sh $m; done
EXTRA_APPEND="input=aes" bash $M/views2.sh on -ia
bash $M/apps3.sh
bash $M/acc2.sh
bash $M/crash2.sh
bash $M/emucon2.sh
bash $M/cycles2.sh
bash $M/save2.sh on
bash $M/seq2.sh
bash $M/selftest2.sh
echo Q2DONE
