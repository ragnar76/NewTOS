#!/usr/bin/env bash
# run-timed.sh <scratch> <kernel8.img> <card.img> <case> <SECONDS:PNG>...
# One headless QEMU raspi4b run (-append "${APPEND:-serial=on memprot=on}") on
# <card.img> -- an APFS clone the caller prepared -- with the SD command trace
# (sdcard_normal_command/read_block/write_block), screendumps at fixed
# wall-clock offsets (qmp_timed.py, copied from
# measurement-2026-10-07-zview02), quit after the last one. Then: trace
# counted (trace-counts.py), fsck_msdos -n on every partition, the card's
# sha256 recorded, card + trace deleted. One QEMU at a time; only our own pid
# is killed.
set -uo pipefail
S="$1"; K="$2"; CARD="$3"; CASE="$4"; shift 4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$S/serial" "$S/shots" "$S/out" "$S/trace"
LOG="$S/serial/$CASE.log"; rm -f "$LOG"; TR="$S/trace/$CASE.trace"; rm -f "$TR"
SD="$(mktemp -d "$S/qmp.XXXXXX")"
specs=(); for sp in "$@"; do specs+=("${sp%%:*}:$CASE-${sp#*:}"); done
cd "$SD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on memprot=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off \
  -trace sdcard_normal_command -trace sdcard_read_block -trace sdcard_write_block -D "$TR" &
QPID=$!
python3 "$HERE/qmp_timed.py" qmp.sock "$S/shots" "${specs[@]}"
cd "$S"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
python3 "$HERE/trace-counts.py" "$TR" > "$S/out/$CASE-trace-counts.txt"; cat "$S/out/$CASE-trace-counts.txt"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
for p in s1 s2; do [ -e "${DEV}$p" ] && { echo "== fsck_msdos -n $p"; fsck_msdos -n "${DEV}$p" 2>&1; }; done > "$S/out/$CASE-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
grep -E "^==|Warning|differ|orphan|Orphan|lost|Cross|Fix|corrupt|BAD" "$S/out/$CASE-fsck.txt" | grep -v Phase
shasum -a 256 "$CARD" > "$S/out/$CASE.card-sha256"; rm -f "$CARD" "$TR"
grep -cE "\[EXC\]|PANIC|Memory violation" "$LOG" | sed 's/^/exceptions in serial: /'
echo "run $CASE done"
