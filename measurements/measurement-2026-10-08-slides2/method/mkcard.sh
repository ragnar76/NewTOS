#!/usr/bin/env bash
# mkcard.sh <scratch> <out.img> <autorun|-> [extra mcopy src:dest ...]
# APFS clone of <scratch>/base.img (tools/make-sd-image.sh base.img 64), C:\NEWDESK.INF with
# "#Z 01 <autorun>@" (template of the PRESENT01 run; "-" = keep the base's own INF = plain EmuDESK).
S=$1; out=$2; auto=$3; shift 3
OFF=$((63*512)); here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -f "$out"; cp -c "$S/base.img" "$out"
if [ "$auto" != "-" ]; then
  sed "s|^#Z .*|#Z 01 ${auto//\\/\\\\}@|" "$here/newdesk-template.inf" > "$S/NEWDESK.INF"
  mcopy -o -i "$out@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
fi
for p in "$@"; do mcopy -o -i "$out@@$OFF" "${p%%:*}" "::${p#*:}"; done
