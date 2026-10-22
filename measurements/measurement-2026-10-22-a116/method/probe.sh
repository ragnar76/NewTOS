#!/usr/bin/env bash
# probe.sh <side head|wt> <mode on|off|strict|verify> -- one A116 probe run on the plain kernel $S/bin/kernel8-<side>-plain.img and
# the probe card $S/cards/probe.img (APFS clone per run via run.sh). Waits for the program's "done" line (or the timeout),
# one screendump at the end. tag = probe-<side>-<mode>.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SIDE=$1; MODE=$2
case $MODE in on) EXTRA="";; *) EXTRA="memprot=$MODE";; esac
APPEND_EXTRA="$EXTRA" TMO=${TMO:-90} QDELAY=1 bash "$HERE/run.sh" "$S/bin/kernel8-$SIDE-plain.img" "$S/cards/probe.img" "probe-$SIDE-$MODE" \
  'A116W: #[0-9]+ done@1=2:end.png'
