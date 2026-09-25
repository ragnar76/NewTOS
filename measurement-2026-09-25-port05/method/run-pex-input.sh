#!/usr/bin/env bash
#
# run-pex-input.sh <kernel8.img> <card.img> <serial.log> <seconds> <feed-at-s>
#
# Same headless raspi4b run as run-pex.sh, but the serial line is a PAIR of
# named pipes (`-serial pipe:`) instead of a write-only file, so the run can
# send ONE byte into the guest after <feed-at-s> seconds.
#
# That byte is the control for the accessory-barrier finding: `appl_exit()`
# blocks in `wait_for_accs(AP_ACCLOSE)`, which sleeps on the WAIT_INPUT wake
# class, and the idle pass only wakes that class when a key or a pointer
# movement arrives (tos-api/src/bdos/process/sched.rs, `idle_wake_pass`).  A
# single serial character therefore has to be enough to un-stick a run that is
# otherwise stuck forever -- and if it is, the stop is input starvation, not a
# deadlock.
set -uo pipefail
K="${1:?}"; IMG="${2:?}"; LOG="${3:?}"; SECS="${4:-70}"; AT="${5:-30}"
PIPE="${LOG%.log}.pipe"
rm -f "$LOG" "$PIPE.in" "$PIPE.out"
mkfifo "$PIPE.in" "$PIPE.out"
# Hold the input pipe open for the whole run, otherwise QEMU sees EOF at once.
sleep "$((SECS + 5))" > "$PIPE.in" &
HOLD=$!
cat "$PIPE.out" > "$LOG" &
CATP=$!
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial pipe:"$PIPE" -kernel "$K" \
  -drive file="$IMG",if=sd,format=raw &
QPID=$!
for s in $(seq 1 "$SECS"); do
  kill -0 "$QPID" 2>/dev/null || break
  if [ "$s" = "$AT" ]; then
    printf 'x' > "$PIPE.in"
    echo "--- fed one serial byte at +${s}s ---"
  fi
  sleep 1
done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
kill -9 "$HOLD" "$CATP" 2>/dev/null; wait "$HOLD" "$CATP" 2>/dev/null
rm -f "$PIPE.in" "$PIPE.out"
echo "run finished: $LOG ($(wc -l < "$LOG" | tr -d ' ') lines)"
