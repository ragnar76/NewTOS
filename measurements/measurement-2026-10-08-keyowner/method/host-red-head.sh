#!/usr/bin/env bash
# host-red-head.sh <scratch root> -- the new host tests against HEAD's code:
# <S>/head-src (git archive HEAD, make-kernels-keyowner.sh) gets this tree's
# tos-api/tests/aes_key_owner.rs and gemdos_ssystem_sysinfo.rs, run with a
# scratch target dir. Expected: red (HEAD hands keys to whoever polls first;
# HEAD answers Ssystem 2020 with EINVFN).
set -uo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
cp "$WS/tos-api/tests/aes_key_owner.rs" "$WS/tos-api/tests/gemdos_ssystem_sysinfo.rs" "$S/head-src/tos-api/tests/"
cd "$S/head-src" && for t in aes_key_owner gemdos_ssystem_sysinfo; do
  echo "== $t against HEAD $(cat "$S/head-src.rev")"
  CARGO_TARGET_DIR="$S/target-hosthead" cargo test -p tos-api --test $t 2>&1 | grep -E "^test |^error|test result" 
done
