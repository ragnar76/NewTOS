#!/usr/bin/env bash
set -uo pipefail
SCR=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/d7ff6c34-253d-4fd6-839f-efa08458ba32/scratchpad/regress
M="$SCR/method"
export SCR

run_case() {
  local tag="$1" tmo="$2"; shift 2
  python3 "$M/apply_injector.py" "$SCR/tree" "$M/$tag.rs" >/dev/null
  python3 "$M/a78_patch2.py" "$SCR/tree" >/dev/null
  python3 "$M/key_step_patch.py" "$SCR/tree" >/dev/null
  ( cd "$SCR/tree" && cargo run -p xtask -- build --board rpi4 ) > "$SCR/build/scratch-build-$tag.log" 2>&1
  cp "$SCR/tmp/base.img" "$SCR/tmp/$tag.img"
  bash "$M/run.sh" "$SCR/tree/kernel8.img" "$SCR/tmp/$tag.img" "$SCR/tmp/out" "$tag" "$tmo" "$@"
}

run_case rg-desk13c 150 "A_IDLE:rg-desk13c-A-idle.png:0.5" "B_ALTC:rg-desk13c-B-altc.png:0.5" "C_SUBDIR:rg-desk13c-C-subdir.png:0.5" "D_COPYBOX:rg-desk13c-D-copybox.png:0.5" "E_CANCELLED:rg-desk13c-E-cancelled.png:0.5" "F_COPYBOX2:rg-desk13c-F-copybox2.png:0.5" "G_COPIED:rg-desk13c-G-copied.png:0.5"

run_case rg-desk13d 175 "A_IDLE:rg-desk13d-A-idle.png:0.5" "D_COPYBOX:rg-desk13d-D-copybox.png:0.5" "E_CONFLICT:rg-desk13d-E-conflict.png:0.5" "F_SKIPPED:rg-desk13d-F-skipped.png:0.5" "G_COPYBOX2:rg-desk13d-G-copybox2.png:0.5" "H_CONFLICT2:rg-desk13d-H-conflict2.png:0.5" "I_OVERWRITTEN:rg-desk13d-I-overwritten.png:0.5"

run_case rg-desk13f 150 "A_IDLE:rg-desk13f-A-idle.png:0.5" "D_SELECTED:rg-desk13f-D-selected.png:0.5" "E_DELBOX:rg-desk13f-E-delbox.png:0.5" "F_CANCELLED:rg-desk13f-F-cancelled.png:0.5" "G_DELBOX2:rg-desk13f-G-delbox2.png:0.5" "H_DELETED:rg-desk13f-H-deleted.png:0.5"

run_case rg-desk24merge 125 "A_IDLE:rg-desk24merge-A-idle.png:0.5" "C_SUBDIR:rg-desk24merge-C-subdir.png:0.5" "D_COPYBOX:rg-desk24merge-D-copybox.png:0.5" "E_CONFLICT:rg-desk24merge-E-conflict.png:0.5" "F_MERGED:rg-desk24merge-F-merged.png:0.5"

run_case rg-desk24rename 135 "A_IDLE:rg-desk24rename-A-idle.png:0.5" "D_COPYBOX:rg-desk24rename-D-copybox.png:0.5" "E_CONFLICT:rg-desk24rename-E-conflict.png:0.5" "F_TYPED:rg-desk24rename-F-typed.png:0.5" "G_RENAMED:rg-desk24rename-G-renamed.png:0.5"

run_case rg-desk25no 120 "A_IDLE:rg-desk25no-A-idle.png:0.5" "C_RMVDEL:rg-desk25no-C-rmvdel.png:0.5" "D_DELBOX:rg-desk25no-D-delbox.png:0.5" "E_STDELDIS:rg-desk25no-E-stdeldis.png:0.5" "F_ANSWERED_NO:rg-desk25no-F-answered.png:0.5"

run_case rg-desk25yes 120 "A_IDLE:rg-desk25yes-A-idle.png:0.5" "C_RMVDEL:rg-desk25yes-C-rmvdel.png:0.5" "D_DELBOX:rg-desk25yes-D-delbox.png:0.5" "E_STDELDIS:rg-desk25yes-E-stdeldis.png:0.5" "F_ANSWERED_YES:rg-desk25yes-F-answered.png:0.5"

run_case rg-desk26 115 "A_IDLE:rg-desk26-A-idle.png:0.5" "D_SELECTED:rg-desk26-D-selected.png:0.5" "E_DELBOX:rg-desk26-E-delbox.png:0.5" "F_DELETED:rg-desk26-F-deleted.png:0.5"

run_case rg-d19full 80 "A_IDLE:rg-d19full-A-idle.png:0.5" "B_WIN:rg-d19full-B-win.png:0.5" "C_DROPPED:rg-d19full-C-dropped.png:0.5"

run_case rg-d19file 120 "A_IDLE:rg-d19file-A-idle.png:0.5" "C_CONF:rg-d19file-C-conf.png:0.5" "D_FILENAME:rg-d19file-D-filename.png:0.5" "F_WIN:rg-d19file-F-win.png:0.5" "G_DROPPED:rg-d19file-G-dropped.png:0.5"

run_case rg-movedrvicon 60 "A_IDLE:rg-movedrvicon-A-idle.png:0.5" "B_MOVED:rg-movedrvicon-B-moved.png:0.5"

echo "BATCH1B DONE"
