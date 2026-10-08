#!/usr/bin/env bash
# compare-input01.sh -- INPUT01 screendump comparison (shotdiff.py, .venv PIL):
#   irq-<run>  vs aes-<run>   (same scratch cards: the only difference is the mode)
#   aes-<run>  vs ofam-freeze <run> (input=aes against the tagged demo state; the
#              ofam cards were seeded at another time, so file time stamps and
#              clock fields may differ -- every OTHER region is looked at)
# Output: ../checks/shotdiff-*.txt and a summary to stdout.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
R="$HERE/.."; O="$HERE/../../measurement-2026-10-08-ofam-freeze/screendumps"
PY="${PY:-$HERE/../../../.venv/bin/python}"
RUNS="acc-on acc-strict desk tera qed desk3a api zv-dclick zv-browser zv-quit doom"
for r in $RUNS; do
  "$PY" "$HERE/shotdiff.py" "$R/screendumps" "irq-$r-" "$R/screendumps" "aes-$r-" > "$R/checks/shotdiff-irq-vs-aes-$r.txt"
  "$PY" "$HERE/shotdiff.py" "$O" "$r-" "$R/screendumps" "aes-$r-" > "$R/checks/shotdiff-ofam-vs-aes-$r.txt"
  echo "$(tail -1 "$R/checks/shotdiff-irq-vs-aes-$r.txt")   |   $(tail -1 "$R/checks/shotdiff-ofam-vs-aes-$r.txt")"
done
