#!/bin/bash
# usage: runts.sh <base.img> <log> <secs>  -- serial lines timestamped on the host
S=$(dirname $0)
cp -c $1 $S/runtmp.img
rm -f $S/fifo; mkfifo $S/fifo
perl -MTime::HiRes=time -ne 'BEGIN{$|=1;$t0=time} printf "%7.2f %s", time-$t0, $_' < $S/fifo > $2 &
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:$S/fifo -kernel $S/kernel8.img -append serial=on -drive file=$S/runtmp.img,if=sd,format=raw &
pid=$!; sleep $3; kill $pid 2>/dev/null; wait $pid 2>/dev/null
sleep 1; rm -f $S/runtmp.img $S/fifo
