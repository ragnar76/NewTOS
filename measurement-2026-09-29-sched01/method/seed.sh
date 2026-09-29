#!/usr/bin/env bash
# seed.sh <image> <inf> [ball]
#
# Build a fresh card with tools/make-sd-image.sh (the single source of truth
# for the card layout — never a hand-crafted image), drop in the NEWDESK.INF
# whose `#Z` line names the program under test, and — with `ball` — PORT07's
# ball-demo fixture (BALL.NAP + its texture + `hud 1`).
#
# Everything else, INCLUDING the two desk accessories C:\ACC\SYSACC.NAC and
# C:\SYSACC2.NAC, comes from make-sd-image.sh unchanged: they are the state
# under test.
set -euo pipefail
IMG="${1:?usage: seed.sh <image> <inf> [ball]}"
INF="${2:?}"; MODE="${3:-plain}"
WS=/Volumes/Daten/Development/PiRusTOS
DEMO=/Volumes/Daten/Development/ball-demo
"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1
cp "$INF" "$V/NEWDESK.INF"
if [ "$MODE" = "ball" ]; then
  cp "$DEMO/BALL.NAP" "$V/BALL.NAP"
  cp "$DEMO/BALL.NTX" "$V/BALL.NTX"
  printf 'hud 1\r\n' > "$V/BALL.CFG"
  rm -f "$V/BALL.LOG"
fi
find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG (inf=$(basename "$INF"), mode=$MODE)"
