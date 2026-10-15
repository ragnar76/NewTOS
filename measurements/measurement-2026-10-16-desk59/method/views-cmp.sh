#!/usr/bin/env bash
# views-cmp.sh -- DESK43 U9-5: the 8 views EL0 vs EL1 (same kernel kv, same mode) with pixdiff.py --boxes, and EL1
# (desk=kernel) vs the U9-0 run's views-u90 (memprot on; U9-0 ran on only). -> stdout
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"; R="$HERE/../../measurement-2026-10-14-desk43-u9-0/screendumps"
for m in on off strict; do
  for i in 1-text 2-viewmenu 3-icons 4-about 5-prefs 6-background 7-config 8-end; do
    a="$D/views-el1-$m-rpi4-$i.png"; b="$D/views-el0-$m-rpi4-$i.png"
    if cmp -s "$a" "$b"; then echo "$m $i: EL0 = EL1"; else echo "$m $i: EL0 != EL1: $(python3 "$HERE/pixdiff.py" "$a" "$b" --boxes | tr '\n' ' ')"; fi
  done
done
for i in 1-text 2-viewmenu 3-icons 4-about 5-prefs 6-background 7-config 8-end; do
  cmp -s "$D/views-el1-on-rpi4-$i.png" "$R/views-u90-rpi4-$i.png" && echo "on $i: EL1 = U9-0" || echo "on $i: EL1 != U9-0"
done
