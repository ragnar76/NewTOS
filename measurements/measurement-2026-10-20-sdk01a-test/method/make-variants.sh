#!/usr/bin/env bash
# make-variants.sh (documentation of what was run by hand): NEW = APFS clone of the working-tree sdk/ after the Tester's own
# `MINTLIB_REFRESH=1 sdk/libc/build-libc.sh`; OLD = clone of the same sdk/ with every file that differs from HEAD restored via
# `git show HEAD:<file>` (git diff --name-only HEAD -- sdk) and every untracked sdk file removed, then MINTLIB_REFRESH=1 build-libc.sh.
# (git archive HEAD is not enough: sdk/apps/*/ vendored sources, mintlib, zview sources are gitignored.) tools/ copied, Applications symlinked
# (doom, mkrsc.py need them). Then build-apps.sh / build-apps2.sh <new|old> in the scratch dir; ZVLAUNCH via build-zvlaunch.sh (SDKDIR=...).
