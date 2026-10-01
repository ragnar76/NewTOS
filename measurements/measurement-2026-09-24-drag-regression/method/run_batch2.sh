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

run_case rg-d19full 180 "A_IDLE:rg-d19full-A-idle.png:0.5" "B_WIN:rg-d19full-B-win.png:0.5" "C_DROPPED:rg-d19full-C-dropped.png:0.5"

run_case rg-d19file 240 "A_IDLE:rg-d19file-A-idle.png:0.5" "C_CONF:rg-d19file-C-conf.png:0.5" "D_FILENAME:rg-d19file-D-filename.png:0.5" "F_WIN:rg-d19file-F-win.png:0.5" "G_DROPPED:rg-d19file-G-dropped.png:0.5"

echo "BATCH2 DONE"
