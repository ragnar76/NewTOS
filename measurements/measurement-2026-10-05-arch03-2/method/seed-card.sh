#!/usr/bin/env bash
#
# seed-card.sh <image> <autorun-path or -> 
#
# Fresh 64 MiB MBR + FAT16 QEMU card for the ARCH03 stage-2 runs (copied from
# the stage-1 method; every program in bin/ goes to C:\), holding the
# REBUILT (PIE) programs, and a NEWDESK.INF whose #Z line autoruns
# <autorun-path> (e.g. 'C:\KBDVEC.NAP'); '-' = no #Z line (plain desktop).
# Deliberately NOT tools/make-sd-image.sh: that script writes through the
# fixed path /Volumes/NEWTOS. This one uses its own volume label and mounts
# the image on a private mount point next to it, never touching any card.
set -euo pipefail
IMG="${1:?}"; AUTO="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
BIN="$HERE/../bin"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" A31TEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
printf 'ARCH03 stage 2 test card\r\n' > "$MNT/README.TXT"
# Every program in bin/ to C:\ (stage-2 probes included); the PORT10 layout
# (measurement-2026-10-02-port10d/method/run-port10.sh) under C:\P10.
for f in "$BIN"/*.NCP "$BIN"/*.NAP "$BIN"/*.NTP; do cp "$f" "$MNT/"; done
mkdir -p "$MNT/P10"
for d in P10KID.NCP P10KID.NTP ONLYNCP.NCP LEGACY.PRG APP.NAP ACC.NAC PLAIN.TXT; do
  cp "$BIN/P10KID.NCP" "$MNT/P10/$d"
done
cp "$WS/sdk/apps/apitest/APITEST.NCP" "$MNT/APITEST.NCP"
# SEED_ACC=1: also C:\ACC\SYSACC.NAC (a desk accessory: its first EL0 entry
# goes through the scheduler's fresh-dispatch path, not Pexec/launch.rs).
if [ "${SEED_ACC:-0}" = "1" ]; then mkdir -p "$MNT/ACC"; cp "$BIN/SYSACC.NAC" "$MNT/ACC/SYSACC.NAC"; fi
mkdir -p "$MNT/Apps/QED" "$MNT/Apps/TERADESK" "$MNT/DOOM"
cp "$WS/sdk/apps/qed/build/QED.NAP" "$WS/sdk/apps/qed/build/qed.rsc" "$MNT/Apps/QED/"
T="$WS/sdk/apps/teradesk/build/dist"
cp "$T/DESKTOP.NAP" "$T/desktop.rsc" "$T/icons.rsc" "$T/cicons.rsc" "$MNT/Apps/TERADESK/"
cp "$HERE/TERADESK.INF" "$MNT/TERADESK.INF"
cp "$HERE/TERADESK.INF" "$MNT/Apps/TERADESK/TERADESK.INF"
cp "$WS/sdk/apps/doom/build/DOOM.NAP" "$MNT/DOOM/DOOM.NAP"
cp "$WS/Applications/doom-wad/doom1.wad" "$MNT/DOOM/DOOM1.WAD"
# EXTRA="<dest>=<host file> ...": further files (dest relative to C:\, '/'
# as separator), e.g. the PORT10 layout P10/P10KID.NTP=bin/P10KID.NCP.
for spec in ${EXTRA:-}; do
  dst="${spec%%=*}"; src="${spec#*=}"
  mkdir -p "$MNT/$(dirname "$dst")"; cp "$src" "$MNT/$dst"
done
if [ "$AUTO" = "-" ]; then
  sed '/^#Z/d' "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
else
  sed "s|^#Z .*|#Z 01 ${AUTO//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
fi
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
echo "seeded $IMG (#Z: $AUTO): $(grep '^#Z' "$MNT/NEWDESK.INF" || echo none)"
