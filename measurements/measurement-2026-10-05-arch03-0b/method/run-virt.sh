#!/bin/sh
# usage: run-virt.sh <img> <secs> <log>
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse -display none -serial file:"$3" -no-reboot -kernel "$1" &
pid=$!
sleep "$2"
kill $pid 2>/dev/null
wait $pid 2>/dev/null
