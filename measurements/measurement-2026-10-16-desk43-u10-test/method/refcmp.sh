#!/usr/bin/env bash
# refcmp.sh -- series2 dumps (EL0 and EL1) vs the MEM05 acceptance references (measurement-2026-10-13-mem05-acceptance, kernel before DESK43), pixdiff.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; R="$HERE/../../measurement-2026-10-13-mem05-acceptance/screendumps"; D="$HERE/../screendumps"
for m in on off strict on-asid verify; do for p in el0 el1; do
  for pr in "desktop-t:desktop-t25" "apitest-t-b:apitest-t40" "teradesk-t:teradesk-t35" "qed-t:qed-t30"; do a=${pr%%:*}; b=${pr##*:}
    ref="$R/acc-$m-rpi4-$b.png"; [ -f "$ref" ] || { echo "$m $p $a: no mem05 reference"; continue; }
    echo "$m $p $a vs mem05: $(python3 "$HERE/pixdiff.py" "$ref" "$D/s-$p-$m-$a.png" --boxes | tr '\n' ' ' | cut -c1-210)"; done; done; done
