#!/usr/bin/env python3
"""instrument-doom.py <src-dir> <dst-dir>

Copies sdk/apps/doom/{doomgeneric_gem.c,doomgeneric_gem_kbdvec.[ch]} into a
scratch directory and adds K02-TEMP console lines to the copy of
doomgeneric_gem.c (never to the shared tree):
  K02 byte XX at T ms      every byte DG_GetKey takes out of the kbdvec ring
  K02 put PRESS|RELEASE key N at T ms   every event handed to DOOM
T = DG_GetTicksMs() = _hz_200 * 5, so T/5 is directly comparable with the
kernel injector's hz_200 stamps.
"""
import sys, pathlib, shutil
src, dst = map(pathlib.Path, sys.argv[1:3])
dst.mkdir(parents=True, exist_ok=True)
for n in ("doomgeneric_gem_kbdvec.c", "doomgeneric_gem_kbdvec.h"):
    shutil.copy(src / n, dst / n)
s = (src / "doomgeneric_gem.c").read_text()
old = """static void keyq_put(int pressed, unsigned char key)
{
"""
assert s.count(old) == 1
s = s.replace(old, old + """	printf("K02 put %s key %d at %lu ms\\n", pressed ? "PRESS" : "RELEASE", key, (unsigned long)DG_GetTicksMs()); /* K02-TEMP */
""")
old = """	int scan = b & 0x7f, make = !(b & 0x80), was, now;
	unsigned char k;
"""
assert s.count(old) == 1
s = s.replace(old, old + """
	printf("K02 byte %02x at %lu ms\\n", b, (unsigned long)DG_GetTicksMs()); /* K02-TEMP */
""")
(dst / "doomgeneric_gem.c").write_text(s)
print("instrumented copy in", dst)
