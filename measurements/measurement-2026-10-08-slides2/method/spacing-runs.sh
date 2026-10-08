#!/usr/bin/env bash
# spacing-runs.sh <scratch root> <N|none...> -- render the real deck with "@spacing N"
# ("none" = the deck as is, no directive -> screendumps nodirective-slide-XX.png).
# Per N: a scratch copy of docs/presentation/ofam-2026-slides.txt with the header
# line "@spacing N" put in front (the deck itself is not edited), installed as
# C:\Apps\PRESENT\SLIDES.TXT + docs/presentation/images/*.PNG beside it, on an APFS
# clone (cp -c) of <S>/base.img (tools/make-sd-image.sh <S>/base.img 64, which
# carries the rebuilt sdk/apps/present/build/PRESENT.NAP); #Z autorun of
# PRESENT.NAP (mkcard.sh); 1920x1080 raspi4b, serial=on, injector kernel
# <S>/k-kinj-nav.img (make-inj-input01.sh <S> <repo> k- nav). Dumps are renamed
# to spacing<N>-slide-XX.png. The card is deleted after the run.
set -uo pipefail
S="${1:?scratch}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="$HERE/../screendumps"; C="$S/cards"; mkdir -p "$C"
for N in "$@"; do
  T="spacing$N"
  if [ "$N" = none ]; then   # control: the deck unchanged, no directive (default must equal spacing 4)
    T=nodirective; cp "$WS/docs/presentation/ofam-2026-slides.txt" "$S/SLIDES-$N.TXT"
  else
    { echo "@spacing $N"; cat "$WS/docs/presentation/ofam-2026-slides.txt"; } > "$S/SLIDES-$N.TXT"
  fi
  args=("$S/SLIDES-$N.TXT:Apps/PRESENT/SLIDES.TXT")
  for p in "$WS"/docs/presentation/images/*.PNG; do args+=("$p:Apps/PRESENT/$(basename "$p")"); done
  bash "$HERE/mkcard.sh" "$S" "$C/$T.img" 'C:\Apps\PRESENT\PRESENT.NAP' "${args[@]}"
  XR=1920 YR=1080 APPEND=serial=on bash "$HERE/run-rpi4-drive-m3.sh" "$S/k-kinj-nav.img" "$C/$T.img" "$T" \
    --mark-delay 0.1 --end '\[m3\] MARK END_DESKTOP' --end-delay 1 --timeout 240
  rm -f "$C/$T.img" "$S/NEWDESK.INF"
  for i in 01 02 03 04 05 06 07 08 09; do
    [ -f "$OUT/$T-S$i.png" ] && mv "$OUT/$T-S$i.png" "$OUT/$T-slide-$i.png"
  done
  [ -f "$OUT/$T-END_DESKTOP.png" ] && mv "$OUT/$T-END_DESKTOP.png" "$OUT/$T-end-desktop.png"
done
