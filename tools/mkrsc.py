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


# ── classic Atari .rsc → native conversion (ADR-013) ──────────────────────
#
# Classic structures are big-endian m68k with 16-bit offsets and char-grid
# coordinates; we byte-swap, expand pointers to 64-bit offsets, and apply
# obfix (char→pixel). Native struct sizes (repr(C)):
#   Object 32, TedInfo 40, BitBlk 24, IconBlk 48.

# AES system-font cell size used by obfix (gl_wchar/gl_hchar).
WCHAR, HCHAR, SCREEN_W = 8, 16, 640

# Object types that don't carry a relocatable ob_spec pointer.
BOX_TYPES = {20, 25, 27}      # G_BOX, G_IBOX, G_BOXCHAR (literal word)
STR_TYPES = {26, 28, 32}      # G_BUTTON, G_STRING, G_TITLE
TED_TYPES = {21, 22, 29, 30}  # G_TEXT, G_BOXTEXT, G_FTEXT, G_FBOXTEXT
IMG_TYPE, ICON_TYPE = 23, 31  # G_IMAGE (BITBLK), G_ICON (ICONBLK)

CLS_OBJ, CLS_TED, CLS_IB, CLS_BB = 24, 28, 34, 14  # classic struct sizes
NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 32, 40, 48, 24  # native struct sizes


def obfix(word, axis):
    """Classic char-grid coordinate → pixel (EmuTOS fix_chpos)."""
    word &= 0xFFFF
    coffset = (word >> 8) & 0xFF
    cpos = word & 0xFF
    if axis == 0:
        cpos *= WCHAR
    elif axis == 1:
        cpos *= HCHAR
    elif axis == 2:
        cpos = SCREEN_W if cpos == 80 else cpos * WCHAR
    else:
        cpos *= HCHAR
    cpos += (coffset - 256) if coffset > 128 else coffset
    return cpos


