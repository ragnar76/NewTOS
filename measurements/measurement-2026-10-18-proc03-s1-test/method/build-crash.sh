#!/usr/bin/env bash
# build-crash.sh <outdir> -- program-exit / crash probes from sdk/examples (freestanding sdk/build.sh path):
# CRASHTST+CRCHILD, UACC3A+UAKID, MPROT3B/S/O+MPKID, E10USR/S/O+E10KID, CTRLCPAR+PTERMCH (variants as measurement-2026-10-13-libc-tty-acceptance/method/build-mprot.sh, build-e10.sh)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"; O="${1:?outdir}"; mkdir -p "$O"; W="$(mktemp -d "$O/w.XXXX")"
b() { sh "$WS/sdk/build.sh" "$1" "$2" > "$W/log" 2>&1 || { cat "$W/log"; exit 1; }; echo "built $(basename "$2")"; }
E="$WS/sdk/examples"
b $E/crashtst.c $O/CRASHTST.NAP; b $E/crchild.c $O/CRCHILD.NAP; b $E/uacc3a.c $O/UACC3A.NAP; b $E/uakid.c $O/UAKID.NAP
b $E/mpkid.c $O/MPKID.NAP; b $E/e10kid.c $O/E10KID.NAP; b $E/ctrlcpar.c $O/CTRLCPAR.NCP; b $E/ptermch.c $O/PTERMCH.NCP
for v in "0 MPROT3B" "1 MPROTS" "2 MPROTO"; do set -- $v; { echo "#define MPROT_MODE $1"; cat $E/mprot3b.c; } > $W/m$1.c; b $W/m$1.c $O/$2.NAP; done
for v in "0 E10USR" "1 E10USRS" "2 E10USRO"; do set -- $v; { echo "#define E10_MODE $1"; cat $E/e10usr.c; } > $W/e$1.c; b $W/e$1.c $O/$2.NAP; done
rm -rf "$W"; shasum -a 256 "$O"/*
