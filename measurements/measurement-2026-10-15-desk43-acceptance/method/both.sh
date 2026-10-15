#!/usr/bin/env bash
# both.sh <kernel-id> <card.img> <name> <mode> <SECONDS:PNG>... -- DESK43 U9-5: one card on one kernel
# (bin/kernel8-<kernel-id>.img), run twice: EL0 NEWDESK (tag <name>-el0-<mode>) and `desk=kernel` (tag <name>-el1-<mode>).
# EXTRA_APPEND adds boot arguments to both. cwd /tmp/m5j/a15.
set -uo pipefail
id="${1:?}"; CARD="${2:?}"; N="${3:?}"; m="${4:?}"; shift 4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$m"; . "$HERE/mode.sh"
BASEARGS="$(echo "$APPEND ${EXTRA_APPEND:-}" | xargs)"
for p in ${PATHS:-el0 el1}; do
  if [ $p = el1 ]; then export APPEND="$BASEARGS desk=kernel"; else export APPEND="$BASEARGS"; fi
  bash "$HERE/run-rpi4.sh" "$HERE/../bin/kernel8-$id.img" "$CARD" "$N-$p-$m" "$@"
done
