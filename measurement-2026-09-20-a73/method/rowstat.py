#!/usr/bin/env python3
"""Pixel statistics of the HELLO.TXT row and the info line in each screendump.
usage: rowstat.py PNG...   (640x480 dumps; row = x 175..470, y 333..347)"""
import sys
from PIL import Image
for p in sys.argv[1:]:
    im = Image.open(p).convert('RGB'); px = im.load()
    def blackfrac(x0,y0,x1,y1):
        n=b=0
        for y in range(y0,y1+1):
            for x in range(x0,x1+1):
                n+=1; r,g,bb = px[x,y]
                if r<64 and g<64 and bb<64: b+=1
        return b/n, b, n
    row = blackfrac(175,333,470,347)
    info = blackfrac(170,80,470,92)
    menu = blackfrac(66,20,240,52)     # File drop-down area (upper part, over desktop)
    ftitle = blackfrac(70,1,105,17)    # "File" title cell
    print("%-14s HELLO-row black=%.3f (%d/%d)  info-line black=%.3f  File-title black=%.3f  dropdown-area black=%.3f" %
          (p.split('/')[-1], row[0], row[1], row[2], info[0], ftitle[0], menu[0]))
