#!/usr/bin/env bash
cd /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad
M=/Volumes/Daten/Development/PiRusTOS/measurements/measurement-2026-10-15-desk43-acceptance/method
for m in on off strict; do bash $M/probes2.sh $m el0 accfree s2allm e9mem stdoutdb stderr ttya port10 pparent mdpar; done
for m in on off; do bash $M/probes2.sh $m el1 accfree s2allm e9mem stdoutdb stderr ttya port10 pparent mdpar; done
bash $M/probes2.sh on el0 mdstep; bash $M/probes2.sh on el1 mdstep
for m in on-asid; do bash $M/probes2.sh $m el0 mdpar mdstep; done
PT=3000 bash $M/probes2.sh verify el0 accfree s2allm e9mem stdoutdb ttya mdpar
bash $M/virt2.sh
echo Q3DONE
