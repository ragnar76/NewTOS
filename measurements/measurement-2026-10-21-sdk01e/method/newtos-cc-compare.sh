#!/usr/bin/env bash
# newtos-cc-compare.sh -- the same programs linked by HEAD's sdk/bin/newtos-cc (git show $HEADREV:...) and by the
# working-tree newtos-cc (+ libclang_rt.builtins.a): byte-identical when the program needs no builtin; the
# programs that need one link only with the new wrapper. Output -> ../checks/newtos-cc-compare.txt
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
W="$S/ncc"; rm -rf "$W"; mkdir -p "$W"; SDK="$WS/sdk"
git -C "$WS" show "$HEADREV:sdk/bin/newtos-cc" > "$W/newtos-cc-head"; chmod +x "$W/newtos-cc-head"
OUT="$HERE/../checks/newtos-cc-compare.txt"; : > "$OUT"
one() {  # one <name> <args...>
  local n=$1; shift
  NEWTOS_SDK="$SDK" "$W/newtos-cc-head" "$@" -o "$W/$n-head" > "$W/$n-head.log" 2>&1; local a=$?
  "$SDK/bin/newtos-cc" "$@" -o "$W/$n-new" > "$W/$n-new.log" 2>&1; local b=$?
  local v
  if [ $a -eq 0 ] && [ $b -eq 0 ]; then
    if cmp -s "$W/$n-head" "$W/$n-new"; then v="byte-identical $(shasum -a 256 < "$W/$n-new" | cut -c1-16)"; else v="DIFFERENT"; fi
  else
    v="head rc=$a ($(grep -o 'undefined symbol: [^ ]*' "$W/$n-head.log" | sort -u | tr '\n' ' ')) new rc=$b"
  fi
  printf '%-14s %s\n' "$n" "$v" | tee -a "$OUT"
}
cat > "$W/q.c" <<'C'
#include <stdlib.h>
#include <stdio.h>
int main(void) { int dp, s; char *p = qecvt(1.0L / 3, 10, &dp, &s); printf("%s %d %d\n", p, dp, s); return 0; }
C
cat > "$W/p.c" <<'C'
#include <math.h>
#include <stdio.h>
int main(void) { printf("%f %f %f\n", (double)powf(2.0f, 0.5f), remainder(10.0, 3.0), fdim(5.0, 3.0)); return 0; }
C
one gem-hello  -nap "$SDK/templates/gem-hello/hello.c"
one con-hello  -ncp "$SDK/templates/console-hello/hello.c"
one mathtst    -ncp -lm "$SDK/examples/mathtst.c"
one libtest    -ncp "$SDK/libc/tests/libtest.c"
one bare-hello -bare -ncp "$SDK/examples/hello.c"
one qecvt      -ncp "$W/q.c"
one powf       -ncp -lm "$W/p.c"
