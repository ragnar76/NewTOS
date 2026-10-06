#!/usr/bin/env bash
# rerun-port10.sh <label> <cards-root> <mode>... -- ARCH03-4: the PORT10 run
# again on a freshly seeded card whose C:\P10 files are verified first (in
# the series the seeding left C:\P10\ONLYNCP.NCP at 0 bytes on some cards --
# a host-side copy defect: Pexec of an empty file = EPLFMT, errno 66). The
# card is checked after the unmount and a short file re-written until every
# C:\P10 file has P10KID.NCP's size.
set -uo pipefail
L="$1"; CR="$2"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
want=$(stat -f %z "$B/P10KID.NCP")
for M in "$@"; do
  . "$HERE/mode.sh"
  C="$CR/rerun-$M"; mkdir -p "$C"
  bash "$HERE/seed-card.sh" "$C/p10.img" 'C:\P10TEST.NCP' > /dev/null
  for try in 1 2 3 4 5; do
    DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$C/p10.img" 2>/dev/null | head -1 | awk '{print $1}')
    mkdir -p "$C/mnt"; diskutil mount -mountPoint "$C/mnt" "${DEV}s1" > /dev/null
    bad=$(for f in "$C"/mnt/P10/*; do [ "$(stat -f %z "$f")" = "$want" ] || echo "$f"; done)
    # Re-write a truncated file through a plain stream copy (the macOS FAT
    # driver left one cp'ed file at 0 bytes after the unmount, see RUNINFO).
    for f in $bad; do cat "$B/P10KID.NCP" > "$f"; done
    sync; diskutil unmount "$C/mnt" > /dev/null; hdiutil detach "$DEV" > /dev/null 2>&1
    [ -z "$bad" ] && { echo "card $M ok (check $try)"; break; }
    echo "card $M check $try: rewrote $bad"
  done
  bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-$L.img" "$C/p10.img" "$L-$M-rpi4-port10-rerun" \
    --end 'port10: (ALL PASS|[0-9]+ of [0-9]+ checks FAILED)' --end-delay 2 --timeout 240
done
