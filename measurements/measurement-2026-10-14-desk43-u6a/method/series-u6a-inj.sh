#!/usr/bin/env bash
# series-u6a-inj.sh -- the injector-kernel half of series-u6a.sh (run first,
# while checks.sh was still building the product kernel). One run at a time.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; C=/tmp/m5j/u6a
for M in on off strict; do
  case $M in on) P="";; off) P="memprot=off";; strict) P="memprot=strict";; esac
  export APPEND="$(echo "serial=on $P" | xargs)"
  for v in z f1 zpanic; do
    "$HERE/run-u6a.sh" "$B/kernel8-inj.img" "$C/card-$v.img" "inj-$M-$v" --timeout 240
  done
done
