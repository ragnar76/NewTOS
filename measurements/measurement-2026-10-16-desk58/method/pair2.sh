#!/usr/bin/env bash
# pair2.sh <kernel-id> <card.img> <name> <mode> STEP... -- one card on bin/kernel8-<id>.img, run with the EL0 NEWDESK (tag <name>-el0-<mode>)
# and with desk=kernel (tag <name>-el1-<mode>); PATHS="el0" limits; EXTRA_APPEND adds boot arguments; marker-keyed steps (mdrive.py).
set -uo pipefail
id="${1:?}"; CARD="${2:?}"; N="${3:?}"; m="${4:?}"; shift 4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$m"; . "$HERE/mode.sh"
BASEARGS="$(echo "$APPEND ${EXTRA_APPEND:-}" | xargs)"
for p in ${PATHS:-el0 el1}; do
  if [ $p = el1 ]; then export APPEND="$BASEARGS desk=kernel"; else export APPEND="$BASEARGS"; fi
  bash "$HERE/mrun.sh" "$HERE/../bin/kernel8-$id.img" "$CARD" "$N-$p-$m" "$@"
done
