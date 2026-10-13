#!/usr/bin/env bash
# desk3a-run.sh <label> <scratch-root> <on|off|strict> -- the stage-3a desktop
# run (desk3a.sh) in one protection mode: builds the injector kernel once per
# label (kernel8-desk3a-<label>.img), then drives it with the mode's boot
# argument; screendumps <label>-<mode>-desk3a-*.png.
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"; M="${3:?mode}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/../bin"
. "$HERE/mode.sh"
if [ ! -f "$BIN/kernel8-desk3a-$L.img" ]; then
  S="$R/desk3a-src"
  sh "$HERE/make-scratch.sh" "$S"
  python3 "$HERE/gen_script.py" "$HERE/desk3a/desk3a.actions" SCRIPT > "$R/desk3a-script.rs"
  python3 "$HERE/apply_injector.py" "$S" "$R/desk3a-script.rs" > /dev/null
  python3 "$HERE/desk_arm_patch.py" "$S" > /dev/null
  (cd "$S" && cargo xtask build --board rpi4 --release > "$R/desk3a-build.log" 2>&1) || { tail -20 "$R/desk3a-build.log"; exit 1; }
  cp "$S/kernel8.img" "$BIN/kernel8-desk3a-$L.img"
fi
mkdir -p "$R/cards"
[ -f "$R/cards/desk3a.img" ] || INF="$HERE/desk3a/newdesk-desk3a.inf" \
EXTRA="D3A/SUB3A/INSIDE.TXT=$HERE/desk3a/INSIDE.TXT D3A/NOTE.TXT=$HERE/desk3a/NOTE.TXT D3A/HELLO.NCP=$BIN/HELLO.NCP" \
  bash "$HERE/seed-card.sh" "$R/cards/desk3a.img" - > /dev/null
cp "$R/cards/desk3a.img" "$R/cards/run-desk3a.img"
T="$L-$M"
bash "$HERE/run-marks.sh" "$BIN/kernel8-desk3a-$L.img" "$R/cards/run-desk3a.img" "$HERE/../screendumps" "$T-rpi4-desk3a" 150 \
  "MARK A_IDLE:$T-desk3a-A_IDLE.png:0.5" "MARK B_SUB:$T-desk3a-B_SUB.png:0.5" "MARK C_UP:$T-desk3a-C_UP.png:0.5" \
  "MARK D_SELECTED:$T-desk3a-D_SELECTED.png:0.5" "MARK E_INFO:$T-desk3a-E_INFO.png:0.5" "MARK F_TYPED:$T-desk3a-F_TYPED.png:0.5" \
  "MARK G_RENAMED:$T-desk3a-G_RENAMED.png:0.5" "MARK H_DRIVE_C:$T-desk3a-H_DRIVE_C.png:0.5" || true
mv "$HERE/../screendumps/$T-rpi4-desk3a-serial.log" "$HERE/../serial/" 2>/dev/null || true
echo "desk3a $T done"
