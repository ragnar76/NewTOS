#!/usr/bin/env bash
# runs-headless.sh <scratch> <kernel8.img> <label> -- APITEST and S2ALL (#Z)
# with a product kernel, memprot on, on APFS clones of the two base cards
# seeded with measurement-2026-10-06-vforkt-fix/method/seed-card.sh
# (<scratch>/base-api.img: #Z C:\APITEST.NCP; <scratch>/base-s2all.img:
# #Z C:\S2ALL.NAP; programs = that run's bin/, APITEST = sdk/apps/apitest).
# Serial verdict lines, fsck, card hash, card deleted (run-gui.sh).
set -uo pipefail
S="$1"; K="$2"; L="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TMPDIR="$S"; unset CHECK
cp -c "$S/base-api.img" "$S/cards/$L-api.img"
bash "$HERE/run-gui.sh" "$S" "$K" "$S/cards/$L-api.img" "$L-api" --end 'TOTAL +[0-9]+' --end-delay 3 --timeout 180 2>&1 | grep -E "^run |^==|Warning|differ|orphan|lost|Cross"
grep -a "TOTAL" "$S/serial/$L-api-serial.log"
cp -c "$S/base-s2all.img" "$S/cards/$L-s2all.img"
bash "$HERE/run-gui.sh" "$S" "$K" "$S/cards/$L-s2all.img" "$L-s2all" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400 2>&1 | grep -E "^run |^==|Warning|differ|orphan|lost|Cross"
grep -aE "S2ALL: |FAIL" "$S/serial/$L-s2all-serial.log" | head
