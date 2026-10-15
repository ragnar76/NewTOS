#!/usr/bin/env bash
# apps-summary.sh -- DESK43 U9-5: per app/crash run the serial verdict lines and the EL0-vs-EL1 screendump comparison
# (same kernel, card, mode, second). -> stdout (checks/acc-apps-summary.txt)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
for f in "$S"/f1*-serial.log "$S"/acc-*-serial.log "$S"/kp-*-serial.log "$S"/kc-*-serial.log "$S"/km-*-serial.log "$S"/st-*-serial.log; do
  [ -e "$f" ] || continue
  t="$(basename "$f" -serial.log)"
  echo "== $t"
  echo "   launches(EL0 slot)=$(grep -ac '\[launch\] EL0: slot' "$f") desktop-up=$(grep -ac 'desktop up' "$f") load-failed=$(grep -ac 'load failed' "$f") EXC=$(grep -acE '\[EXC\]|\[crash\]' "$f") rust-panic=$(grep -ac 'Rust panic' "$f") prompts=$(grep -ac 'Press any key' "$f")"
  grep -aE "exited with rc=|could not be loaded|TOTAL|CRASHTST: (PASS|FAIL)|\[HELLOGEM\]|\[acc\] |desk-selftest\] (PASS|FAIL|summary|done)" "$f" | tr -d '\r' | sed 's/^/   /' | head -20
done
echo "=== screendumps EL0 vs EL1"
for f in "$D"/f1*-el0-*.png "$D"/acc-el0-*.png "$D"/kc-el0-*.png "$D"/km-el0-*.png; do
  [ -e "$f" ] || continue
  g="${f/-el0-/-el1-}"; [ -e "$g" ] || { echo "$(basename "$f"): no EL1 twin"; continue; }
  if cmp -s "$f" "$g"; then echo "$(basename "$f"): EL0 = EL1"; else echo "$(basename "$f"): EL0 != EL1 $(python3 "$HERE/pixdiff.py" "$g" "$f" --boxes | tr '\n' ' ')"; fi
done
