#!/usr/bin/env bash
# build-programs.sh -- DESK43 U6a: build HELLOGEM.NAP and its panic-test twin
# HGPANIC.NAP with programs/build.sh (the shared tree, unchanged), plus the C
# reference HELLO2.NCP with sdk/build.sh, into ../bin (gitignored); then the
# ELF headers / dynamic sections / relocations of all three into ../checks.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
WS="$(cd "$M/../.." && pwd)"
B="$M/bin"; C="$M/checks"; mkdir -p "$B" "$C"
"$WS/programs/build.sh" hello-gem "$B/HELLOGEM.NAP" > "$C/u6a-build-hellogem.txt" 2>&1
"$WS/programs/build.sh" hello-gem "$B/HGPANIC.NAP" -- --features panic-test > "$C/u6a-build-hgpanic.txt" 2>&1
"$WS/sdk/build.sh" "$WS/sdk/examples/hello.c" "$B/HELLO2.NCP" > "$C/u6a-build-hello2-c.txt" 2>&1
RO="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-readobj"
for f in HELLOGEM.NAP HGPANIC.NAP HELLO2.NCP; do
  "$RO" -h -l -d -r "$B/$f" > "$C/u6a-readobj-$f.txt"
done
"$RO" -h -l -d "$WS/sdk/apps/qed/build/QED.NAP" > "$C/u6a-readobj-QED.NAP-reference.txt"
"$RO" -r "$WS/sdk/apps/qed/build/QED.NAP" | awk '/R_AARCH64/ {print $2}' | sort | uniq -c > "$C/u6a-reloc-types-QED.NAP-reference.txt"
for f in HELLOGEM.NAP HGPANIC.NAP; do
  "$RO" -r "$B/$f" | awk '/R_AARCH64/ {print $2}' | sort | uniq -c > "$C/u6a-reloc-types-$f.txt"
done
(cd "$B" && shasum -a 256 HELLOGEM.NAP HGPANIC.NAP HELLO2.NCP; ls -l HELLOGEM.NAP HGPANIC.NAP HELLO2.NCP)
