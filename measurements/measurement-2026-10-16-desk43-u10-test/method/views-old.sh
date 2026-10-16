#!/usr/bin/env bash
# views2.sh <mode> [tag-suffix] -- 8-view script on kernel kv (views.script; injector tag u7v), plain card, EL0 vs desk=kernel;
# dumps 3 s after the last injected byte of each script group (markers: the N-th "[u7v] desktop feed" line, N by fx.py). EXTRA_APPEND ok.
set -uo pipefail
m="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
read -ra NS <<< "$(python3 "$HERE/fx.py" "$HERE/views.script" 230 1310 2210 3010 3610 4410 5010 5810 6410 7210)"
names=(1-text 2-a 3-b 4-c 5-d 6-e 7-f 8-g 9-config 10-end)
S=(); for i in 0 1 2 3 4 5 6 7 8 9; do S+=("\\[u7v\\] desktop feed@${NS[$i]}=3:${names[$i]}.png"); done
PATHS="${PATHS:-el0}" bash "$HERE/pair2.sh" kvold /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/plain.img "vold${2:-}" "$m" "${S[@]}"
