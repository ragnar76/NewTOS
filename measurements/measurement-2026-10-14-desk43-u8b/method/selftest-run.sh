#!/usr/bin/env bash
# (DESK43 U8b copy of measurement-2026-10-14-desk43-u6b/method/selftest-run.sh: output names u6b -> u8b)
# selftest-run.sh <workdir> <source-tree> -- DESK43 U8a / DESK45: the desktop's four self-tests in one
# repeatable command, plus the counter-check that a normal boot leaves the card alone.
#   1. rsync <source-tree> -> <workdir>/tree-selftest + inj-selftest-return.py (Return for the one
#      confirmation box dir::selftest's delete waits on; see that file), release rpi4 kernel
#      `--features selftest-desk` -> bin/kernel8-selftest-desk.img
#   2. private copy of the MEM05 plain card; headless QEMU raspi4b serial=on, screendump at 75 s, quit;
#      serial -> serial/selftest-desk-rpi4-serial.log; PASS/FAIL lines -> checks/u8b-selftest-lines.txt
#   3. counter-check: the product kernel bin/kernel8-u8b.img on another private copy for 40 s
#      (desktop up, idle); card sha256 before/after -> checks/u8b-selftest-cardcheck.txt
set -euo pipefail
W="${1:?workdir}"; SRC="${2:?source tree}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
CARD_SRC="${CARD_SRC:-/tmp/m5j/u5b/cards/plain.img}"
mkdir -p "$W/s"
export SOURCE_DATE_EPOCH=1791979200
rsync -a --delete --exclude target "$SRC/" "$W/tree-selftest/"
python3 "$HERE/inj-selftest-return.py" "$W/tree-selftest"
(cd "$W/tree-selftest" && cargo xtask build --board rpi4 --release --features selftest-desk) > "$W/build-selftest.log" 2>&1 \
  || { tail -30 "$W/build-selftest.log"; exit 1; }
grep -E "generated [0-9]+ warning|^error" "$W/build-selftest.log" || true
cp "$W/tree-selftest/kernel8.img" "$M/bin/kernel8-selftest-desk.img"

boot() { # boot <kernel> <card> <serial log> <seconds:png>...
  local K="$1" CARD="$2" LOG="$3"; shift 3
  rm -f "$LOG" "$W/s/qmp.sock"
  qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
    -kernel "$K" -drive file="$CARD",if=sd,format=raw \
    -serial file:"$LOG" -qmp unix:"$W/s/qmp.sock",server=on,wait=off &
  local QPID=$!
  python3 "$HERE/qmp_timed.py" "$W/s/qmp.sock" "$M/screendumps" "$@"
  for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
  kill -9 "$QPID" 2>/dev/null || true; wait "$QPID" 2>/dev/null || true
}

CARD="$W/selftest-card.img"; cp "$CARD_SRC" "$CARD"
LOG="$M/serial/selftest-desk-rpi4-serial.log"
boot "$M/bin/kernel8-selftest-desk.img" "$CARD" "$LOG" 75:selftest-desk-rpi4-t75.png
tr -d '\r' < "$LOG" | grep -aE '^\[desk(-selftest|-dir|-inf|-ins)\]|\[desk\] desktop up|\[u8a-st\]|\[EXC\]|PANIC' > "$M/checks/u8b-selftest-lines.txt" || true
{ echo "PASS lines: $(grep -c ' PASS ' "$M/checks/u8b-selftest-lines.txt")  FAIL lines: $(grep -c ' FAIL ' "$M/checks/u8b-selftest-lines.txt")";
  grep -E 'passed, [0-9]+ failed|selftest\] (begin|done)' "$M/checks/u8b-selftest-lines.txt"; } > "$M/checks/u8b-selftest-summary.txt"
rm -f "$CARD"

CARD="$W/normal-card.img"; cp "$CARD_SRC" "$CARD"
B="$(shasum -a 256 "$CARD" | awk '{print $1}')"
boot "$M/bin/kernel8-u8b.img" "$CARD" "$W/normal-boot-serial.log" 40:selftest-counter-normal-t40.png
A="$(shasum -a 256 "$CARD" | awk '{print $1}')"
{ echo "normal boot (kernel8-u8b.img, plain card copy, 40 s): card sha256 before $B"; echo "                                                       after  $A";
  [ "$A" = "$B" ] && echo "UNCHANGED" || echo "CHANGED";
  echo "desk-selftest lines in the normal boot's serial log: $(grep -ac 'desk-selftest\|desk-dir\]\|desk-inf\]\|desk-ins\]' "$W/normal-boot-serial.log")"; } > "$M/checks/u8b-selftest-cardcheck.txt"
rm -f "$CARD"
cat "$M/checks/u8b-selftest-summary.txt" "$M/checks/u8b-selftest-cardcheck.txt"
