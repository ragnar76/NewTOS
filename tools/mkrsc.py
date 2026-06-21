#!/usr/bin/env python3
"""mkrsc — author a native NewTOS .RSC resource (see ADR-013).

The native format mirrors the kernel's repr(C) `Object`/`TedInfo`/… layout
(little-endian), with internal pointers stored as **byte offsets from the
blob base**; `rsrc_load` relocates them to absolute pointers on load.

This tool emits native resources directly (no classic-Atari conversion).
A separate mode (`convert`, planned) will read a classic big-endian
EmuTOS `.rsc` and re-emit it in this format.

Layout produced:
    NrscHeader(28B) | [u32 tree-index] | [Object;N] | string data

Object (32B, repr(C)):
    +0  i16 ob_next   +2  i16 ob_head   +4  i16 ob_tail
    +6  u16 ob_type   +8  u16 ob_flags  +10 u16 ob_state
    +16 u64 ob_spec   (pad at +12..16)
    +24 i16 ob_x  +26 i16 ob_y  +28 i16 ob_width  +30 i16 ob_height
"""
import struct
import sys

# Object types / flags / state (must match tos-api/src/aes/object.rs).
G_BOX, G_BUTTON, G_STRING = 20, 26, 28
SELECTABLE, EXIT, LASTOB = 0x0001, 0x0004, 0x0020

OBJ_SIZE = 32
HDR_SIZE = 28


def obj(next_, head, tail, typ, flags, state, spec, x, y, w, h):
    return struct.pack(
        "<hhhHHH4xQhhhh", next_, head, tail, typ, flags, state, spec, x, y, w, h
    )


def build_demo():
    """A simple dialog: framed box + title string + OK button."""
    strings = b""
    # Place strings after header + tree-index + objects; patched below.
    titles = []

    def addstr(s):
        nonlocal strings
        off = len(strings)
        strings += s.encode() + b"\x00"
        return off  # relative to start of the string section

    title_rel = addstr("Loaded from SD via rsrc_load")
    ok_rel = addstr("OK")

    n_objects = 3
    n_trees = 1
    off_trindex = HDR_SIZE
    off_objects = off_trindex + n_trees * 4
    off_strings = off_objects + n_objects * OBJ_SIZE

    # ob_spec for strings = absolute blob offset (off_strings + relative).
    s_title = off_strings + title_rel
    s_ok = off_strings + ok_rel

    objects = b"".join([
        obj(-1, 1, 2, G_BOX, 0, 0, 1 << 16, 170, 150, 300, 130),
        obj(2, -1, -1, G_STRING, 0, 0, s_title, 24, 28, 250, 16),
        obj(0, -1, -1, G_BUTTON, SELECTABLE | EXIT | LASTOB, 0, s_ok, 100, 90, 100, 28),
    ])

    trindex = struct.pack("<I", 0)  # tree 0 root = object 0
    total = off_strings + len(strings)
    header = struct.pack(
        "<4sIIIIII", b"NRSC", 1, n_trees, n_objects, off_trindex, off_objects, total
    )
    return header + trindex + objects + strings


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "TEST.RSC"
    blob = build_demo()
    with open(out, "wb") as f:
        f.write(blob)
    print(f"wrote {out} ({len(blob)} bytes)")


if __name__ == "__main__":
    main()
