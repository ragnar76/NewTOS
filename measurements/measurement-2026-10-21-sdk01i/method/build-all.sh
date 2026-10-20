#!/usr/bin/env bash
# build-all.sh <label> <cwd> <logdir>
# Runs every SDK build script by ABSOLUTE path with <cwd> as the working
# directory, one after the other, and prints "<rc> <script>" per script.
# Library builders first (each called directly rebuilds its archive), then the
# programs. Logs: <logdir>/<label>/<n>-<name>.log (scratch, not in the run dir).
set -u
LABEL=$1; CWD=$2; LOGS=$3/$1
WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"    # repository root
S="$WS/sdk"
mkdir -p "$LOGS"
# sdk/build.sh defaults to /tmp/HELLO2.NCP and fsbench to mktemp in TMPDIR:
# both redirected into the scratch tree so nothing lands in shared /tmp.
export TMPDIR="$3/tmp"; mkdir -p "$TMPDIR"
LIST=(
  "libc/build-libc.sh"
  "gemlib/build-gemlib.sh"
  "cflib/build-cflib.sh"
  "libm/build-libm.sh"
  "ldg/build-ldg.sh"
  "zlib/build-zlib.sh"
  "libpng/build-libpng.sh"
  "libjpeg/build-libjpeg.sh"
  "giflib/build-giflib.sh"
  "libexif/build-libexif.sh"
  "libshare/build-libshare.sh"
  "windom/build-windom.sh"
  "ldg/build-ldg-demo.sh"
  "ldg/build-ldg-client.sh"
  "apps/build-qed.sh"
  "apps/build-teradesk.sh"
  "apps/build-coremark.sh"
  "apps/build-doom.sh"
  "apps/apitest/build-apitest.sh"
  "apps/chaintest/build-chaintest.sh"
  "apps/gemtest/build-gemtest.sh"
  "apps/present/build-present.sh"
  "zview/build-zview.sh"
  "examples/fsbench/build-fsbench.sh"
  "examples/libtest/build-libtest.sh"
  "build.sh"
  "gemlib/build-gemlib-m68k.sh"
  "apps/build-doom-m68k.sh"
)
n=0
for s in "${LIST[@]}"; do
  n=$((n+1)); log="$LOGS/$(printf %02d $n)-$(basename "$s" .sh).log"
  args=()
  [ "$s" = build.sh ] && args=("$S/examples/hello.c" "$3/out/$LABEL-HELLO2.NCP")
  # build-present.sh is committed without the x bit (mode 100644): run it
  # through bash, still by absolute path; every other script is exec'd.
  run=(); [ -x "$S/$s" ] || run=(bash)
  ( cd "$CWD" && ${run[@]+"${run[@]}"} "$S/$s" ${args[@]+"${args[@]}"} ) > "$log" 2>&1
  printf '%3d %s%s\n' $? "sdk/$s" "${run:+   (via bash: not executable)}"
done
# the host tests -- both from <cwd>
( cd "$CWD" && "$S/templates/tests/newtos-cc-host.sh" ) > "$LOGS/90-newtos-cc-host.log" 2>&1
printf '%3d %s\n' $? "sdk/templates/tests/newtos-cc-host.sh"
( cd "$CWD" && "$S/run-host-tests.sh" ) > "$LOGS/91-run-host-tests.log" 2>&1
printf '%3d %s   (%s)\n' $? "sdk/run-host-tests.sh" "$(grep -E '^host tests:' "$LOGS/91-run-host-tests.log" | tail -1)"
