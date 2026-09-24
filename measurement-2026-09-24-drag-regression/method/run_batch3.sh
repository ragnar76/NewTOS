#!/usr/bin/env bash
# Corrected wait-marker: the console-return blocking read
# (console_pause_for_key -> console_read_key_blocking) does not pump the H02
# injector (it lives outside the AES-pump anchor, confirmed by source
# inspection), so a scripted "mark" step scheduled after the drop can never
# fire once cmdline.c has returned and the desktop is showing the
# "[ Press any key ]" console prompt -- this is a genuine, pre-existing
# harness limitation (the ORIGINAL desk19-desk20 measurement hit the exact
# same ceiling: its own serial log ends at "[launch] returned", no mark
# after). The programs's OWN dual-logged Cconws output is the correct wait
# marker instead -- it appears in the log the instant Cconws is called,
# before the blocking wait even starts.
set -uo pipefail
SCR=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/d7ff6c34-253d-4fd6-839f-efa08458ba32/scratchpad/regress
M="$SCR/method"
export SCR

run_case() {
  local tag="$1" tmo="$2"; shift 2
  python3 "$M/apply_injector.py" "$SCR/tree" "$M/$tag.rs" >/dev/null
  python3 "$M/a78_patch2.py" "$SCR/tree" >/dev/null
  python3 "$M/key_step_patch.py" "$SCR/tree" >/dev/null
  ( cd "$SCR/tree" && cargo run -p xtask -- build --board rpi4 ) > "$SCR/build/scratch-build-$tag-v2.log" 2>&1
  cp "$SCR/tmp/base.img" "$SCR/tmp/$tag-v2.img"
  bash "$M/run.sh" "$SCR/tree/kernel8.img" "$SCR/tmp/$tag-v2.img" "$SCR/tmp/out" "$tag-v2" "$tmo" "$@"
}

run_case rg-d19full 60 "A_IDLE:rg-d19full-v2-A-idle.png:0.5" "B_WIN:rg-d19full-v2-B-win.png:0.5" 'CMDLINE: len 13 arg|rg-d19full-v2-C-cmdline.png|0.8'

run_case rg-d19file 60 "A_IDLE:rg-d19file-v2-A-idle.png:0.5" "C_CONF:rg-d19file-v2-C-conf.png:0.5" "D_FILENAME:rg-d19file-v2-D-filename.png:0.5" "F_WIN:rg-d19file-v2-F-win.png:0.5" 'CMDLINE: len 10 arg|rg-d19file-v2-G-cmdline.png|0.8'

echo "BATCH3 DONE"
