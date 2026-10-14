#!/usr/bin/env bash
# repro-proto-wrapper.sh -- DESK43 U9-2: the PROTOTYPE of programs/rustc-stable-metadata.sh used for the reproducibility
# experiment of checks/u912-repro.txt step 2 (run as RUSTC_WRAPPER). Superseded by the product script.
set -euo pipefail
args=("$@")
n=${#args[@]}
key="${CARGO_PKG_NAME:-} ${CARGO_PKG_VERSION:-}"
for ((i = 1; i < n; i++)); do
  case "${args[$i]}" in
    --crate-type|--crate-name) key+=" ${args[$i]} ${args[$((i+1))]}";;
    --cfg) key+=" --cfg ${args[$((i+1))]}";;
  esac
done
h=$(printf '%s' "$key" | shasum -a 256 | cut -c1-16)
for ((i = 1; i < n; i++)); do
  if [ "${args[$((i-1))]}" = "-C" ] && [[ "${args[$i]}" == metadata=* ]]; then args[$i]="metadata=$h"; fi
done
exec "${args[@]}"
