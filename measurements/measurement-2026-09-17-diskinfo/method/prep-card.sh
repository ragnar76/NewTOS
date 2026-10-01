#!/usr/bin/env bash
# prep-card.sh OUT_IMG DESKTOP_NAP AUTORUN
#
# Build one card for the T01 machine run from the recorded base image. Derived
# from measurement-2026-09-17-a65/method/prep-card.sh, with one hardening: the
# mount point is taken from `diskutil info` for the slice WE attached instead
# of assuming /Volumes/NEWTOS -- the user's own SD card lives under /Volumes
# and must never be written to.
set -euo pipefail
OUT_IMG="$1"; TERA="$2"; AUTORUN="${3:-}"
BASE="/Volumes/Daten/Development/PiRusTOS/measurement-2026-09-13-regression/qemu-tmp/sd-base.img"

cp -f "$BASE" "$OUT_IMG"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT_IMG" | head -1 | awk '{print $1}')
trap 'hdiutil detach "$DEV" >/dev/null 2>&1 || true' EXIT
diskutil mount "${DEV}s1" >/dev/null
V=$(diskutil info "${DEV}s1" | awk -F': *' '/Mount Point/{print $2}')
[ -n "$V" ] || { echo "error: no mount point for ${DEV}s1" >&2; exit 1; }
# Paranoia: the mounted slice must be the image we just attached.
case "$(diskutil info "$V" | awk -F': *' '/Device Node/{print $2}')" in
  "${DEV}s1") : ;;
  *) echo "error: $V is not ${DEV}s1 -- refusing to write" >&2; exit 1 ;;
esac
echo "card: $V  (= ${DEV}s1, our own image copy)"

cp -f "$TERA" "$V/Apps/TERADESK/DESKTOP.NAP"
if [ -n "$AUTORUN" ]; then
    printf '#Z 01 %s@\r\n' "$AUTORUN" > "$V/NEWDESK.INF"
else
    printf 'newdesk placeholder\r\n' > "$V/NEWDESK.INF"
fi
sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
trap - EXIT
echo "prepared $OUT_IMG autorun='${AUTORUN:-<built-in desktop>}'"
