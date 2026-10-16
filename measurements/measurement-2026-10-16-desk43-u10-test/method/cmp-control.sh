#!/usr/bin/env bash
# cmp-control.sh -- this run's dumps vs the same run on the CONTROL kernels (U9 base dab1fe62, same card, mode on). One line per pair.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"
for n in 1-text 2-a 3-b 4-c 5-d 6-e 7-f 8-g 9-config 10-end; do echo "views-el0-on-$n vs vold: $(python3 "$HERE/pixdiff.py" "$D/vold-el0-on-$n.png" "$D/views-el0-on-$n.png" | head -1)"; done
while read -r o n; do echo "$n vs $o: $(python3 "$HERE/pixdiff.py" "$D/$o.png" "$D/$n.png" --boxes | tr '\n' ' ' | cut -c1-260)"; done <<'EOF'
crchwold-el0-on-start crchw-el0-on-start
crchwold-el0-on-report crchw-el0-on-report
crchwold-el0-on-end crchw-el0-on-end
ri3old-el0-on-A-start ri3-el0-on-A-start
ri3old-el0-on-B-closed ri3-el0-on-B-closed
ri3old-el0-on-B2-fsel ri3-el0-on-B2-fsel
ri3old-el0-on-C-read ri3-el0-on-C-read
f1crashold-el0-on-app f1crash-el0-on-app
f1crashold-el0-on-end f1crash-el0-on-end
EOF
