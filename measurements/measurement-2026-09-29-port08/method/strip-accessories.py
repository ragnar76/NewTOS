#!/usr/bin/env python3
"""Remove the two desk accessories from a seeded card image (control arm).

usage: strip-accessories.py <image>
"""
import os, subprocess, sys

img = sys.argv[1]
dev = subprocess.check_output(
    ["hdiutil", "attach", "-imagekey", "diskimage-class=CRawDiskImage",
     "-nomount", img], text=True).split()[0]
try:
    subprocess.run(["diskutil", "mount", dev + "s1"], check=True, capture_output=True)
    V = "/Volumes/NEWTOS"
    for f in ("ACC/SYSACC.NAC", "SYSACC2.NAC"):
        p = os.path.join(V, f)
        if os.path.exists(p):
            os.remove(p)
            print("removed", f)
    subprocess.run(["sync"])
    subprocess.run(["diskutil", "unmount", V], check=True, capture_output=True)
finally:
    subprocess.run(["hdiutil", "detach", dev], capture_output=True)
