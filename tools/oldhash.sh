#!/bin/sh
# Translate a commit hash (prefix) from before the 2026-10-19 history rewrite
# into the current one. Usage: tools/oldhash.sh ba40722f
set -e
map="$(dirname "$0")/../docs/commit-map.txt"
[ -n "$1" ] || { echo "usage: $0 <old-hash-prefix>" >&2; exit 2; }
grep -v '^#' "$map" | awk -v p="$1" 'index($1, p) == 1 { print $2 }'
