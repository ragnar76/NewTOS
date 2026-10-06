#!/usr/bin/env bash
# ppdesk-run.sh <label> <scratch-root> <on|off|strict> -- TASK/PPARENT: a
# program started by a desktop DOUBLE-CLICK (sh_ldapp NORMAL_APP) -- the
# desk3a-run.sh machinery (DESK50 IKBD injector in a scratch copy, armed at
# "[desk] desktop up" by desk_arm_patch.py) with ppdesk/ppdesk.actions on a
# card whose NEWDESK.INF opens C:\PPD\ (holding only PPDESK.NAP), no #Z.
# The injector kernel is built once per label (kernel8-ppdesk-<label>.img);
# never applied to the shared tree.
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"; M="${3:?mode}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/../bin"
. "$HERE/mode.sh"
if [ ! -f "$BIN/kernel8-ppdesk-$L.img" ]; then
  S="$R/ppdesk-src"
  sh "$HERE/make-scratch.sh" "$S"
  python3 "$HERE/gen_script.py" "$HERE/ppdesk/ppdesk.actions" SCRIPT > "$R/ppdesk-script.rs"
  python3 "$HERE/apply_injector.py" "$S" "$R/ppdesk-script.rs" > /dev/null
  python3 "$HERE/desk_arm_patch.py" "$S" > /dev/null
  (cd "$S" && cargo xtask build --board rpi4 --release > "$R/ppdesk-build.log" 2>&1) || { tail -20 "$R/ppdesk-build.log"; exit 1; }
  cp "$S/kernel8.img" "$BIN/kernel8-ppdesk-$L.img"
fi
mkdir -p "$R/cards"
[ -f "$R/cards/ppdesk.img" ] || INF="$HERE/ppdesk/newdesk-ppdesk.inf" \
EXTRA="PPD/PPDESK.NAP=$BIN/PPDESK.NAP" \
  bash "$HERE/seed-card.sh" "$R/cards/ppdesk.img" - > /dev/null
cp "$R/cards/ppdesk.img" "$R/cards/run-ppdesk.img"
T="$L-$M"
bash "$HERE/run-marks.sh" "$BIN/kernel8-ppdesk-$L.img" "$R/cards/run-ppdesk.img" "$HERE/../screendumps" "$T-rpi4-ppdesk" 150 \
  "MARK A_IDLE:$T-ppdesk-A_IDLE.png:0.5" "MARK B_BACK:$T-ppdesk-B_BACK.png:0.5" || true
mv "$HERE/../screendumps/$T-rpi4-ppdesk-serial.log" "$HERE/../serial/" 2>/dev/null || true
echo "ppdesk $T done"
