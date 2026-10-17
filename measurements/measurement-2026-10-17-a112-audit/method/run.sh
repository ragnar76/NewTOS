#!/bin/bash
# usage: run.sh <base.img> <log> <secs>
S=$(dirname $0)
cp -c $1 $S/runtmp.img
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:$2 -kernel $S/kernel8.img -append serial=on -drive file=$S/runtmp.img,if=sd,format=raw &
pid=$!; sleep $3; kill $pid 2>/dev/null; wait $pid 2>/dev/null
rm -f $S/runtmp.img
