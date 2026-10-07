#!/usr/bin/env bash
# series-all.sh <scratch> -- MEMPROT-FAST: the whole protection series with the
# verification mode on (memprot=verify), one QEMU at a time:
#   1. <scratch>/ser = measurement-2026-10-06-arch03-4 method/bin copied
#      (method/arch03-4-adaptations.txt), series-mf.sh on off strict on-asid
#   2. <scratch>/ls  = measurement-2026-10-06-libshare method/bin copied +
#      ls-runs-mf.sh (LDGTEST, LDGDEMO popup + acceptance, zView start)
#      in on strict off on-asid
set -uo pipefail
S="${1:?scratch}"
export SOCKROOT="$S/ser/sk"; mkdir -p "$SOCKROOT"
echo "=== arch03-4 series $(date)"
bash "$S/ser/method/series-mf.sh" "$S/ser/cards" "$S/ser/build" on off strict on-asid
export SOCKROOT="$S/ls/sk"; mkdir -p "$SOCKROOT"
for m in on strict off on-asid; do
  echo "=== ldg/zview $m $(date)"
  bash "$S/ls/method/ls-runs-mf.sh" "$S/ls/k" "$m" ldg popup accept2 zview
done
echo "=== all done $(date)"
