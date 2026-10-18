#!/usr/bin/env bash
# virt-boot.sh <kernel-virt.img (flat binary)> <log> <secs> -- qemu-virt boot as xtask run (gic-version=2, cortex-a72, 272M, ramfb, xhci+kbd+mouse), serial -> log
K="$1"; LOG="$2"; N="$3"; rm -f "$LOG"
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse -display none -serial file:"$LOG" -no-reboot -kernel "$K" &
pid=$!; sleep "$N"; kill $pid 2>/dev/null; wait $pid 2>/dev/null; true
