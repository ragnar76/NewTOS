#!/usr/bin/env bash
# card.sh <p4-bin-dir> <img> -- the EmuCON card: the PROC04 probe card of measurement-2026-10-18-proc04-redir-test
# (method/curdir/card.sh: P4Z.NAP #Z -> P4Z2.NAP -> shel_write(EMUCON) in C:\) plus TABS.TXT and LONG.TXT in C:\ and C:\SUB (EmuCON starts in C:\SUB).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$1"
python3 -I "$HERE/mklong.py" "$S/LONG.TXT"
bash "$HERE/mkcard.sh" "$2" "$HERE/p4.inf" AUTO/P4AUTO.NCP="$B/P4AUTO.NCP" P4Z.NAP="$B/P4Z.NAP" P4Z2.NAP="$B/P4Z2.NAP" \
  ACC/P4ACC.NAC="$B/P4ACC.NAC" SUB/P4E.NCP="$B/P4E.NCP" SUB/P4KA.NCP="$B/P4KA.NCP" SUB/P4KZ.NAP="$B/P4KZ.NAP" \
  SUB/P4INNER.TXT="$B/P4INNER.TXT" TABS.TXT="$HERE/TABS.TXT" LONG.TXT="$S/LONG.TXT" \
  SUB/TABS.TXT="$HERE/TABS.TXT" SUB/LONG.TXT="$S/LONG.TXT"
shasum -a 256 "$S/LONG.TXT" "$HERE/TABS.TXT" "$B"/*
