#!/usr/bin/env bash
# probes2.sh <mode> <path el0|el1> [names] -- SDK probe programs autorun from the card's #Z (cards /tmp/m5j/acc/cards = MEM05 acceptance),
# kernel bin/kernel8-acc.img, serial-marker driven (run-rpi4-drive.sh/drive.py --end REGEX, recipes of mem05-acceptance acc-series.sh).
# tag p-<path>-<mode>-rpi4-<name>. S2ALL strict needs the crash kernel (not built here) -> skipped under strict.
set -uo pipefail
m="${1:?}"; PP="${2:?}"; shift 2; ONLY="${*:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/tmp/m5j/acc/cards; K="$HERE/../bin/kernel8-acc.img"
M="$m"; . "$HERE/mode.sh"; A4B="$A4"; [ $PP = el1 ] && A4B="$A4 desk=kernel"
T="p-$PP-$m"
want() { [ -z "$ONLY" ] || [[ " $ONLY " == *" $1 "* ]]; }
rd() { APPEND="$A4B${XA:+ $XA}" bash "$HERE/run-rpi4-drive.sh" "$@"; }
want accfree && rd "$K" "$C/accfree.img" "$T-rpi4-accfree" --end 'ACCFREE: (PASS|FAIL)' --end-delay 2 --timeout 400
want s2allm && [ "$BASE" != strict ] && rd "$K" "$C/s2allm.img" "$T-rpi4-s2allm" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
want e9mem && rd "$K" "$C/e9mem.img" "$T-rpi4-e9mem" --end '\[E9MEM\] E9MEM: (PASS|FAIL)' --end-delay 2 --timeout 400
want stdoutdb && rd "$K" "$C/stdoutdb.img" "$T-rpi4-stdoutdb" --end 'DBG: DONE' --end-delay 2 --timeout 200
want stderr && rd "$K" "$C/serr.img" "$T-rpi4-stderr-newcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
want stderr && rd "$K" "$C/serr-oldcrt0.img" "$T-rpi4-stderr-oldcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
want stderr && rd "$K" "$C/stderra.img" "$T-rpi4-stderra" --end 'STDERRA\[PAR\]: DONE' --end-delay 3 --timeout 150
want ttya && rd "$K" "$C/ttya.img" "$T-rpi4-ttya" --end 'TTYPROBE: (PASS|FAIL)' --end-delay 2 --timeout 200
want port10 && rd "$K" "$C/p10/rerun-${BASE/verify/on}/p10.img" "$T-rpi4-port10" --end 'port10: (ALL PASS|[0-9]+ of [0-9]+ checks FAILED)' --end-delay 2 --timeout 240
want pparent && rd "$K" "$C/pparent.img" "$T-rpi4-pparent" --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 300
want mdpar && XA="mdlimit=${MDPAR_LIMIT:-64}" rd "$K" "$C/mdpar.img" "$T-rpi4-mdpar" --end 'MDPAR: (PASS|FAIL)' --end-delay 3 --timeout ${PT:-800} --stamp
want mdstep && XA="mdlimit=128" rd "$K" "$C/mdstep.img" "$T-rpi4-mdstep" --end 'MDSTEP: DONE' --end-delay 3 --timeout 3000 --stamp
