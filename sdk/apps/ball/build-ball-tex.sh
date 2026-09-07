#!/usr/bin/env bash
#
# build-ball-tex.sh -- make BALL.NTX, the demo's texture, out of the artwork.
#
# The texture is a pipeline and not a picture (see ball-tex.h): the artwork is
# an equirectangular PNG, tools/png2ntx.py reduces it ONCE here on a fast
# machine, and both target machines -- a Raspberry Pi 4 and an Atari TT with a
# NOVA card -- load the result exactly as it is. Nothing is quantised at run
# time on either of them.
#
# This script exists so the provenance is written down rather than remembered:
# WHICH artwork, at WHICH size, with WHICH reducer. Re-run it whenever the
# artwork changes.
#
#   source : Applications/Demo-bouncing-Ball/*.png   (the user's artwork)
#   output : sdk/apps/ball/BALL.NTX  -> C:\BALL.NTX on the card
#
# 256x128 is the right size for BOTH machines and that was measured, not
# assumed. On the Pi at 1920x1080 the ball is 306 px across; on the TT at
# 640x480 it is 102 px. Even at 102 px the ball's centre maps roughly 64
# texels of a 256-wide texture onto its 51-pixel radius -- about one texel per
# pixel -- so the texture is not the limit there. Halving it to 128x64 was
# rendered and compared: it is visibly blockier on the TT, not smoother.
#
# Usage:  sdk/apps/ball/build-ball-tex.sh [artwork.png]
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"

ART="${1:-$ROOT/Applications/Demo-bouncing-Ball/ChatGPT Image 6. Sept. 2026, 21_07_39.png}"
OUT="$HERE/BALL.NTX"

if [ ! -f "$ART" ]; then
    echo "error: artwork not found: $ART" >&2
    echo "       pass the equirectangular PNG as the first argument" >&2
    exit 1
fi

echo "artwork : $ART"
python3 "$ROOT/tools/png2ntx.py" "$ART" "$OUT" --size 256x128
echo
echo "tools/make-sd-image.sh installs it as C:\\BALL.NTX. Delete it and the"
echo "demo falls back to the texture it draws from geometry -- both paths are"
echo "live, and the demo says which one it took in C:\\BALL.LOG."
