#!/usr/bin/env bash
# steps.sh <script> <tag> <trigger> <delay> -- prints one mdrive STEP per mark of <script>.marks: a dump <delay> s after the
# injected byte that closed the mark (marker = the N-th "[<tag>] <trigger> feed" serial line, N via fx.py).
S_="$1"; TAG="$2"; TRIG="$3"; DL="$4"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
while read -r n t; do N=$(python3 "$HERE/fx.py" "$S_" "$t"); printf '%s\n' "\\[$TAG\\] $TRIG feed@$N=$DL:$n.png"; done < "$S_.marks"
