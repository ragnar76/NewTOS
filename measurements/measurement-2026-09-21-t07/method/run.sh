#!/usr/bin/env bash
# T07 runtime evidence: both .NCPs from C:\AUTO\ in QEMU raspi4b, serial to a file.
# Run from the repository root. Scratch image lives in the job scratchpad, not /tmp.
set -euo pipefail
SCR="${SCR:?set SCR to a scratch directory}"
sdk/libc/build-libc.sh sdk/examples/setjmptst.c sdk/libc/build/setjmptst.NCP
sdk/libm/build-libm.sh                                   # -> sdk/libm/build/mathtst.NCP
SD_SETJMPTST_AUTO=1 SD_MATHTST_AUTO=1 tools/make-sd-image.sh "$SCR/t07-sd.img" 64
cargo xtask build --board rpi4                           # -> kernel8.img (dev profile, unchanged kernel)
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
    -serial "file:$SCR/t07-serial.log" -kernel kernel8.img \
    -drive "file=$SCR/t07-sd.img,if=sd,format=raw" &
pid=$!
sleep 75          # boot info screen ~15 s + AUTO folder + desktop
kill $pid
grep -E 'auto\]|setjmptst|mathtst|launch\]|EXC|PANIC' "$SCR/t07-serial.log"
