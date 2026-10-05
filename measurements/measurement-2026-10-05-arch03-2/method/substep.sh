#!/usr/bin/env bash
# substep.sh <label> <scratch-root> -- everything a sub-step is checked with:
# static checks (checks.sh), the scratch kernels (scratch-kernels.sh), the
# probes (build-s2-tests.sh), then the QEMU series (runs-s2.sh). Summary of
# each part to stdout; details under ../checks and ../serial.
set -uo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$HERE/checks.sh" "$L"
bash "$HERE/scratch-kernels.sh" "$L" "$R/scr"
bash "$HERE/build-s2-tests.sh" "$HERE/../bin" > "$HERE/../checks/$L-probes.txt" 2>&1 && echo "probes built" || { echo "probe build FAILED"; tail "$HERE/../checks/$L-probes.txt"; exit 1; }
bash "$HERE/runs-s2.sh" "$L" "$R/cards" > "$HERE/../checks/$L-runs.txt" 2>&1
S="$HERE/../serial"
echo "runs: $(grep -c 'rc=0' "$HERE/../checks/$L-runs.txt") of $(grep -c '^run ' "$HERE/../checks/$L-runs.txt") ended normally"
for f in "$S/$L"-*-serial.log; do
  printf '%-34s %s\n' "$(basename "$f" -serial.log)" "$(grep -a -o -E 'desktop up|VERIFIED via [^ ]+|TOTAL +[0-9]+ +[0-9]+ +[0-9]+|[A-Z0-9]+: (PASS|FAIL)|S2ALL: (PASS|FAIL.*)|port10: (ALL PASS.*|[0-9]+ of [0-9]+ checks FAILED)|=== [A-Z0-9/ ]+ (PASS|FAIL) ===|\[sw3b\][^:]*: FAIL' "$f" | sort | uniq -c | tr '\n' ';' | tr -s ' ')"
done
