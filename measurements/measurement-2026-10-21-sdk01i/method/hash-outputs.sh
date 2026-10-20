#!/usr/bin/env bash
# hash-outputs.sh <stamp-file>
# sha256 of every program/archive under sdk/ written after <stamp-file>
# (= by the build-all.sh run that started after it), repo-relative, sorted.
set -u
WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$WS"
find sdk -type f -newer "$1" \( -name '*.a' -o -name '*.NCP' -o -name '*.NAP' \
     -o -name '*.NLD' -o -name '*.NAC' -o -name '*.NTP' -o -name '*.PRG' -o -name '*.TOS' \) \
  | LC_ALL=C sort | while read -r f; do shasum -a 256 "$f"; done
