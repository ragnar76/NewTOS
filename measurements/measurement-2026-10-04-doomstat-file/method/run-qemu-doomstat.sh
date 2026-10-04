#!/usr/bin/env bash
# run-qemu-doomstat.sh <kernel8.img> <sd.img> <outdir> <tag> <SECONDS:PNG...>
# Headless QEMU raspi4b with the card (-append serial=on), QMP screendumps at
# the given offsets, then QMP quit (qmp_timed.py); afterwards DOOMSTAT.TXT is
# fetched from C:\DOOM on the card with mtools (partition at sector 63).
# Derived from measurement-2026-10-04-keys02-doom/method/run-qemu.sh.
set -uo pipefail
K="$1"; SD="$2"; OUT="$3"; TAG="$4"; shift 4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT"
LOG="$OUT/$TAG-serial.log"
SDIR="$(mktemp -d /private/tmp/claude-501/dst.XXXXXX)"   # unix socket path < 104 bytes
SOCK="$SDIR/qmp.sock"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
  -kernel "$K" -drive file="$SD",if=sd,format=raw -append serial=on \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_timed.py" "$SOCK" "$OUT" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null; rm -f "$SOCK"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
mdir -i "$SD@@$((63*512))" ::DOOM > "$OUT/$TAG-mdir-DOOM.txt" 2>&1
mcopy -n -i "$SD@@$((63*512))" ::DOOM/DOOMSTAT.TXT "$OUT/DOOMSTAT.TXT" || echo "NO DOOMSTAT.TXT on the card"
