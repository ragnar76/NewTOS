#!/usr/bin/env bash
# build-all.sh <label> -- delete the SDK archives, rebuild every library and
# app script, hash every produced archive/program. Output: $OUTDIR/<label>/
set -u
LABEL=$1
REPO=/Volumes/Daten/Development/PiRusTOS
SDK=$REPO/sdk
OUTDIR=${OUTDIR:-/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/651de586-7dd7-487b-9e92-c8525c58f508/scratchpad/k/runs}
D=$OUTDIR/$LABEL; mkdir -p "$D/logs"
STAMP=$D/stamp; : > "$STAMP"; sleep 1
for a in libc/libc.a gemlib/gemlib.a cflib/cflib.a libm/libm.a compiler-rt/libclang_rt.builtins.a \
         ldg/libldg.a zlib/libz.a libpng/libpng.a libjpeg/libjpeg.a giflib/libgif.a libexif/libexif.a \
         windom/libwindom1.a libshare/libshare.a; do rm -f "$SDK/$a"; done
cd "$D" || exit 1     # start OUTSIDE the checkout on purpose
for s in libc/build-libc.sh gemlib/build-gemlib.sh cflib/build-cflib.sh libm/build-libm.sh \
         compiler-rt/build-compiler-rt.sh ldg/build-ldg.sh zlib/build-zlib.sh libpng/build-libpng.sh \
         libjpeg/build-libjpeg.sh giflib/build-giflib.sh libexif/build-libexif.sh windom/build-windom.sh \
         libshare/build-libshare.sh \
         apps/build-qed.sh apps/build-teradesk.sh apps/build-hypview.sh zview/build-zview.sh \
         apps/build-coremark.sh apps/build-doom.sh apps/gemtest/build-gemtest.sh \
         apps/chaintest/build-chaintest.sh apps/present/build-present.sh apps/apitest/build-apitest.sh \
         examples/fsbench/build-fsbench.sh examples/libtest/build-libtest.sh \
         ldg/build-ldg-client.sh ldg/build-ldg-demo.sh; do
  n=$(echo "$s" | tr / _)
  t0=$(date +%s)
  "$SDK/$s" > "$D/logs/$n.log" 2>&1; rc=$?
  echo "$s rc=$rc $(( $(date +%s) - t0 ))s" | tee -a "$D/results.txt"
done
# sdk/build.sh (bare) and newtos-cc / newtos.mk templates
"$SDK/build.sh" "$SDK/examples/hello.c" "$D/HELLOB.NCP" > "$D/logs/build.sh.log" 2>&1; echo "build.sh rc=$?" | tee -a "$D/results.txt"
"$SDK/bin/newtos-cc" -v -bare -o "$D/HELLOC.NCP" "$SDK/examples/hello.c" > "$D/logs/newtos-cc.log" 2>&1; echo "newtos-cc rc=$?" | tee -a "$D/results.txt"
"$SDK/bin/newtos-cc" -v -lm -o "$D/MATHTST.NCP" "$SDK/examples/mathtst.c" > "$D/logs/newtos-cc-m.log" 2>&1; echo "newtos-cc -lm rc=$?" | tee -a "$D/results.txt"
for t in console-hello gem-hello; do
  rm -rf "$D/tpl-$t"; cp -R "$SDK/templates/$t" "$D/tpl-$t"
  make -C "$D/tpl-$t" NEWTOS_SDK="$SDK" > "$D/logs/tpl-$t.log" 2>&1; echo "template $t rc=$?" | tee -a "$D/results.txt"
done
# hashes: every archive / program written since the stamp
( cd "$REPO" && find sdk "$D" -newer "$STAMP" -type f \( -name '*.a' -o -name '*.NCP' -o -name '*.NAP' -o -name '*.NTP' -o -name '*.NAC' -o -name '*.NLD' -o -name '*.PRG' \) \
    -not -path '*/obj/*' | sed "s|^$D/|RUN/|" | sort | while read -r f; do
      p=$f; case $f in RUN/*) p=$D/${f#RUN/} ;; esac
      printf '%s  %s\n' "$(shasum -a 256 "$p" | cut -d' ' -f1)" "$f"; done ) > "$D/sha256.txt"
wc -l < "$D/sha256.txt"
