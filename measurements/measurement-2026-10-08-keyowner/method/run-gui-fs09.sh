#!/usr/bin/env bash
# KEYOWNER copy (from measurement-2026-10-08-ofam-freeze/method/fs09/run-gui.sh):
# screendumps go to ../screendumps (drive-m3.py writes the serial log to
# ../serial), no SD trace (TRACE is not set by keyowner-runs.sh).
# FS09: copied from measurement-2026-10-07-stdiobuf; the trace carries host
# time stamps (-msg timestamp=on) and, with drive-m3.py --stamp, is split per
# [m3] MARK segment (mark-counts.py -> <tag>-mark-counts.txt).
# COPIED from measurement-2026-10-07-fat-curcl/method/run-gui.sh for TASK/FS08
# part 4 (stdiobuf). Change: with TRACE=1 the SD commands are traced
# (sdcard_normal_command/read_block/write_block) into <scratch>/trace/<tag>.trace,
# counted with trace-counts.py into <scratch>/out/<tag>-trace-counts.txt, and
# the trace is deleted.
# run-gui.sh <scratch> <injector kernel8.img> <card.img> <tag> [drive-m3.py options]
# One injector run (QEMU raspi4b, -append "${APPEND:-serial=on}" = memprot on)
# on <card.img> -- an APFS clone the caller prepared; drive-m3.py (from
# measurement-2026-10-07-acctitle) takes a screendump per "[m3] MARK" into
# <scratch>/shots and the serial log into <scratch>/serial. Then: optional
# host check CHECK="<cmd>" (the card path appended), fsck_msdos -n on every
# partition, the card's sha256 to <scratch>/out/<tag>.card-sha256, the card
# deleted. Sockets are relative to a private dir in <scratch> (the absolute
# scratch path exceeds the 104-byte UNIX socket limit). One QEMU at a time;
# only our own pid is killed.
set -uo pipefail
S="$1"; K="$2"; CARD="$3"; TAG="$4"; shift 4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"; mkdir -p "$S/out"
SDIR="$(mktemp -d "$S/q.XXXXXX")"
TRX=(); TR="$S/trace/$TAG.trace"
if [ "${TRACE:-0}" = 1 ]; then mkdir -p "$S/trace"; rm -f "$TR"
  TRX=(-msg timestamp=on -trace sdcard_normal_command -trace sdcard_read_block -trace sdcard_write_block -D "$TR"); fi
cd "$SDIR"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:ser.sock,server=on,wait=on -qmp unix:qmp.sock,server=on,wait=off ${TRX[@]+"${TRX[@]}"} &
QPID=$!
python3 "$HERE/drive-m3.py" ser.sock qmp.sock "$OUT" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd "$S"; rm -f "$SDIR/ser.sock" "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
if [ "${TRACE:-0}" = 1 ]; then python3 "$HERE/trace-counts.py" "$TR" > "$S/out/$TAG-trace-counts.txt"
  if [ -f "$S/serial/$TAG-stamped.log" ]; then python3 "$HERE/mark-counts.py" "$S/serial/$TAG-stamped.log" "$TR" > "$S/out/$TAG-mark-counts.txt"; cat "$S/out/$TAG-mark-counts.txt"; fi
  rm -f "$TR"
  grep -E "read commands|blocks read" "$S/out/$TAG-trace-counts.txt"; fi
if [ -n "${CHECK:-}" ]; then $CHECK "$CARD" > "$S/out/$TAG-hostcheck.txt" 2>&1; tail -1 "$S/out/$TAG-hostcheck.txt"; fi
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
for p in s1 s2; do [ -e "${DEV}$p" ] && { echo "== fsck_msdos -n $p"; fsck_msdos -n "${DEV}$p" 2>&1; }; done > "$S/out/$TAG-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
grep -E "^==|Warning|differ|orphan|Orphan|lost|Cross|Fix|corrupt|BAD" "$S/out/$TAG-fsck.txt" | grep -v "Phase"
shasum -a 256 "$CARD" > "$S/out/$TAG.card-sha256"; rm -f "$CARD"
