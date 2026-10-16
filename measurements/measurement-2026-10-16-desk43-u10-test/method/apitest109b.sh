#!/usr/bin/env bash
# apitest109.sh -- APITEST rebuilt from the HEAD source (sdk/apps/apitest/build-apitest.sh -> scratch; the sdk copy dated 2026-10-13 is stale = 106 tests)
# on card apitest109b.img, product kernel kernel8-u10t.img, modes on off strict on-asid verify (EL0 NEWDESK; there is no other desktop path).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in ${MODES:-on off strict on-asid verify}; do
  M="$m"; . "$HERE/mode.sh"
  TMO=200 bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-u10t.img" /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/apitest109b.img "api109b-$m" 'TOTAL +[0-9]+@1=3:t.png'
done
