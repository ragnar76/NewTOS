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


def swap16_raster(b):
    """Byte-swap each 16-bit word of a raster copied from a classic `.rsc`.

    Classic Atari icon/image rasters (BITBLK `bi_pdata`, ICONBLK
    `ib_pmask`/`ib_pdata`) are **big-endian** 16-bit words with the MSB as the
    leftmost pixel. Our native (NRSC / native-RSHDR) format is little-endian by
    contract, and the kernel blit reads each raster word as a LITTLE-endian
    `u16` (`kernel/src/vdi_backend.rs` `blit_mono`: `w & (0x8000 >> (sx & 15))`),
    matching the built-in host-order `[u16]` set in `kernel/src/desk/icons.rs`.
    Copying the classic bytes verbatim would swap the two 8-pixel halves of every
    16-pixel word (scrambled icons), so swap the two bytes of each word here.

    Both raster lengths are word-aligned (BITBLK `wb*hl`, ICONBLK
    `((wicon+15)//16)*2*hicon`), so an odd trailing byte is not expected; if one
    ever appears, keep it verbatim (a swap needs a byte pair) and warn rather
    than corrupt or crash.
    """
    n = len(b) & ~1
    out = bytearray(len(b))
    out[0:n:2] = b[1:n:2]   # native low byte  <- classic high byte
    out[1:n:2] = b[0:n:2]   # native high byte <- classic low byte
    if len(b) & 1:
        out[-1] = b[-1]
        print(f"warning: odd-length raster ({len(b)} bytes); trailing byte "
              f"left unswapped (not word-aligned)", file=sys.stderr)
    return bytes(out)


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
    # PTR = free-string/free-image pointer-slot width (matches ob_spec).
    # version 5 = LP64/68-byte header, 6 = ILP32/68-byte header: both carry the
    # frstr/frimg pointer tables (as v3/v4 did) AND the TEDINFO/ICONBLK/BITBLK
    # array bases + counts, so rsrc_gaddr/rsrc_saddr resolve the R_TEDINFO/
    # R_ICONBLK/R_BITBLK family + their sub-pointer aliases (AUD-P6-002). The
    # kernel loader stays backward-compatible with the earlier headers: v1/v2
    # (28-byte, no tables), v3/v4 (44-byte, +frstr/frimg tables only).
    if ilp32:
        NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 24, 28, 36, 16
        OBJ_FMT = "<hhhHHHIhhhh"          # 4-byte ob_spec @ 12
        TED_FMT = "<IIIhhhhhhhh"          # 4-byte te_ptext/te_ptmplt/te_pvalid
        BB_FMT = "<Ihhhhh2x"              # 4-byte bi_pdata, pad to 16
        IB_FMT = "<IIIhhhhhhhhhhh2x"      # 4-byte ib_pmask/ib_pdata/ib_ptext, 36
        PTR, PTR_FMT, version = 4, "<I", 6
    else:
        NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 32, 40, 48, 24
        OBJ_FMT = "<hhhHHH4xQhhhh"        # 8-byte ob_spec @ 16
        TED_FMT = "<QQQhhhhhhhh"
        BB_FMT = "<Qhhhhh6x"
        IB_FMT = "<QQQhhhhhhhhhhh2x"
        PTR, PTR_FMT, version = 8, "<Q", 5

    d = open(path, "rb").read()
    H = struct.unpack(">18H", d[:36])
    (vrsn, p_obj, p_ted, p_ib, p_bb, p_frstr, p_str, p_imd, p_frimg,
     p_tri, nobs, ntree, nted, nib, nbb, nstr, nimg, rssize) = H
    if vrsn & 0x0004:
        sys.exit("new-format (CICONBLK) .rsc not supported")

    # Native section base offsets (counts are fixed up front). The v5/v6 header
    # is 68 bytes: the v3/v4 fields (through off_frimg/nimg) plus the TEDINFO/
    # ICONBLK/BITBLK array bases + counts (off_ted/nted, off_ib/nib, off_bb/nbb).
    # A free-string then a free-image pointer table sits between the ICONBLKs and
    # the string/image data (each table = count * PTR bytes).
    HDR = 68
    off_tri = HDR
    off_obj = off_tri + ntree * 4
    off_ted = off_obj + nobs * NAT_OBJ
    off_bb = off_ted + nted * NAT_TED
    off_ib = off_bb + nbb * NAT_BB
    off_frstr = off_ib + nib * NAT_IB
    off_frimg = off_frstr + nstr * PTR
    off_data = off_frimg + nimg * PTR

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
        # Raster bytes only (intern_bytes is called for BITBLK bi_pdata and
        # ICONBLK ib_pmask/ib_pdata exclusively; strings go through intern_str).
        # Byte-swap each 16-bit word big-endian classic -> little-endian native.
        data.extend(swap16_raster(d[coff:coff + length]))
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

    # Free-string pointer table: nstr slots, each a native offset to the
    # interned alert/string data (classic p_frstr holds LONG offsets into the
    # string data; EmuTOS relocates them via fix_nptrs/R_FRSTR). rsrc_gaddr
    # R_FRSTR returns &frstr[i]; R_STRING returns frstr[i].
    frstr = bytearray()
    for i in range(nstr):
        coff = struct.unpack(">I", d[p_frstr + i * 4:p_frstr + i * 4 + 4])[0]
        frstr.extend(struct.pack(PTR_FMT, intern_str(coff)))

    # Free-image pointer table: nimg slots, each a native offset to a BITBLK.
    # In a classic .rsc a free image *is* a BITBLK (verified against gem.rsc:
    # frimg[i] points into the bb array), relocated as a raw pointer. We remap
    # each into the native bb array; anything outside it is unsupported (rather
    # than fabricate a location). QED has nimg == 0, so this loop is empty there.
    frimg = bytearray()
    for i in range(nimg):
        coff = struct.unpack(">I", d[p_frimg + i * 4:p_frimg + i * 4 + 4])[0]
        if coff == 0:
            nat = 0
        elif p_bb <= coff < p_bb + nbb * CLS_BB and (coff - p_bb) % CLS_BB == 0:
            nat = nat_of(coff, p_bb, CLS_BB, off_bb, NAT_BB)
        else:
            sys.exit(f"free image {i} at 0x{coff:x} is not a BITBLK in the "
                     f"bb array [{p_bb:#x},{p_bb + nbb * CLS_BB:#x}); unsupported")
        frimg.extend(struct.pack(PTR_FMT, nat))

    # Tree index → native root OBJECT indices.
    tri = bytearray()
    for t in range(ntree):
        toff = struct.unpack(">I", d[p_tri + t * 4:p_tri + t * 4 + 4])[0]
        tri.extend(struct.pack("<I", (toff - p_obj) // CLS_OBJ))

    total = off_data + len(data)
    # v5/v6 header: magic + 16 u32 (68 bytes). Field order MUST match the kernel
    # `NrscHeader` struct (tos-api/src/aes/resource.rs): the v3/v4 fields, then
    # off_ted/nted, off_ib/nib, off_bb/nbb (the TEDINFO/ICONBLK/BITBLK bases).
    header = struct.pack(
        "<4s16I", b"NRSC", version, ntree, nobs, off_tri, off_obj, total,
        off_frstr, nstr, off_frimg, nimg,
        off_ted, nted, off_ib, nib, off_bb, nbb)
    blob = header + tri + objs + teds + bbs + ibs + bytes(frstr) + bytes(frimg) + data
    assert len(blob) == total, (len(blob), total)
    open(out, "wb").write(blob)
    print(f"converted {path} -> {out}: {ntree} trees, {nobs} objects, "
          f"{nted} tedinfo, {nbb} bitblk, {nib} iconblk, {nstr} frstr, "
          f"{nimg} frimg ({total} bytes) "
          f"[{'ILP32/24B-OBJECT' if ilp32 else 'LP64/32B-OBJECT'}]")


def convert_rshdr(path, out):
    """Classic Atari .rsc → native-RSHDR file for an app's OWN RSC loader.

    Target consumer: tho-otto's rsc_load.c (`xrsrc_load`, bundled by TeraDesk),
    which maps the on-disk bytes directly onto the app's native C structs and
    then relocates offsets → pointers in `xrsrc_fix`. On m68k that on-disk
    format IS the classic .rsc; on NewTOS LP64/LE the same parser expects:

      RSHDR   18 × u16 LITTLE-endian, classic field order, rsh_vrsn = 0
              (no XRSC, no CICON extension); all section offsets are u16
              byte offsets from file start (hard <64K limit — enforced).
      OBJECT  32-byte LP64 gemlib layout; ob_x/y/w/h stay RAW char-grid
              packed words (the parser runs its own rsc_obfix at load —
              pre-converting here would double-convert).
      ob_spec non-box types hold the BYTE OFFSET of their target struct /
              string (xrsrc_fix does `buf + offset` generically for the
              G_STRING family, G_ICON, G_IMAGE and the TED types alike).
      TEDINFO/ICONBLK/BITBLK  40/48/24-byte LP64 layouts, pointer fields
              hold byte offsets (fixed by `+= buf` / `rsc_pointer`).
      trindex/frstr/frimg  u64 slots (they are pointer arrays on LP64),
              each holding a byte offset.
      data    icon/image bitmaps + strings raw, exactly as the NRSC
              converter emits them (same kernel objc_draw renderer).

    Unsupported inputs (CICON extension, G_PROGDEF/G_CICON objects) abort
    loudly rather than emit something the parser would mis-relocate.
    """
    d = open(path, "rb").read()
    H = struct.unpack(">18H", d[:36])
    (vrsn, p_obj, p_ted, p_ib, p_bb, p_frstr, p_str, p_imd, p_frimg,
     p_tri, nobs, ntree, nted, nib, nbb, nstr, nimg, rssize) = H
    if vrsn & 0x0004:
        sys.exit("new-format (CICONBLK) .rsc not supported")

    OBJ_FMT = "<hhhHHH4xQhhhh"        # 8-byte ob_spec @ 16
    TED_FMT = "<QQQhhhhhhhh"
    BB_FMT = "<Qhhhhh6x"
    IB_FMT = "<QQQhhhhhhhhhhh2x"

    # Section layout: header padded to 40 so every u64-bearing struct/table
    # start is 8-aligned (all native struct sizes are multiples of 8).
    HDR = 40
    off_tri = HDR
    off_obj = off_tri + ntree * 8     # trindex is an OBJECT*[] → u64 slots
    off_ted = off_obj + nobs * NAT_OBJ
    off_bb = off_ted + nted * NAT_TED
    off_ib = off_bb + nbb * NAT_BB
    off_frstr = off_ib + nib * NAT_IB
    off_frimg = off_frstr + nstr * 8  # char*[] → u64 slots
    off_data = off_frimg + nimg * 8   # BITBLK*[] → u64 slots

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
        # Raster bytes only (intern_bytes is called for BITBLK bi_pdata and
        # ICONBLK ib_pmask/ib_pdata exclusively; strings go through intern_str).
        # Byte-swap each 16-bit word big-endian classic -> little-endian native.
        data.extend(swap16_raster(d[coff:coff + length]))
        interned[key] = nat
        return nat

    def nat_of(coff, base, csize, nbase, nsize):
        return nbase + ((coff - base) // csize) * nsize

    teds = bytearray()
    for i in range(nted):
        o = p_ted + i * CLS_TED
        (ptext, ptmpl, pvalid, font, j1, just, color, j2, thick, txtlen,
         tmplen) = struct.unpack(">IIIhhhhhhhh", d[o:o + CLS_TED])
        teds.extend(struct.pack(
            TED_FMT, intern_str(ptext), intern_str(ptmpl),
            intern_str(pvalid), font, j1, just, color, j2, thick, txtlen, tmplen))

    bbs = bytearray()
    for i in range(nbb):
        o = p_bb + i * CLS_BB
        pdata, wb, hl, x, y, col = struct.unpack(">Ihhhhh", d[o:o + CLS_BB])
        bbs.extend(struct.pack(
            BB_FMT, intern_bytes(pdata, wb * hl), wb, hl, x, y, col))

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

    objs = bytearray()
    for i in range(nobs):
        o = p_obj + i * CLS_OBJ
        nxt, head, tail, typ, flags, state, spec, x, y, w, h = struct.unpack(
            ">hhhHHHIhhhh", d[o:o + CLS_OBJ])
        base_type = typ & 0xFF
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
            sys.exit(f"object {i}: type {base_type} has no native-RSHDR "
                     f"relocation (G_PROGDEF/G_CICON unsupported)")
        # Coordinates stay RAW packed words — the consumer runs rsc_obfix.
        objs.extend(struct.pack(
            OBJ_FMT, nxt, head, tail, typ, flags, state, nspec, x, y, w, h))

    frstr = bytearray()
    for i in range(nstr):
        coff = struct.unpack(">I", d[p_frstr + i * 4:p_frstr + i * 4 + 4])[0]
        frstr.extend(struct.pack("<Q", intern_str(coff)))

    frimg = bytearray()
    for i in range(nimg):
        coff = struct.unpack(">I", d[p_frimg + i * 4:p_frimg + i * 4 + 4])[0]
        if coff == 0:
            nat = 0
        elif p_bb <= coff < p_bb + nbb * CLS_BB and (coff - p_bb) % CLS_BB == 0:
            nat = nat_of(coff, p_bb, CLS_BB, off_bb, NAT_BB)
        else:
            sys.exit(f"free image {i} at 0x{coff:x} is not a BITBLK; unsupported")
        frimg.extend(struct.pack("<Q", nat))

    # Tree index: u64 byte offsets of each tree's root OBJECT.
    tri = bytearray()
    for t in range(ntree):
        toff = struct.unpack(">I", d[p_tri + t * 4:p_tri + t * 4 + 4])[0]
        tri.extend(struct.pack("<Q", off_obj + ((toff - p_obj) // CLS_OBJ) * NAT_OBJ))

    total = off_data + len(data)
    # Every RSHDR field is u16 — the whole native file must stay below 64K.
    hdr_fields = (0, off_obj, off_ted, off_ib, off_bb, off_frstr, off_data,
                  off_data, off_frimg, off_tri, nobs, ntree, nted, nib, nbb,
                  nstr, nimg, total)
    for name, v in zip(("vrsn", "object", "tedinfo", "iconblk", "bitblk",
                        "frstr", "string", "imdata", "frimg", "trindex",
                        "nobs", "ntree", "nted", "nib", "nbb", "nstring",
                        "nimages", "rssize"), hdr_fields):
        if v > 0xFFFF:
            sys.exit(f"native-RSHDR field rsh_{name}={v} exceeds the u16 "
                     f"classic-header limit (file too large for this format)")
    header = struct.pack("<18H", *hdr_fields) + b"\x00" * (HDR - 36)
    blob = header + tri + objs + teds + bbs + ibs + bytes(frstr) + bytes(frimg) + data
    assert len(blob) == total, (len(blob), total)
    open(out, "wb").write(blob)
    print(f"converted {path} -> {out}: {ntree} trees, {nobs} objects, "
          f"{nted} tedinfo, {nbb} bitblk, {nib} iconblk, {nstr} frstr, "
          f"{nimg} frimg ({total} bytes) [native-RSHDR/LP64 for xrsrc_load]")


def main():
    if len(sys.argv) >= 2 and sys.argv[1] == "convert":
        # convert [--ilp32|--rshdr] SRC [OUT]:
        #   --ilp32  emits the 24-byte OBJECT / 4-byte ob_spec NRSC layout for
        #            GNU-ILP32 apps (QED), else the LP64 32-byte NRSC layout.
        #   --rshdr  emits a native-RSHDR file for an app's OWN rsc loader
        #            (tho-otto rsc_load.c / TeraDesk xrsrc_load), NOT for the
        #            kernel rsrc_load.
        rest = sys.argv[2:]
        ilp32 = rshdr = False
        if rest and rest[0] == "--ilp32":
            ilp32 = True
            rest = rest[1:]
        elif rest and rest[0] == "--rshdr":
            rshdr = True
            rest = rest[1:]
        src = rest[0]
        out = rest[1] if len(rest) > 1 else "OUT.RSC"
        if rshdr:
            convert_rshdr(src, out)
        else:
            convert(src, out, ilp32=ilp32)
        return
    out = sys.argv[1] if len(sys.argv) > 1 else "TEST.RSC"
    blob = build_demo()
    with open(out, "wb") as f:
        f.write(blob)
    print(f"wrote {out} ({len(blob)} bytes)")


if __name__ == "__main__":
    main()
