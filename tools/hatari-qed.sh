#!/bin/sh
# hatari-qed.sh -- start EmuTOS 1.4 on an emulated Atari TT with the m68k QED,
# so its file selector can be photographed as the REFERENCE for ours.
#
# Same machine as the dumper reference runs (TT, 640x480x16, EmuTOS 1.4 from
# the Hatari bundle, sha 5805c0a4... = bit-identical to the user's TT ROM), so
# a screenshot here is comparable to a Pi 4 screenshot at 640x480.
#
# C: is GEMDOS-emulated from ~/.nt-hatari/hd, with QED staged under C:\QED.
# Runs WITH a window: this one is meant to be clicked, not scripted.
#
#   Hatari screenshot: F12 -> Screenshot, or Alt-G. Files land in the cwd.
#   Quit: F12 -> Quit, or Alt-Q.
set -eu

HATARI="/Volumes/Daten/hatari-2.6.1_macos/Hatari.app/Contents/MacOS/Hatari"
TOS="/Volumes/Daten/hatari-2.6.1_macos/etos512de.img"
HD="$HOME/.nt-hatari/hd"

[ -x "$HATARI" ] || { echo "Hatari not found: $HATARI" >&2; exit 1; }
[ -f "$TOS" ]    || { echo "TOS image not found: $TOS" >&2; exit 1; }
[ -d "$HD/QED" ] || { echo "QED not staged in $HD/QED" >&2; exit 1; }

exec "$HATARI" \
    --machine tt \
    --tos "$TOS" \
    --harddrive "$HD" \
    --sound off \
    "$@"
