#!/usr/bin/env bash
# runall.sh — the whole measurement, from one product state.
#
# Rebuilds BOTH scratch trees from the shared tree as it stands (`after`) and
# from its committed HEAD (`before`), then drives every run. Everything lands
# in $SCR/tmp/out. `runone.sh` only swaps the injector script and rebuilds, so
# the trees have to be re-made whenever the product sources change — that is
# what this wrapper is for.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCR="${SCR:?set SCR to the scratch directory}"
export SCR
BASE="$SCR/tmp/base.img"

bash "$HERE/make_scratch.sh" "$SCR/after"  "$HERE/d37g.rs" after
bash "$HERE/make_scratch.sh" "$SCR/before" "$HERE/d37g.rs" before
bash "$HERE/make_scratch.sh" "$SCR/probe"  "$HERE/d30b.rs" after probe

run() { bash "$HERE/runone.sh" "$@"; }

# ── DESK37 ──────────────────────────────────────────────────────────────────
for M in after before; do
  run "$SCR/$M" "$BASE" "d37g-$M" 120 \
      "B_WIN1:d37g-$M-B-win1.png:0.5" "C_APP:d37g-$M-C-app.png:0.5" "D_APP:d37g-$M-D-app.png:0.5"
  run "$SCR/$M" "$BASE" "d37q-$M" 150 \
      "B_WIN1:d37q-$M-B-win1.png:0.5" "C_QED:d37q-$M-C-qed.png:0.5" "D_QED:d37q-$M-D-qed.png:0.5"
done

# ── DESK36 ──────────────────────────────────────────────────────────────────
for M in after before; do
  run "$SCR/$M" "$BASE" "d36-$M" 180 \
      "B_WIN1:d36-$M-B-win1.png:0.5" "C_OVER_PROG:d36-$M-C-prog.png:0.5" \
      "D_OVER_FOLDER:d36-$M-D-folder.png:0.5" "E_OVER_FILE:d36-$M-E-file.png:0.5" \
      "F_OVER_TRASH:d36-$M-F-trash.png:0.5" "G_BACK:d36-$M-G-back.png:0.5" \
      "H_RELEASED:d36-$M-H-released.png:0.5"
done

# ── DESK30 ──────────────────────────────────────────────────────────────────
for M in after before; do
  run "$SCR/$M" "$BASE" "d30-$M" 150 \
      "B_WIN1:d30-$M-B-win1.png:0.5" "C_DRAGGING:d30-$M-C-drag.png:0.5" \
      "D_SHORTCUT:d30-$M-D-shortcut.png:0.5" "E_SAVED:d30-$M-E-saved.png:0.5"
done
# The card the `after` run saved is the input of the two cold-start runs.
cp "$SCR/tmp/d30-after.img" "$SCR/tmp/saved.img"
run "$SCR/probe" "$SCR/tmp/saved.img" d30b 120 \
    "A_BOOT:d30b-A-coldstart.png:0.8" "B_END:d30b-B-end.png:0.5"
run "$SCR/after" "$SCR/tmp/saved.img" d30c 150 \
    "A_BOOT:d30c-A-coldstart.png:0.8" "C_QED:d30c-C-qed.png:0.5" "D_QED:d30c-D-qed.png:0.5"
run "$SCR/after" "$BASE" d30d 150 \
    "B_WIN1:d30d-B-win1.png:0.5" "C_SHORTCUT:d30d-C-shortcut.png:0.5" \
    "D_OPENED:d30d-D-opened.png:0.8"

echo "runall: done"
