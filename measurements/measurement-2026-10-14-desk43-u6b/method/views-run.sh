#!/usr/bin/env bash
# DESK43 U6b copy of measurement-2026-10-14-desk43-u8a/method/views-run.sh (unchanged below this header; the injector it applies is
# this run's inj-desk-views.py, armed at the kernel's desktop-up line). Source tree: rsync of the shared tree.
# views-run.sh <workdir> <source-tree> <label> -- DESK43 U7: screen proof on the
# paths that use the cell metrics, driven by inj-desk-views.py (keys + IKBD mouse).
# Same shape as seq-run.sh, but WITHOUT svc-seq-trace (nothing is written to the
# card, so the "bytes used" info line cannot differ between kernels):
#   1. rsync <source-tree> -> <workdir>/tree-views-<label>, injector applied to that copy only
#   2. release rpi4 kernel (no extra features)
#   3. private copy of the MEM05 plain card
#   4. headless QEMU raspi4b serial=on, screendumps at the phase midpoints, quit
set -euo pipefail
W="${1:?workdir}"; SRC="${2:?source tree}"; L="${3:?label}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
CARD_SRC="${CARD_SRC:-/tmp/m5j/u5b/cards/plain.img}"
mkdir -p "$W/s"
export SOURCE_DATE_EPOCH=1791979200
rsync -a --delete --exclude target "$SRC/" "$W/tree-views-$L/"
python3 "$HERE/inj-desk-views.py" "$W/tree-views-$L"
(cd "$W/tree-views-$L" && cargo xtask build --board rpi4 --release) > "$W/build-views-$L.log" 2>&1 \
  || { tail -30 "$W/build-views-$L.log"; exit 1; }
grep -E "generated [0-9]+ warning|^error" "$W/build-views-$L.log" || true
cp "$W/tree-views-$L/kernel8.img" "$M/bin/kernel8-views-$L.img"
CARD="$W/views-card-$L.img"; cp "$CARD_SRC" "$CARD"
LOG="$M/serial/views-$L-rpi4-serial.log"
rm -f "$LOG" "$W/s/qmp.sock"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -kernel "$M/bin/kernel8-views-$L.img" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:"$W/s/qmp.sock",server=on,wait=off &
QPID=$!
P="views-$L-rpi4"
python3 "$HERE/qmp_timed.py" "$W/s/qmp.sock" "$M/screendumps" \
  18:$P-1-text.png 21:$P-2-viewmenu.png 23.5:$P-3-icons.png 28.5:$P-4-about.png \
  35.5:$P-5-prefs.png 42.5:$P-6-background.png 49.5:$P-7-config.png 53:$P-8-end.png
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null || true; wait "$QPID" 2>/dev/null || true
rm -f "$CARD"
echo "views run $L done"
