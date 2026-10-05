#!/usr/bin/env bash
# substep.sh <label> <scratch-root> -- everything a sub-step is checked with:
# static checks (checks.sh), the probes (build-probes.sh; CASES=<list> limits
# UACC3A to the sites converted so far), the scratch kernels
# (scratch-kernels.sh; the crash kernel embeds the probes for virt), then the
# QEMU series (runs-3a.sh). SKIP_CHECKS=1 skips checks.sh (already run).
# Summary of each part to stdout; details under ../checks and ../serial.
set -uo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[ "${SKIP_CHECKS:-0}" = 1 ] || bash "$HERE/checks.sh" "$L"
bash "$HERE/build-probes.sh" "$HERE/../bin" "${CASES:-}" > "$HERE/../checks/$L-probes.txt" 2>&1 && echo "probes built (${CASES:-all})" || { echo "probe build FAILED"; tail "$HERE/../checks/$L-probes.txt"; exit 1; }
bash "$HERE/scratch-kernels.sh" "$L" "$R/scr"
bash "$HERE/runs-3a.sh" "$L" "$R/cards" > "$HERE/../checks/$L-runs.txt" 2>&1
S="$HERE/../serial"
echo "runs: $(grep -c 'rc=0' "$HERE/../checks/$L-runs.txt") of $(grep -c '^run ' "$HERE/../checks/$L-runs.txt") ended normally"
for f in "$S/$L"-*-serial.log; do
  printf '%-34s %s\n' "$(basename "$f" -serial.log)" "$(grep -a -o -E 'desktop up|VERIFIED via [^ ]+|TOTAL +[0-9]+ +[0-9]+ +[0-9]+|[A-Z0-9]+: (PASS|FAIL)|S2ALL: (PASS|FAIL.*)|UACC3A: (PASS|FAIL)|UACC3A: case . rc=[^ ]+ WRONG|port10: (ALL PASS.*|[0-9]+ of [0-9]+ checks FAILED)|=== [A-Z0-9/ ]+ (PASS|FAIL) ===|\[sw3b\][^:]*: FAIL' "$f" | sort | uniq -c | tr '\n' ';' | tr -s ' ')"
done