def convert(path, out, ilp32=False):
    # Native struct sizes + pack formats by target ABI (ADR-020 stage 4).
    # ob_spec / te_p* / ib_p* / bi_pdata are POINTERS: 8 bytes on LP64, 4 on
    # ILP32. An ILP32 app (QED, GNU -mabi=ilp32) links a 24-byte OBJECT with a
    # 4-byte ob_spec @ 12 and correspondingly smaller TEDINFO/ICONBLK/BITBLK
    # (verified by compile-time sizeof against gemlib mt_gem.h: OBJECT=24,
    # TEDINFO=28, ICONBLK=36, BITBLK=16). The relocated pointer values still fit
    # a 4-byte field because the per-process RSC window sits in low VA. The
    # NrscHeader/tree-index (u32 byte offsets) are ABI-independent; `version`
    # marks the OBJECT width (1 = LP64/64-bit ob_spec, 2 = ILP32/32-bit ob_spec)
    # for tooling, but the kernel relocates by the *caller's* ABI.
    if ilp32:
        NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 24, 28, 36, 16
        OBJ_FMT = "<hhhHHHIhhhh"          # 4-byte ob_spec @ 12
        TED_FMT = "<IIIhhhhhhhh"          # 4-byte te_ptext/te_ptmplt/te_pvalid
        BB_FMT = "<Ihhhhh2x"              # 4-byte bi_pdata, pad to 16
        IB_FMT = "<IIIhhhhhhhhhhh2x"      # 4-byte ib_pmask/ib_pdata/ib_ptext, 36
        version = 2
    else:
        NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 32, 40, 48, 24
        OBJ_FMT = "<hhhHHH4xQhhhh"        # 8-byte ob_spec @ 16
        TED_FMT = "<QQQhhhhhhhh"
        BB_FMT = "<Qhhhhh6x"
        IB_FMT = "<QQQhhhhhhhhhhh2x"
        version = 1

    d = open(path, "rb").read()
    H = struct.unpack(">18H", d[:36])
    (vrsn, p_obj, p_ted, p_ib, p_bb, p_frstr, p_str, p_imd, p_frimg,
     p_tri, nobs, ntree, nted, nib, nbb, nstr, nimg, rssize) = H
    if vrsn & 0x0004:
        sys.exit("new-format (CICONBLK) .rsc not supported")

    # Native section base offsets (counts are fixed up front).
    off_tri = HDR_SIZE
    off_obj = off_tri + ntree * 4
    off_ted = off_obj + nobs * NAT_OBJ
    off_bb = off_ted + nted * NAT_TED
    off_ib = off_bb + nbb * NAT_BB
    off_data = off_ib + nib * NAT_IB

    data = bytearray()
    interned = {}

    def intern_str(coff):
        if coff == 0:
            return 0
        if coff in interned:
            return interned[coff]
        end = d.index(b"\x00", coff)
        nat = off_data + len(data)
        data.extend(d[coff:end + 1])
        interned[coff] = nat
        return nat

    def intern_bytes(coff, length):
        if coff == 0 or length <= 0:
            return 0
        key = (coff, length)
        if key in interned:
            return interned[key]
        nat = off_data + len(data)
        data.extend(d[coff:coff + length])
        interned[key] = nat
        return nat

    # Native struct offset of classic struct #k in a fixed array.
    def nat_of(coff, base, csize, nbase, nsize):
        return nbase + ((coff - base) // csize) * nsize

    # TEDINFOs (remap text/template/valid string pointers).
    teds = bytearray()
    for i in range(nted):
        o = p_ted + i * CLS_TED
        (ptext, ptmpl, pvalid, font, j1, just, color, j2, thick, txtlen,
         tmplen) = struct.unpack(">IIIhhhhhhhh", d[o:o + CLS_TED])
        teds.extend(struct.pack(
            TED_FMT, intern_str(ptext), intern_str(ptmpl),
            intern_str(pvalid), font, j1, just, color, j2, thick, txtlen, tmplen))

    # BITBLKs (remap raster data).
    bbs = bytearray()
    for i in range(nbb):
        o = p_bb + i * CLS_BB
        pdata, wb, hl, x, y, col = struct.unpack(">Ihhhhh", d[o:o + CLS_BB])
        bbs.extend(struct.pack(
            BB_FMT, intern_bytes(pdata, wb * hl), wb, hl, x, y, col))

    # ICONBLKs (remap mask/data/text).
    ibs = bytearray()
    for i in range(nib):
        o = p_ib + i * CLS_IB
        f = struct.unpack(">IIIhhhhhhhhhhh", d[o:o + CLS_IB])
        pmask, pdata, ptext = f[0], f[1], f[2]
        wicon, hicon = f[8], f[9]
        rb = ((wicon + 15) // 16) * 2 * hicon
        ibs.extend(struct.pack(
            IB_FMT, intern_bytes(pmask, rb), intern_bytes(pdata, rb),
            intern_str(ptext), *f[3:]))

    # OBJECTs (obfix coords + remap ob_spec by type).
    objs = bytearray()
    for i in range(nobs):
        o = p_obj + i * CLS_OBJ
        nxt, head, tail, typ, flags, state, spec, x, y, w, h = struct.unpack(
            ">hhhHHHIhhhh", d[o:o + CLS_OBJ])
        base_type = typ & 0xFF  # high byte = extended flags
        if base_type in BOX_TYPES:
            nspec = spec & 0xFFFFFFFF
        elif base_type in STR_TYPES:
            nspec = intern_str(spec)
        elif base_type in TED_TYPES:
            nspec = nat_of(spec, p_ted, CLS_TED, off_ted, NAT_TED)
        elif base_type == IMG_TYPE:
            nspec = nat_of(spec, p_bb, CLS_BB, off_bb, NAT_BB)
        elif base_type == ICON_TYPE:
            nspec = nat_of(spec, p_ib, CLS_IB, off_ib, NAT_IB)
        else:
            nspec = 0  # G_PROGDEF / G_CICON: unsupported, drawn empty
        objs.extend(struct.pack(
            OBJ_FMT, nxt, head, tail, typ, flags, state, nspec,
            obfix(x, 0), obfix(y, 1), obfix(w, 2), obfix(h, 3)))

    # Tree index → native root OBJECT indices.
    tri = bytearray()
    for t in range(ntree):
        toff = struct.unpack(">I", d[p_tri + t * 4:p_tri + t * 4 + 4])[0]
        tri.extend(struct.pack("<I", (toff - p_obj) // CLS_OBJ))

    total = off_data + len(data)
    header = struct.pack(
        "<4sIIIIII", b"NRSC", version, ntree, nobs, off_tri, off_obj, total)
    blob = header + tri + objs + teds + bbs + ibs + data
    assert len(blob) == total, (len(blob), total)
    open(out, "wb").write(blob)
    print(f"converted {path} -> {out}: {ntree} trees, {nobs} objects, "
          f"{nted} tedinfo, {nbb} bitblk, {nib} iconblk ({total} bytes) "
          f"[{'ILP32/24B-OBJECT' if ilp32 else 'LP64/32B-OBJECT'}]")


def main():
    if len(sys.argv) >= 2 and sys.argv[1] == "convert":
        # convert [--ilp32] SRC [OUT]: --ilp32 emits the 24-byte OBJECT / 4-byte
        # ob_spec layout for GNU-ILP32 apps (QED), else the LP64 32-byte layout.
        rest = sys.argv[2:]
        ilp32 = False
        if rest and rest[0] == "--ilp32":
            ilp32 = True
            rest = rest[1:]
        src = rest[0]
        out = rest[1] if len(rest) > 1 else "OUT.RSC"
        convert(src, out, ilp32=ilp32)
        return
    out = sys.argv[1] if len(sys.argv) > 1 else "TEST.RSC"
    blob = build_demo()
    with open(out, "wb") as f:
        f.write(blob)
    print(f"wrote {out} ({len(blob)} bytes)")


if __name__ == "__main__":
    main()
