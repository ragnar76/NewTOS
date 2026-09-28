#!/usr/bin/env bash
# seed-ball.sh <image> <BALL.NAP> [hud 0|1]
# Put the demo, its texture, a BALL.CFG and the #Z autorun line on the card.
set -euo pipefail
IMG="${1:?usage: seed-ball.sh <image> <BALL.NAP> [hud]}"
NAP="${2:?}"; HUD="${3:-1}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEMO=/Volumes/Daten/Development/ball-demo
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1
cp "$NAP" "$V/BALL.NAP"
cp "$DEMO/BALL.NTX" "$V/BALL.NTX"
printf 'hud %s\r\n' "$HUD" > "$V/BALL.CFG"
rm -f "$V/BALL.LOG"
cp "$HERE/newdesk-ball.inf" "$V/NEWDESK.INF"
find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG with $(basename "$NAP") (hud $HUD)"
