#!/usr/bin/env bash
# build.sh -- VT52PI.NAP (the acceptance program) and the release rpi4 kernel for the QEMU run, into the scratch dir.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; mkdir -p "$S"
"$WS/sdk/bin/newtos-cc" -nap -Wall -Wextra -o "$S/VT52PI.NAP" "$WS/sdk/examples/vt52pi.c"
(cd "$WS" && cargo xtask build --board rpi4 --release) > "$S/kernel-build.log" 2>&1
cp "$WS/kernel8.img" "$S/kernel8-rpi4.img"
bash "$HERE/mkcard.sh" "$S/base.img" "$HERE/z-vt52pi.inf" VT52PI.NAP="$S/VT52PI.NAP" > /dev/null
shasum -a 256 "$S/VT52PI.NAP" "$S/kernel8-rpi4.img"
