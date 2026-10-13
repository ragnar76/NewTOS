#!/usr/bin/env bash
# build-extra.sh -- (LIBC-TTY-ACCEPT) the helper binaries the aux-acceptance had reused or built separately:
# bin/j (PPZJ, ENVKID J-variant, PPDESKJ), bin/m (ENVKID M-variant), STDERRQ/STDERRO (EmuCON stderr, new/old crt0),
# bin/oldcrt0/STDERRP.NAP (old crt0 = crt0-before-libc-main.S, relinked against the CURRENT libc.a).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
mkdir -p "$B/j" "$B/m" "$B/oldcrt0"
bash "$HERE/build-ppj.sh" "$B/j" || exit 1
bash "$HERE/build-envkidm.sh" "$B/m" || exit 1
bash "$HERE/build-stderrq.sh" "$B" || exit 1                      # STDERRQ.NAP new crt0
CRT0="$HERE/crt0-before-libc-main.S" bash "$HERE/build-stderrq.sh" "$B/oldcrt0" || exit 1   # old crt0 STDERRQ
mv "$B/oldcrt0/STDERRQ.NAP" "$B/STDERRO.NAP"
# old-crt0 STDERRP (with spawn): same recipe, other source flags
sed -e 's/^build stderrp STDERRQ.NAP -DSTDERRP_NOSPAWN$/build stderrp STDERRP.NAP/' "$HERE/build-stderrq.sh" > "$HERE/.build-stderrp-old.sh"
CRT0="$HERE/crt0-before-libc-main.S" bash "$HERE/.build-stderrp-old.sh" "$B/oldcrt0" || exit 1
rm -f "$HERE/.build-stderrp-old.sh"
(cd "$B" && shasum -a 256 j/* m/* oldcrt0/* STDERRQ.NAP STDERRO.NAP > SHA256SUMS-extra)
echo extra-done
