#!/usr/bin/env bash
# (copied unchanged from measurements/measurement-2026-10-14-desk43-u5a/method/ for DESK43 U6a)
# make-scratch.sh <scratch-dir> -- copy the shared tree (sources only, no
# target/.git/measurements/references/...) into <scratch-dir> for a
# scratch-instrumented build; files instrumented by an earlier run are
# overwritten with the shared tree's version (rsync --delete). The
# instrumentation is never applied to the shared tree.
set -euo pipefail
D="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
mkdir -p "$D"
rsync -a --delete --exclude target --exclude .git --exclude measurements --exclude references \
  --exclude Applications --exclude graphify-out --exclude hardware --exclude 'sd-card-backup-*' \
  --exclude tosdump --exclude sdk/apps --exclude docs "$WS/" "$D/"
