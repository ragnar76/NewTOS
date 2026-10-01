#!/usr/bin/env python3
"""Faithful re-implementation of plat-newtos.c's r_str()/r_dec() with rbuf[200].

Shows that write_report()'s text does not fit the demo's own 200-byte buffer:
at any resolution it is cut at rlen == 199, and at 1920x1080 the cut falls
inside the word "call" -- byte-for-byte what C:\\BALL.LOG shows on the Pi.
"""
SIZE = 200                      # static char rbuf[200];
buf = bytearray(SIZE)
rlen = 0


def r_str(s):                   # while (*s && rlen < (int)sizeof(rbuf) - 1)
    global rlen
    for ch in s:
        if rlen >= SIZE - 1:
            break
        buf[rlen] = ord(ch)
        rlen += 1


def r_dec(v):
    r_str(str(v))


for (w, h) in ((1920, 1080), (640, 480)):
    rlen = 0
    r_str("BALL: NewTOS screen "); r_dec(w); r_str("x"); r_dec(h); r_str("x"); r_dec(16)
    r_str(", memory form "); r_str("VERIFIED")
    r_str(" (fd_wdwidth = pixels/16, one device word per pixel), colour table ")
    r_str("MEASURED through vs_color")
    r_str(", one page over vro_cpyfm -- no XBIOS screen-base call")
    r_str("\r\n")
    out = bytes(buf[:rlen])
    print("%dx%d -> Fwrite length %d, tail %r" % (w, h, rlen, out[-32:]))
