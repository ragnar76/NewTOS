#!/usr/bin/env bash
# errors.sh -- newtos-cc error paths, run with dash (Linux /bin/sh stand-in), a fake SDK that has only
# libc (scratch copy: newtos.ld, crt0.S, include/, libc -> symlinks), PRGFLAGS values read back with
# `cargo xtask prgflags`, a build outside the repository, and the -l / -nac / -ntp smoke links.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
F="$S/fakesdk"; rm -rf "$F"; mkdir -p "$F/libc" "$F/bin" "$S/e"
cp "$WS/sdk/newtos.ld" "$WS/sdk/crt0.S" "$F/"; ln -s "$WS/sdk/include" "$F/include"
for x in mintlib crt0.S libc.a; do ln -s "$WS/sdk/libc/$x" "$F/libc/$x"; done
cp "$WS/sdk/bin/newtos-cc" "$F/bin/"
cd "$WS/sdk/templates/gem-hello"; make -s >/dev/null
N="dash $WS/sdk/bin/newtos-cc"
t() { echo "\$ $1"; eval "$1" 2>&1 | sed "s|$S|\$S|g; s|$WS|\$WS|g"; echo "  -> rc=${PIPESTATUS[0]}"; }
t "dash $F/bin/newtos-cc -nap hello.c -o $S/e/X.NAP"
t "dash $F/bin/newtos-cc -nap build/hello.o -o $S/e/X.NAP"
t "$N -lfoo hello.c -o $S/e/X.NCP"
t "$N -bare -lgem hello.c"
t "$N -nap -ncp hello.c"
t "$N -prot foo hello.c"
t "CLANG=noclang $N hello.c"
t "$N -nap hello.c -o $S/e/X.NCP"
t "$N hello.c -o $S/e/HELLOK.NAP && cmp $S/e/HELLOK.NAP HELLO.NAP && echo 'kind from -o: identical to make build'"
for o in "-prot global" "-prgflags 0x3 -prot readable" "-prgflags 2" "-prot super"; do
  t "$N -ncp $o $WS/sdk/templates/console-hello/hello.c -o $S/e/PF.NCP && (cd $WS && cargo xtask prgflags $S/e/PF.NCP 2>/dev/null | tail -1)"
done
mkdir -p "$S/e/outside"; cp hello.c Makefile "$S/e/outside/"
t "cd $S/e/outside && make -s NEWTOS_SDK=$WS/sdk && cmp HELLO.NAP $WS/sdk/templates/gem-hello/HELLO.NAP && echo 'outside the repo (rustup default: '\$(rustup show active-toolchain)'): identical'"
printf '#include <cflib.h>\n#include <math.h>\n#include <zlib.h>\n#include <png.h>\nint main(void){init_app(NULL);do_alert(1,0,"[1][x][OK]");exit_gem();return (int)sqrt(4.0)+(int)zlibVersion()[0]+(int)png_access_version_number();}\n' > "$S/e/cf.c"
t "$N -nap -lcflib -lm -lpng $S/e/cf.c -o $S/e/CF.NAP && echo link ok"
printf '#include <windom.h>\n#include <stdio.h>\n#include <jpeglib.h>\n#include <gif_lib.h>\n#include <libexif/exif-data.h>\n#include <libshare.h>\nint f(void){return 0;}\n' > "$S/e/inc.c"
t "$N -nap -lwindom -ljpeg -lgif -lexif -lshare -c $S/e/inc.c -o $S/e/inc.o && echo compile ok"
t "$N -nac hello.c -o $S/e/ACC.NAC && echo link ok"
t "$N -ntp $WS/sdk/templates/console-hello/hello.c -o $S/e/H.NTP && echo link ok"
