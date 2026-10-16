#!/usr/bin/env bash
# (LIBC-FSTAT: copied from measurement-2026-10-16-proc06/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf, kernel id lf, reference proc06; changes listed in RUNINFO.TXT.)
# (PROC06: copied from measurement-2026-10-16-t16-desk60/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf, kernel id p06; changes listed in RUNINFO.TXT.)
# (T16+DESK60: copied from measurement-2026-10-16-desk58/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/t16d60, kernel id t16d60.)
# (DESK58 copy of measurement-2026-10-16-desk59/method (itself a copy of measurement-2026-10-15-desk43-acceptance/method); scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf instead of a15.)
# mrun.sh <kernel8.img> <card.img> <tag> STEP...  -- headless QEMU raspi4b (APPEND as run-rpi4.sh), serial to a file, marker-keyed
# screendumps via mdrive.py (STEP syntax there). KEEP=1 keeps the card copy; TMO=<s> per-step marker timeout. One QEMU at a time.
set -uo pipefail
K="$1"; SD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"; SER="$HERE/../serial"; mkdir -p "$OUT" "$SER" /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf/s
LOG="$SER/$TAG-serial.log"
SDIR="$(mktemp -d /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf/s/q.XXXXXX)"
CARD="/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf/run-$TAG.img"; cp "$SD" "$CARD"; rm -f "$LOG"
# QMP socket path RELATIVE to the run's scratch dir (sun_path limit; no /tmp symlink).
cd "$SDIR"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" \
  -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
python3 "$HERE/mdrive.py" qmp.sock "$OUT" "$TAG" "$LOG" --timeout "${TMO:-150}" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd "$HERE"; rm -rf "$SDIR"; [ "${KEEP:-}" = 1 ] || rm -f "$CARD"
echo "run $TAG: rc=$RC; battery: $(pmset -g batt | sed -n 1p | cut -c1-40)"
