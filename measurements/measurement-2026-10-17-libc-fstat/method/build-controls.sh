#!/usr/bin/env bash
# build-controls.sh <outdir> <old-libc.a> -- LIBC-FSTAT red/A controls: the serial-verdict probes of this run linked against the
# libc.a BEFORE LIBC-FSTAT (scratch copy of sdk/libc/libc.a taken before the rebuild, sha256 313a7bc0...), so the A/B pair differs
# only in newtos-arch/do_fstat.c. Uses sed copies of build-s2-tests.sh / build-fs11.sh / build-tty.sh / build-stderrq.sh in which
# "$LIBC/libc.a" becomes "${LIBCA:-$LIBC/libc.a}" (copies deleted afterwards):
#   STDOUTDBG.NAP (+ STDOUTDB.NAP copy), STDERRP.NAP, STDERRA.NAP, TTYA.NAP, TTYPROBE.NAP, oldcrt0/STDERRP.NAP, FSTATP.NAP
#   (and the other s2 probes the s2 script builds as a side effect).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="${1:?outdir}"; export LIBCA="${2:?old libc.a}"; mkdir -p "$B/oldcrt0"; B="$(cd "$B" && pwd)"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1791979200}"
for s in build-s2-tests build-fs11 build-tty build-stderrq; do
  sed 's|"\$LIBC/libc.a"|"${LIBCA:-$LIBC/libc.a}"|' "$HERE/$s.sh" > "$HERE/.ctl-$s.sh"
  [ "$(grep -cF '${LIBCA:-$LIBC/libc.a}' "$HERE/.ctl-$s.sh")" = 1 ] || { echo "sed failed: $s"; exit 1; }
done
sed -i '' -e 's/^build stderrp STDERRQ.NAP -DSTDERRP_NOSPAWN$/build stderrp STDERRP.NAP/' "$HERE/.ctl-build-stderrq.sh"
S2_TESTS="stdoutdbg stderrp" bash "$HERE/.ctl-build-s2-tests.sh" "$B" > "$B/ctl-s2.log" 2>&1 || { echo "FAIL s2"; exit 1; }
cp "$B/STDOUTDBG.NAP" "$B/STDOUTDB.NAP"
bash "$HERE/.ctl-build-fs11.sh" "$B" || exit 1
bash "$HERE/.ctl-build-tty.sh" "$B" || exit 1
CRT0="$HERE/crt0-before-libc-main.S" bash "$HERE/.ctl-build-stderrq.sh" "$B/oldcrt0" || exit 1
rm -f "$HERE"/.ctl-*.sh
bash "$HERE/build-fstatp.sh" "$B" "$LIBCA" > /dev/null && mv "$B/FSTATP-OLD.NAP" "$B/FSTATP.NAP"
(cd "$B" && shasum -a 256 STDOUTDB.NAP STDERRP.NAP STDERRA.NAP TTYA.NAP TTYPROBE.NAP oldcrt0/STDERRP.NAP FSTATP.NAP > SHA256SUMS)
shasum -a 256 "$LIBCA"
