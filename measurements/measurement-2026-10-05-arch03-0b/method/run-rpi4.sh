#!/bin/sh
# usage: run-rpi4.sh <img> <append> <secs> <log>
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$4" -kernel "$1" -append "$2" &
pid=$!
sleep "$3"
kill $pid 2>/dev/null
wait $pid 2>/dev/null
