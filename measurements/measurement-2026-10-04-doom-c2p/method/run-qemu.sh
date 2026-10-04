#!/usr/bin/env bash
# run-qemu.sh <kernel8.img> <sd.img> <outdir> <tag> <WxH> <SECONDS:PNG...>
# PORT06's run-doom.sh (measurement-2026-10-02-port06/method) with one
# change: the VideoCore framebuffer size is a parameter. QEMU's bcm2835-fb
# answers the kernel's get-physical-size query (kernel/src/drivers/bcm2711/
# framebuffer.rs query_physical_size) with its xres/yres properties (default
# 640x480), so -global bcm2835-fb.xres/yres gives the kernel -- and the VDI --
# a 1920x1080 screen like the real Pi's, without touching the kernel.
# Headless, QMP screendumps at the given offsets, then quit (qmp_timed.py
# from PORT06, unchanged). The host has no `timeout`.
# 2026-10-04 (doom-aspect): copied from measurement-2026-10-02-port06-eddi,
# one change: -append serial=on (since SER01 a board-rpi4 kernel is
# UART-silent without it).
set -uo pipefail
K="$1"; SD="$2"; OUT="$3"; TAG="$4"; RES="$5"; shift 5
P06="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../measurement-2026-10-02-port06/method" && pwd)"
XR="${RES%x*}"; YR="${RES#*x}"
mkdir -p "$OUT"
LOG="$OUT/$TAG-serial.log"
SDIR="$(mktemp -d /private/tmp/claude-501/p06s.XXXXXX)"   # unix socket path < 104 bytes
SOCK="$SDIR/qmp.sock"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
  -global bcm2835-fb.xres="$XR" -global bcm2835-fb.yres="$YR" \
  -kernel "$K" -append serial=on -drive file="$SD",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$P06/qmp_timed.py" "$SOCK" "$OUT" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null; rm -f "$SOCK"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG ($RES): rc=$RC"
