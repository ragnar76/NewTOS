#!/usr/bin/env bash
# runs-all.sh <scratch root> <head|new> -- every KEYOWNER part of
# keyowner-runs.sh for one kernel set, serially (one QEMU at a time), then
# the exception count of every serial log of that set.
set -uo pipefail
S="${1:?scratch root}"; P="${2:?head|new}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PARTS="sikeys si si-aes sitera siqed emucon acc-on acc-strict desk tera qed desk3a desk3a-acc api zv-dclick zv-browser zv-quit zvfull doom deskw qedw qedo teraw"
bash "$HERE/keyowner-runs.sh" "$S" "$P" $PARTS
echo "=== exceptions per serial log ($P)"
for f in "$HERE/../serial/$P"-*-serial.log; do
  printf '%s %s\n' "$(grep -caE '\[EXC\]|PANIC|Memory violation' "$f")" "$(basename "$f")"
done
