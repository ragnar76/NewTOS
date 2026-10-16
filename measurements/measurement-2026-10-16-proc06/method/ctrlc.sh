#!/usr/bin/env bash
# ctrlc.sh -- PROC06 on-target Ctrl-C case (spec host-test list item 8): card ctrlc.img (#Z C:\CTRLCPAR.NCP), kernel kc (fix) in
# modes on/off/strict and kc-red (fix reverted in the scratch copy) in mode on; EL0 default desktop. CTRLCPAR Pexecs PTERMCH "c", the
# injector (launch:CTRLCPAR, ctrlc.script) feeds Ctrl-C 5 s after the launch, CTRLCPAR prints "CTRLCPAR: Pexec rc=<n> [OK|FAIL...]".
# Dumps: 3 s after the CTRLCPAR line (t.png), 8 s after "[desk] desktop up" (desk.png: CTRLCPAR ended, the shell went on). One QEMU at a time.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06/cards/ctrlc.img
run() { local id="$1" m="$2"; M="$m"; . "$HERE/mode.sh"
  bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-$id.img" "$C" "ctrlc-$id-el0-$m" 'CTRLCPAR: Pexec rc=@1=3:t.png' '\[desk\] desktop up@1=8:desk.png'; }
run kc-red on
for m in on off strict; do run kc $m; done
