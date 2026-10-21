#!/bin/sh
# run-hv.sh NAME SECONDS [SHOTS...]: clone base.img, install HypView, boot raspi4b headless,
# take QMP screendumps at the given second marks, kill QEMU, delete the clone.
# Env: KERNEL (default run/kernel8.img), CFG (HYP_VIEW.CFG to install), EXTRA (files for C:\APPS\HYPVIEW).
set -u
S=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad
R=$S/run; NAME=$1; SECS=$2; shift 2
B=/Volumes/Daten/Development/PiRusTOS/sdk/apps/hypview/build
KERNEL=${KERNEL:-$R/kernel8.img}
OUT=$R/out-$NAME; mkdir -p $OUT
IMG=$R/clone-$NAME.img
rm -f $IMG; cp -c $R/base.img $IMG || exit 1
M="-i $IMG@@32256"
mmd $M ::/APPS/HYPVIEW 2>/dev/null
mcopy $M -o $B/HYP_VIEW.NAP $B/hyp_view.rsc ::/APPS/HYPVIEW/
[ -n "${CFG:-}" ] && mcopy $M -o "$CFG" ::/APPS/HYPVIEW/HYP_VIEW.CFG
for f in ${EXTRA:-}; do mcopy $M -o "$f" ::/APPS/HYPVIEW/; done
printf '#R 02\r\n#Z 01 C:\\APPS\\HYPVIEW\\HYP_VIEW.NAP@\r\n' > $OUT/NEWDESK.INF
mcopy $M -o $OUT/NEWDESK.INF ::/NEWDESK.INF
mdir $M ::/APPS/HYPVIEW > $OUT/dir.txt
cd $R
rm -f q-$NAME.sock
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:$OUT/serial.log \
  -qmp unix:q-$NAME.sock,server,nowait -kernel $KERNEL -append serial=on \
  -drive file=$IMG,if=sd,format=raw & QP=$!
t=0
for s in "$@"; do
  sleep $((s - t)); t=$s
  ./qmpshot q-$NAME.sock $OUT/shot-$s.ppm >/dev/null 2>&1 && sips -s format png $OUT/shot-$s.ppm --out $OUT/shot-$s.png >/dev/null 2>&1 && rm -f $OUT/shot-$s.ppm
done
[ $SECS -gt $t ] && sleep $((SECS - t))
kill $QP 2>/dev/null; wait $QP 2>/dev/null
rm -f q-$NAME.sock $IMG
ls $OUT
