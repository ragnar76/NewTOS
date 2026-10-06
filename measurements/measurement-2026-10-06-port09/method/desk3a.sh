#!/usr/bin/env bash
# desk3a.sh <label> <scratch-root> -- the stage-3a desktop run: a scratch copy
# of the shared tree (make-scratch.sh) with the DESK50 IKBD injector
# (apply_injector.py, armed at "[desk] desktop up" by desk_arm_patch.py)
# driving desk3a/desk3a.actions; a private card with C:\D3A\ {SUB3A\INSIDE.TXT,
# HELLO.NCP, NOTE.TXT} and desk3a/newdesk-desk3a.inf; screendumps at each MARK.
# The instrumentation is never applied to the shared tree.
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/../bin"
S="$R/desk3a-src"
sh "$HERE/make-scratch.sh" "$S"
python3 "$HERE/gen_script.py" "$HERE/desk3a/desk3a.actions" SCRIPT > "$R/desk3a-script.rs"
python3 "$HERE/apply_injector.py" "$S" "$R/desk3a-script.rs" > /dev/null
python3 "$HERE/desk_arm_patch.py" "$S" > /dev/null
(cd "$S" && cargo xtask build --board rpi4 --release > "$R/desk3a-build.log" 2>&1) || { tail -20 "$R/desk3a-build.log"; exit 1; }
cp "$S/kernel8.img" "$BIN/kernel8-desk3a-$L.img"
mkdir -p "$R/cards"
INF="$HERE/desk3a/newdesk-desk3a.inf" \
EXTRA="D3A/SUB3A/INSIDE.TXT=$HERE/desk3a/INSIDE.TXT D3A/NOTE.TXT=$HERE/desk3a/NOTE.TXT D3A/HELLO.NCP=$BIN/HELLO.NCP" \
  bash "$HERE/seed-card.sh" "$R/cards/desk3a.img" - > /dev/null
cp "$R/cards/desk3a.img" "$R/cards/run-desk3a.img"
bash "$HERE/run-marks.sh" "$BIN/kernel8-desk3a-$L.img" "$R/cards/run-desk3a.img" "$HERE/../screendumps" "$L-rpi4-desk3a" 150 \
  "MARK A_IDLE:$L-desk3a-A_IDLE.png:0.5" "MARK B_SUB:$L-desk3a-B_SUB.png:0.5" "MARK C_UP:$L-desk3a-C_UP.png:0.5" \
  "MARK D_SELECTED:$L-desk3a-D_SELECTED.png:0.5" "MARK E_INFO:$L-desk3a-E_INFO.png:0.5" "MARK F_TYPED:$L-desk3a-F_TYPED.png:0.5" \
  "MARK G_RENAMED:$L-desk3a-G_RENAMED.png:0.5" "MARK H_DRIVE_C:$L-desk3a-H_DRIVE_C.png:0.5" || true
mv "$HERE/../screendumps/$L-rpi4-desk3a-serial.log" "$HERE/../serial/" 2>/dev/null || true
echo "desk3a $L done"
