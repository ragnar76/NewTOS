#!/usr/bin/env python3
"""DESK16 acceptance: compare the pixels on screen with `desk/icon.rsc`.

Reads the rasters out of `references/emutos/desk/icons.c` (the C the EmuTOS
build generates from `icon.rsc` with `ird`) and checks, pixel by pixel, that
what the screendump shows really is that `ICONBLK` -- rather than trusting the
eye.  `ICONBLK[5]` (IG_REMOV) is `rs_icondata5` over `rs_iconmask0`: the
resource gives the removable disk **no mask of its own**, it shares the hard
disk's.

Blitting rule (`gr_gicon`): where the mask bit is 0 the background shows
through, where it is 1 the pixel is black if the data bit is set and white if
it is not.  Only mask-set pixels are compared.

usage:
  pixcheck.py preview PNG rs_icondataN     # the ADINSDSK icon preview
  pixcheck.py deskicon PNG rs_icondataN    # the first desktop icon (C:)

Run from the repository root.
"""
import re
import sys

from PIL import Image

SRC = open('references/emutos/desk/icons.c').read()


def arr(name):
    m = re.search(r'static const WORD %s\[\] = \{(.*?)\};' % name, SRC, re.S)
    return [int(x, 16) for x in re.findall(r'0x([0-9A-Fa-f]{4})', m.group(1))]


def bit(a, row, col):
    return (a[row * 2 + (col >> 4)] >> (15 - (col & 15))) & 1


# Where the 32x32 bitmap sits on the 640x480 screen.
#
# preview: ADINSDSK ROOT (156,91) + ID_IBOX (gx=72, y_ibox=184) + ID_ICON
#   (GAP=8, (IBOX_H-g_hicon)/2=10) + ib_xicon = (g_wicon-32)/2 = 22.
#   -> (156+72+8+22, 91+184+10) = (258,285).  Every term is printed by the
#   run's own `[probe] ADINSDSK` lines.
# deskicon: DROOT child 9 box (0,19) + ib_xicon 22, ib_yicon 0 -> (22,19),
#   printed by the run's own `[probe] desk obj 9` line.
ORIGIN = {'preview': (258, 285), 'deskicon': (22, 19)}

# The drive-letter overlay cell (`ib_xchar`,`ib_ychar`) = (13,16), 6x6 in the
# small AES font: `gr_gicon` draws the letter over the bitmap, so those pixels
# are not the resource's and are skipped.  The ADINSDSK preview carries no
# letter (`insert_icon` blanks it), the desktop icon does.
LETTER_CELL = (13, 19, 16, 22)


def main():
    kind, png, data_name = sys.argv[1], sys.argv[2], sys.argv[3]
    mask = arr('rs_iconmask0' if data_name in ('rs_icondata0', 'rs_icondata5') else
               'rs_iconmask' + data_name[len('rs_icondata'):])
    data = arr(data_name)
    im = Image.open(png).convert('RGB')
    x0, y0 = ORIGIN[kind]
    bad = tot = 0
    for r in range(32):
        for c in range(32):
            if kind == 'deskicon' and LETTER_CELL[0] <= c < LETTER_CELL[1] \
               and LETTER_CELL[2] <= r < LETTER_CELL[3]:
                continue
            if bit(mask, r, c) == 0:
                continue
            tot += 1
            want = (0, 0, 0) if bit(data, r, c) else (255, 255, 255)
            if im.getpixel((x0 + c, y0 + r)) != want:
                bad += 1
    print('%s %s vs %s: %d mask pixels compared, %d mismatching'
          % (kind, png.split('/')[-1], data_name, tot, bad))
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
