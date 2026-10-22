#!/bin/bash
# scenarios.sh <tag head|base> -- the developer's qemu-virt scenarios (hubtree overfull mtp storage default), mouse move check each
S=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/651de586-7dd7-487b-9e92-c8525c58f508/scratchpad/usb01-test
M=/Volumes/Daten/Development/PiRusTOS/measurements/measurement-2026-10-23-usb01-test
V=$1
for T in default hubtree overfull mtp storage; do
  $M/method/run-virt.sh $S/bin/virt-dev-$V.img $M/serial/$V-$T.log 60 $T $M/method/qmp_mouse.py $S/shots $V-$T 2>&1 | grep -v "^screendump"
  python3 -I $M/method/pixdiff.py $S/shots/$V-$T-0-before.png $S/shots/$V-$T-1-after.png | head -1 | sed "s/^/$V $T mouse: /"
done
