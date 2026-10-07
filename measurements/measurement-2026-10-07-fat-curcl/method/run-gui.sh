#!/usr/bin/env bash
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
mkdir -p "$S/shots" "$S/serial" "$S/out"
SDIR="$(mktemp -d "$S/q.XXXXXX")"
cd "$SDIR"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:ser.sock,server=on,wait=on -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
python3 "$HERE/drive-m3.py" ser.sock qmp.sock "$S/shots" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd "$S"; rm -f "$SDIR/ser.sock" "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
if [ -n "${CHECK:-}" ]; then $CHECK "$CARD" > "$S/out/$TAG-hostcheck.txt" 2>&1; tail -1 "$S/out/$TAG-hostcheck.txt"; fi
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
for p in s1 s2; do [ -e "${DEV}$p" ] && { echo "== fsck_msdos -n $p"; fsck_msdos -n "${DEV}$p" 2>&1; }; done > "$S/out/$TAG-fsck.txt"
hdiutil detach "$DEV" >/dev/null 2>&1
grep -E "^==|Warning|differ|orphan|Orphan|lost|Cross|Fix|corrupt|BAD" "$S/out/$TAG-fsck.txt" | grep -v "Phase"
shasum -a 256 "$CARD" > "$S/out/$TAG.card-sha256"; rm -f "$CARD"
