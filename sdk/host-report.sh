#!/usr/bin/env bash
#
# host-report.sh -- build the SDK on this host and write one report a tester
# can send back (TASK/SDK01 K: the C path without Rust, first Linux check).
#
# It installs nothing, clones nothing, writes nothing outside the checkout:
# everything goes to target/host-report/ (gitignored). Each step records its
# exit code, its run time and the last lines of its log; the full logs sit
# next to the report. A failing step does not stop the run -- later steps
# then usually fail too, which is part of the picture.
#
# Steps:
#   1  host facts       OS, CPU, clang/ld.lld/llvm-*/python3/make versions, locale
#   2  tool resolution  sdk/bin/newtos-cc --toolchain
#   3  core libraries   build-cflib.sh (libc, gemlib, cflib), build-libm.sh,
#                       build-compiler-rt.sh
#   4  host tests       sdk/run-host-tests.sh
#   5  two programs     newtos-cc: the quick start's hello.c, examples/mathtst.c (-lm)
#   6  QED, TeraDesk    only if sdk/apps/qed and sdk/apps/teradesk are checked
#                       out (docs/sdk/00-quickstart.md 5.2); else SKIP
#   7  checksums        sha256 of every produced program and resource file
#
# Usage:   sdk/host-report.sh
# Result:  target/host-report/report.txt  and  target/host-report.tar.gz
#          (the .tar.gz holds report.txt plus every step log)
# Guide:   docs/sdk/17-linux-test.md
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SDK/.." && pwd)"
OUT="$ROOT/target/host-report"
REPORT="$OUT/report.txt"
rm -rf "$OUT"
mkdir -p "$OUT/logs" "$OUT/programs"

if command -v sha256sum >/dev/null 2>&1; then SHA() { sha256sum "$@"; }
else SHA() { shasum -a 256 "$@"; }
fi

say() { printf '%s\n' "$*" | tee -a "$REPORT"; }

# fact <label> <command...> -- one line per fact, "(missing)" when absent
fact() {
  local label="$1"; shift
  local val
  if val="$("$@" 2>&1 | head -1)" && [ -n "$val" ]; then :; else val="(missing)"; fi
  printf '  %-14s %s\n' "$label" "$val" | tee -a "$REPORT"
}

# step <name> <command...> -- run, log, record rc + seconds + log tail
step() {
  local name="$1"; shift
  local log="$OUT/logs/$name.log"
  local t0 t1 rc
  t0=$(date +%s)
  ( cd "$ROOT" && "$@" ) >"$log" 2>&1
  rc=$?
  t1=$(date +%s)
  say ""
  say "--- $name: rc=$rc, $((t1 - t0)) s   (logs/$name.log)"
  tail -15 "$log" | sed 's/^/  | /' | tee -a "$REPORT"
  return $rc
}

say "NewTOS host report, $(date -u '+%Y-%m-%d %H:%M UTC')"
say "checkout: $(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null)$(git -C "$ROOT" diff --quiet 2>/dev/null || echo ' (modified)')"
say ""
say "=== 1 host facts"
fact "os"         uname -srm
[ -r /etc/os-release ] && fact "distribution" sh -c '. /etc/os-release; echo "$PRETTY_NAME"'
[ "$(uname -s)" = Darwin ] && fact "macos" sw_vers -productVersion
fact "clang"      clang --version
fact "clang path" sh -c 'command -v clang'
fact "aarch64"    sh -c 'clang -print-targets 2>/dev/null | grep -i aarch64 | head -1 | sed "s/^ *//"'
fact "ld.lld"     ld.lld --version
fact "llvm-ar"    sh -c 'llvm-ar --version | grep -i version'
fact "host cc"    cc --version
fact "make"       make --version
fact "patch"      patch --version
fact "python3"    python3 --version
fact "rustc"      rustc --version
fact "LANG"       sh -c 'echo "${LANG:-(unset)} / LC_ALL=${LC_ALL:-(unset)}"'
fact "NEWTOS_TOOLCHAIN" sh -c 'echo "${NEWTOS_TOOLCHAIN:-(unset, auto)}"'

say ""
say "=== 2 tool resolution"
"$SDK/bin/newtos-cc" --toolchain 2>&1 | sed 's/^/  /' | tee -a "$REPORT"

say ""
say "=== 3 core libraries"
step libs-cflib       sdk/cflib/build-cflib.sh
step libs-libm        sdk/libm/build-libm.sh
step libs-compiler-rt sdk/compiler-rt/build-compiler-rt.sh

say ""
say "=== 4 host tests"
step host-tests       sdk/run-host-tests.sh

say ""
say "=== 5 two programs"
# hello.c is the program of docs/sdk/00-quickstart.md 3b, verbatim
cat >"$OUT/programs/hello.c" <<'HELLO'
#include <stdio.h>
#include <mint/osbind.h>

int main(void)
{
    printf("Hello from NewTOS, GEMDOS %04x\n", (unsigned)(Sversion() & 0xffff));
    printf("Press a key.\n");
    Cconin();
    return 0;
}
HELLO
step hello   "$SDK/bin/newtos-cc" -Wall -o "$OUT/programs/HELLO.NCP"   "$OUT/programs/hello.c"
step mathtst "$SDK/bin/newtos-cc" -Wall -o "$OUT/programs/MATHTST.NCP" "$SDK/examples/mathtst.c" -lm
for f in "$OUT/programs/"*.NCP; do
  [ -f "$f" ] && command -v file >/dev/null 2>&1 && say "  $(basename "$f"): $(file -b "$f")"
done

say ""
say "=== 6 QED, TeraDesk"
if [ -d "$SDK/apps/qed/.git" ]; then
  say "  qed checkout:      $(git -C "$SDK/apps/qed" rev-parse --short HEAD)  (expected f12055f)"
  step qed sdk/apps/build-qed.sh
else
  say "  SKIP qed -- sdk/apps/qed not checked out (docs/sdk/00-quickstart.md 5.2)"
fi
if [ -d "$SDK/apps/teradesk/.git" ]; then
  say "  teradesk checkout: $(git -C "$SDK/apps/teradesk" rev-parse --short HEAD)  (expected b5532b8)"
  step teradesk sdk/apps/build-teradesk.sh
else
  say "  SKIP teradesk -- sdk/apps/teradesk not checked out (docs/sdk/00-quickstart.md 5.2)"
fi

say ""
say "=== 7 checksums (sha256)"
for f in "$OUT/programs/"*.NCP \
         "$SDK/apps/qed/build/QED.NAP" "$SDK/apps/qed/build/qed.rsc" \
         "$SDK/apps/teradesk/build/dist/DESKTOP.NAP" \
         "$SDK/apps/teradesk/build/dist/"*.rsc; do
  [ -f "$f" ] && SHA "$f" | sed "s|$ROOT/||; s/^/  /" | tee -a "$REPORT"
done

( cd "$OUT/.." && tar czf host-report.tar.gz host-report/report.txt host-report/logs )
say ""
say "report:  $REPORT"
say "archive: $ROOT/target/host-report.tar.gz"
