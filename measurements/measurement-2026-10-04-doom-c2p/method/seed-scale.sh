#!/usr/bin/env bash
#
# seed-scale.sh <image> <DOOM.NAP> <doom1.wad> <newdesk.inf> [<LAUNCH.NAP> <command line>]
#
# PORT06's seed-doom.sh (measurement-2026-10-02-port06/method) with two
# changes: the NEWDESK.INF is an argument (the #Z line differs per run), and
# an optional launcher program is put at C:\LAUNCH.NAP together with the
# command line it runs, C:\LAUNCH.TXT (method/launch.c). Fresh QEMU card from
# tools/make-sd-image.sh every time (not changed); the WAD goes onto this
# scratch image only. Mounted on a private mount point, never under /Volumes.
set -euo pipefail
IMG="${1:?usage: seed-scale.sh <image> <DOOM.NAP> <doom1.wad> <inf> [<LAUNCH.NAP>]}"
NAP="${2:?}"; WAD="${3:?}"; INF="${4:?}"; LAUNCH="${5:-}"; CMD="${6:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null
MNT="$(dirname "$IMG")/mnt.$$"
mkdir -p "$MNT"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mkdir -p "$MNT/DOOM"
cp "$NAP" "$MNT/DOOM/DOOM.NAP"
cp "$WAD" "$MNT/DOOM/DOOM1.WAD"
cp "$INF" "$MNT/NEWDESK.INF"
if [ -n "$LAUNCH" ]; then cp "$LAUNCH" "$MNT/LAUNCH.NAP"; printf "%s\r\n" "$CMD" > "$MNT/LAUNCH.TXT"; fi
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
find "$MNT" -name '._*' -delete 2>/dev/null || true
sync
echo "seeded $(basename "$IMG"): DOOM.NAP $(shasum -a 256 "$NAP" | cut -c1-16), INF $(basename "$INF")${LAUNCH:+, LAUNCH.NAP + LAUNCH.TXT "$CMD"}"
