#!/bin/sh
# Find TASK/ tickets whose header contradicts their own receipt.
#
# A ticket that says OFFEN while a *-STATUS-DONE.MD sits next to it makes the
# board look fuller than the work is -- and the board is what the user reads to
# decide what happens next. Eleven of them had drifted by 2026-09-14.
#
# Exits non-zero when it finds one, so it can gate a commit if we ever want it to.
cd "$(dirname "$0")/.." || exit 2
n=0
for f in TASK/*-STATUS-DONE.MD; do
    [ -e "$f" ] || continue
    t="TASK/$(basename "$f" | sed 's/-STATUS-DONE\.MD$//').md"
    [ -f "$t" ] || continue
    if grep -q '^Status: \*\*\(OFFEN\|BACKLOG\)' "$t"; then
        echo "$t says OPEN, but $(basename "$f") says DONE"
        n=$((n + 1))
    fi
done
if [ "$n" -gt 0 ]; then
    echo "--- $n ticket(s) contradict their own receipt"
    exit 1
fi
echo "TASK/: no ticket contradicts its receipt"
