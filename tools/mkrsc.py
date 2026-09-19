#!/usr/bin/env python3
"""mkrsc — author a native NewTOS .RSC resource (see ADR-013).

The native format mirrors the kernel's repr(C) `Object`/`TedInfo`/… layout
(little-endian), with internal pointers stored as **byte offsets from the
blob base**; `rsrc_load` relocates them to absolute pointers on load. A
converted resource (`convert`, NRSC v9/v10) keeps the classic file's packed
char-grid OBJECT coordinates; `rsrc_load` converts them to pixels on load
with the live font cell and screen width (EmuTOS `rs_obfix`).

This tool emits a demo native resource directly, and (`convert`) reads a
classic big-endian Atari `.rsc` and re-emits it in this format.

Layout produced (`convert`, NRSC v13/v14):
    RSHDR(36B, LE u16, padded to 40) | NrscHeader(84B) @40 | [ptr tree-index]
    | [Object;N] | [TedInfo] | [BitBlk] | [IconBlk] | [CiconBlk] | [Cicon]
    | frstr table | frimg table | data
(the demo `build_demo` writes the historical 28-byte v1 header). Every offset
-- the RSHDR's u16 ones and the NrscHeader's u32 ones alike -- counts from the
file's first byte, the RSHDR.

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
# coordinates; we byte-swap and expand pointers to 64-bit offsets. Native
# struct sizes (repr(C)): Object 32, TedInfo 40, BitBlk 24, IconBlk 48.
#
# The OBJECT coordinates (ob_x/ob_y/ob_width/ob_height) are copied as the
# classic file's PACKED CHAR-GRID WORDS -- character count in the low byte,
# signed pixel offset in the high byte -- and NOT converted here. EmuTOS
# converts them when the resource is LOADED, with the live font cell and
# screen width of the machine it is loaded on (aes/gemrslib.c:53-99
# fix_chpos/rs_obfix, run over every object by fix_objects :606-637 from
# rs_fixit :816-820; gl_wchar/gl_hchar/gl_width set in gemgraf.c:393-405).
# The kernel loader does the same from NRSC v9 (LP64) / v10 (ILP32) on
# (tos-api/src/aes/resource.rs, fix_objects_coords). Doing the arithmetic
# here with a fixed 8x16 cell and a fixed 640-pixel screen -- as this tool
# did through v7/v8 -- froze every resource to that one profile: an
# 80-character root stayed 640 px wide on a 1920-wide workstation (audit
# B10, docs/analysis/audit-applinit-rsrc-load-2026-09-11.md, TASK/A40).

# Raster data (BITBLK `bi_pdata`, ICONBLK `ib_pmask`/`ib_pdata`) is read as
# 16-bit WORDS by every consumer — EmuTOS `gr_gblt(WORD *pimage, ...)`
# (aes/gemgraf.c:631), our kernel blit (`blit_plane`, tos-api/src/aes/object.rs)
# and an app's own loader (TeraDesk `xrsrc_load`) alike — so every raster must
# start on an even offset from the blob base.
#
# A classic Atari `.rsc` gets that for free: its rasters live in a separate,
# even-aligned `rsh_imdata` section and each is an even number of bytes
# (`bi_wb*bi_hl`, `((ib_wicon+15)//16)*2*ib_hicon`). We intern strings AND
# rasters into ONE `data` section, so an odd-length label would otherwise push
# the next raster onto an odd offset; `intern_bytes` pads to this boundary and
# the converters assert the result. Both `off_data` bases are even by
# construction, so the padding alone establishes the invariant.
RASTER_ALIGN = 2

# TEDINFO string reservations — the three pointers are NOT alike, and the
# classic `.rsc` format treats them differently (verified against
# `references/emutos` @ 1a7484ab):
#
#   te_ptext / te_txtlen
#       te_txtlen is a BUFFER CAPACITY, not a string length: it is the number
#       of bytes (NUL included) the field may hold, and the resource text is
#       often shorter — empty, even.  Everything that edits a field bounds
#       itself by te_txtlen and NOT by the string: EmuTOS `ob_edit`
#       (`aes/gemobed.c:394,407,420,432`), `inf_sset` → `strlcpy(text, pstr,
#       ted->te_txtlen)` (`util/optimize.c:111`), our `ins_char`
#       (`tos-api/src/aes/object.rs`).  A classic file therefore reserves the
#       full te_txtlen bytes behind te_ptext, which is measurable as the
#       distance to the next string: `gem.rsc` ted[0] te_ptext=0x0c8b,
#       te_ptmplt=0x0cb2 → 39 = te_txtlen 39, with 38 NULs in between.
#       Checked over every TEDINFO of `aes/gem.rsc` (13), TeraDesk's
#       `desktop.rsc` (104) and QED's `qed.rsc` (118): gap == te_txtlen in
#       ALL of them, 0 exceptions.  EmuTOS' own resource compiler holds the
#       same contract when it turns a `.rsc` into C: `tools/erd.c:3006-3007`
#       reserves `te_ptext_len += te_txtlen` bytes of `rs_te_text[]` per field.
#       => reserve the declared te_txtlen.
#
#   te_ptmplt / te_tmplen
#       te_tmplen is a STRING LENGTH ("length of template string",
#       `include/obdefs.h:182`), and the template is read-only: `ob_edit`
#       copies it out (`strcpy(D.g_tmpstr, edblk.te_ptmplt)`,
#       `gemobed.c:348`) and never writes it, EmuTOS recomputes the word as
#       `strlen+1` on load (`gemrslib.c:666`), and an app that wants a
#       different template supplies its own storage (TeraDesk
#       `rsc_fixtmplt`, `sdk/apps/teradesk/resource.c:351-352`).  Measured:
#       te_tmplen == strlen+1 in all 235 TEDINFOs above, so honouring the
#       declared value costs nothing and cannot under-reserve.
#       => reserve max(declared, strlen+1) — in practice strlen+1.
#
#   te_pvalid
#       has NO length word at all.  It is read-only too, and shorter than the
#       field on purpose: `ob_edit` expands it into a scratch buffer up to
#       te_tmplen (`gemobed.c:351-355`), `erd` even compresses it
#       (`shrink_valid`).  There is no declared length to reserve.
#       => strlen+1, as before.


def ted_reserve(d, coff, declared):
    """Bytes to reserve behind a TEDINFO string pointer in the data section.

    `declared` is the length word the classic file carries for that pointer
    (te_txtlen / te_tmplen), or 0 where the format declares none (te_pvalid).
    Never under-reserves the string itself, so a file whose length word is
    too small still yields a NUL-terminated buffer.
    """
    if coff == 0:
        return 0
    return max(d.index(b"\x00", coff) + 1 - coff, declared)

# Object types that don't carry a relocatable ob_spec pointer.
BOX_TYPES = {20, 25, 27}      # G_BOX, G_IBOX, G_BOXCHAR (literal word)
STR_TYPES = {26, 28, 32}      # G_BUTTON, G_STRING, G_TITLE
TED_TYPES = {21, 22, 29, 30}  # G_TEXT, G_BOXTEXT, G_FTEXT, G_FBOXTEXT
IMG_TYPE, ICON_TYPE = 23, 31  # G_IMAGE (BITBLK), G_ICON (ICONBLK)
CICON_TYPE = 33               # G_CICON (CICONBLK, new-format .rsc only)

CLS_OBJ, CLS_TED, CLS_IB, CLS_BB = 24, 28, 34, 14  # classic struct sizes
NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 32, 40, 48, 24  # native struct sizes

# Colour icons (TASK/A41, docs/api/aes/resource-fileselector.md §5). A
# new-format .rsc (`rsh_vrsn & NEW_FORMAT_RSC`, rsdefs.h:39) carries its
# CICONBLKs behind the classic sections, reached through the extension array
# at `rsh_rssize` (`get_ciconblkptr`, aes/gemrslib.c:484-508). On disk each
# CICONBLK is a variable-length record (§5.3): a 38-byte head -- three
# placeholder pointers, the eleven ICONBLK WORDs, and `mainlist` holding the
# NUMBER of CICONs that follow -- then `mono_words = ib_wicon/16 * ib_hicon`
# WORDs of mono data, the same of mono mask, a fixed 12-byte label buffer,
# then the CICON chain (§5.4): a 22-byte head -- num_planes, four placeholder
# pointers of which `sel_data` is a FLAG, and `next_res` = 1 if another
# variant follows -- then `mono_words * num_planes` WORDs of colour data, one
# plane of mask, and the same pair again when `sel_data` was flagged.
#
# The native record is the C struct the application links (gemlib mt_gem.h
# `CICONBLK { ICONBLK monoblk; CICON *mainlist; }`, `CICON { short num_planes;
# short *col_data, *col_mask, *sel_data, *sel_mask; struct cicon_data
# *next_res; }`), NOT a private table: TeraDesk copies a CICONBLK by value
# (`sdk/apps/teradesk/icon.c:1640,1675` `*h = *icons[n].ob_spec.ciconblk`)
# and reads `monoblk` through `ob_spec.ciconblk` (`dir.c:2090-2102`), so the
# bytes behind a G_CICON's ob_spec must BE that struct, with `mainlist` a real
# pointer at its C offset. The pointer fields hold blob offsets (0 = absent),
# relocated by the kernel like every other native pointer; the loader then
# runs EmuTOS's `fix_cicons` (best_match + device-form transform) over them.
# Native sizes: LP64 CICONBLK 56 (48 + 8), CICON 48 (2 + 6 pad + 5 * 8);
# ILP32 CICONBLK 40 (36 + 4), CICON 24 (2 + 2 pad + 5 * 4).
CLS_CIB_HEAD, CLS_CIC_HEAD = 38, 22  # on-disk record heads (pre-fix-up)
CLS_CIB_TEXT = 12                    # fixed mono label buffer (gemrslib.c:245)
NEW_FORMAT_RSC = 0x0004              # rsdefs.h:39
NAT_CIB, NAT_CIC = 56, 48            # native LP64 CICONBLK / CICON

# Largest blob the kernel loader holds (`tos-api/src/aes/resource.rs`
# `RSC_CAP`): a larger file is refused by `rsrc_load`, never truncated, so the
# converter fails loudly instead of emitting a resource that cannot load.
RSC_CAP = 64 * 1024

# The native RSHDR preamble of a kernel-loadable resource (NRSC v13/v14,
# TASK/A41 part 2; `resource.rs` NRSC_VER_RSHDR / RSHDR_PAD). EmuTOS publishes
# the loaded FILE as AESGLOBAL.ap_rscmem (`rs_readit`, aes/gemrslib.c:787:
# `rs_global->ap_rscmem = rs_hdr`) -- the classic RSHDR at its head -- and
# applications parse that header themselves: TeraDesk's xrsrc_gaddr
# (sdk/apps/teradesk/rsc_load.c:496-541, on the AES-loaded cicons.rsc from
# icon.c:2750) reads aes_global[7], tests IS_XRSC_HEADER (rsh_vrsn & 3 == 3 ->
# the RSXHDR with `unsigned long` fields, else the 18 x u16 RSHDR) and takes
# rsh_trindex / rsh_object / ... as byte offsets from that address. So the
# image the kernel hands out must BEGIN with that header: 18 little-endian
# u16 in the classic field order (rsdefs.h:16-36), offsets from the file's
# first byte -- the representation `--rshdr` already writes and TeraDesk's
# parser already reads -- padded to 40 bytes so the NRSC header and every
# pointer-bearing section behind it keep 8-byte alignment. The kernel checks
# the preamble against the NRSC header before it relocates anything.
#
# Every RSHDR field is a u16, so the whole image has to stay below 64 KiB;
# the converter refuses a larger one (RSC_CAP less the preamble, per the
# A41 part-2 brief -- the hard bounds are 0xFFFF per field and RSC_CAP).
RSHDR_SIZE = 36                      # 18 x u16 (rsdefs.h:16-36)
RSHDR_PAD = 40                       # the preamble, padded for alignment
RSHDR_FIELDS = ("vrsn", "object", "tedinfo", "iconblk", "bitblk", "frstr",
                "string", "imdata", "frimg", "trindex", "nobs", "ntree",
                "nted", "nib", "nbb", "nstring", "nimages", "rssize")
RSC_VERSION_MASK = 0x0003            # rsc_load.h:68; & 3 == 3 -> RSXHDR


def pack_rshdr(fields, where):
    """Pack the 18 RSHDR u16 fields little-endian, padded to RSHDR_PAD.

    `fields` is the (vrsn, object, tedinfo, iconblk, bitblk, frstr, string,
    imdata, frimg, trindex, nobs, ntree, nted, nib, nbb, nstring, nimages,
    rssize) tuple; each must fit a u16 (the classic header's width, and the
    reason the image is bounded at 64 KiB), and `vrsn & 3` must not be 3 --
    that would make TeraDesk's IS_XRSC_HEADER read the u16 fields as
    `unsigned long`s. Either violation aborts loudly.
    """
    for name, v in zip(RSHDR_FIELDS, fields):
        if not 0 <= v <= 0xFFFF:
            sys.exit(f"{where}: native-RSHDR field rsh_{name}={v} exceeds the u16 "
                     f"classic-header limit (file too large for this format)")
    if fields[0] & RSC_VERSION_MASK == RSC_VERSION_MASK:
        sys.exit(f"{where}: rsh_vrsn={fields[0]:#x} would select the RSXHDR "
                 f"(unsigned long) header layout (IS_XRSC_HEADER); the fields "
                 f"written here are u16")
    return struct.pack("<18H", *fields) + b"\x00" * (RSHDR_PAD - RSHDR_SIZE)


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


def read_cicon_table(d, vrsn, rssize):
    """Parse the CICONBLK extension of a new-format classic `.rsc`.

    Returns a list of blocks, one per CICONBLK in table order (= the index a
    G_CICON's classic `ob_spec` holds, `fix_objects` gemrslib.c:623-626):
    `(words, pdata, pmask, ptext, variants)` with `words` the eleven ICONBLK
    WORDs (`ib_char` .. `ib_htext`), `pdata`/`pmask`/`ptext` the classic file
    offsets of the mono data (`mono_words` WORDs), mono mask (same) and the
    12-byte label, and `variants` a list of `(num_planes, col_data, col_mask,
    sel_data, sel_mask)` file offsets (`sel_*` = 0 when the variant has no
    SELECTED image). Mirrors `fix_cicons` / `fixup_all_ciconblks` /
    `fixup_colour_icons` (gemrslib.c:549-573, :226-256, :185-217) -- the same
    walk EmuTOS does in place, done here on the read-only file. An old-format
    file (no `NEW_FORMAT_RSC` bit) or one whose extension array has no CICON
    table (`extarray[1]` 0 or -1) yields an empty list.
    """
    if not vrsn & NEW_FORMAT_RSC:
        return []
    # extarray[0] = true file length, extarray[1] = CICON table offset
    # (`get_ciconblkptr`); [2..] are "other extensions" EmuTOS never reads.
    true_len, cptr = struct.unpack(">ii", d[rssize:rssize + 8])
    if true_len > len(d):
        sys.exit(f"new-format .rsc declares {true_len} bytes, file has {len(d)}")
    if cptr in (0, -1):
        return []
    # The pointer table: placeholders up to a -1 sentinel; the count is only
    # ever derived by walking to the sentinel (gemrslib.c:560-561).
    n = 0
    while struct.unpack(">i", d[cptr + n * 4:cptr + n * 4 + 4])[0] != -1:
        n += 1
    p = cptr + (n + 1) * 4         # `cicondata = (CICON *)(p+1)` (:563)
    blocks = []
    for _ in range(n):
        head = struct.unpack(">IIIhhhhhhhhhhhI", d[p:p + CLS_CIB_HEAD])
        words = head[3:14]
        num_cicons = head[14]
        wicon, hicon = words[5], words[6]
        mono_words = (wicon // 16) * hicon    # gemrslib.c:235 (WORDs)
        q = p + CLS_CIB_HEAD
        pdata = q
        q += mono_words * 2
        pmask = q
        q += mono_words * 2
        ptext = q
        q += CLS_CIB_TEXT
        variants = []
        for _ in range(num_cicons):
            planes, _cd, _cm, sel_flag, _sm, next_res = struct.unpack(
                ">hIIIII", d[q:q + CLS_CIC_HEAD])
            q += CLS_CIC_HEAD
            col_data = q
            q += mono_words * planes * 2
            col_mask = q
            q += mono_words * 2
            sel_data = sel_mask = 0
            if sel_flag:
                sel_data = q
                q += mono_words * planes * 2
                sel_mask = q
                q += mono_words * 2
            variants.append((planes, col_data, col_mask, sel_data, sel_mask))
            # `next_res == 1L` chains into the next variant right here; any
            # other value ends the chain (gemrslib.c:211-214). The count in
            # `mainlist` already bounds the loop, so the flag is only
            # cross-checked, not trusted.
            if (next_res == 1) != (len(variants) < num_cicons):
                sys.exit(f"CICONBLK {len(blocks)}: next_res/mainlist count "
                         f"disagree at variant {len(variants)}")
        if q > len(d):
            sys.exit(f"CICONBLK {len(blocks)} runs past the end of the file")
        blocks.append((words, pdata, pmask, ptext, variants))
        p = q
    return blocks


def convert(path, out, ilp32=False):
    # Native struct sizes + pack formats by target ABI (ADR-020 stage 4).
    # ob_spec / te_p* / ib_p* / bi_pdata are POINTERS: 8 bytes on LP64, 4 on
    # ILP32. An ILP32 app (QED, GNU -mabi=ilp32) links a 24-byte OBJECT with a
    # 4-byte ob_spec @ 12 and correspondingly smaller TEDINFO/ICONBLK/BITBLK
    # (verified by compile-time sizeof against gemlib mt_gem.h: OBJECT=24,
    # TEDINFO=28, ICONBLK=36, BITBLK=16). The relocated pointer values still fit
    # a 4-byte field because the per-process RSC window sits in low VA. The
    # NrscHeader's own fields are u32 and ABI-independent; `version` marks the
    # OBJECT width (odd = LP64/64-bit ob_spec, even = ILP32/32-bit ob_spec) for
    # tooling, but the kernel relocates by the *caller's* ABI -- the tree index
    # included, from v7/v8 on, where it became a PTR-wide pointer table.
    # PTR = free-string/free-image/tree-index pointer-slot width (matches
    # ob_spec).
    # version 5 = LP64/68-byte header, 6 = ILP32/68-byte header: both carry the
    # frstr/frimg pointer tables (as v3/v4 did) AND the TEDINFO/ICONBLK/BITBLK
    # array bases + counts, so rsrc_gaddr/rsrc_saddr resolve the R_TEDINFO/
    # R_ICONBLK/R_BITBLK family + their sub-pointer aliases (AUD-P6-002).
    # version 7 = LP64, 8 = ILP32: same 68-byte header, but the **tree index**
    # is a relocatable pointer table like frstr/frimg -- PTR-wide slots holding
    # the byte OFFSET of each tree's root OBJECT, which the loader relocates to
    # addresses. That is what EmuTOS's `fix_trindex` publishes through
    # AESGLOBAL.ap_ptree (aes/gemrslib.c:591-603: `ptreebase = get_sub(0,
    # rsh_trindex, sizeof(LONG)); rs_global->ap_ptree = ptreebase;` then
    # `fix_long(ptreebase+ii)` per tree), and it is what `get_addr(R_TREE, i)`
    # reads back (`return rs_global->ap_ptree[rsindex];`, gemrslib.c:124).
    # v1..v6 held root OBJECT *indices* in fixed 4-byte slots instead, which no
    # application could be handed as a pointer array (A35).
    # version 9 = LP64, 10 = ILP32: same 68-byte header, but the OBJECT
    # coordinates are the classic file's packed CHAR-GRID words, converted by
    # the kernel at load with the live gl_wchar/gl_hchar/gl_width (EmuTOS
    # rs_fixit/fix_objects/rs_obfix, aes/gemrslib.c:816-820, :606-637,
    # :53-99) -- see the section comment above (A40). v1..v8 hold pixels
    # computed here with a fixed 8x16/640 profile. The kernel loader stays
    # backward-compatible with every earlier header: v1/v2 (28-byte, no
    # tables), v3/v4 (44-byte, +frstr/frimg tables), v5/v6 (68-byte,
    # +substruct bases, index tree table), v7/v8 (pointer tree index, pixels).
    # version 11 = LP64, 12 = ILP32: an 84-byte header -- the v5..v10 fields
    # plus the CICONBLK array base + count (off_cib/n_cib) and the CICON
    # array base + count (off_cic/n_cic) -- for the colour icons of a
    # new-format .rsc (TASK/A41; see the CICON section comment above). The
    # records are the application's C structs with blob offsets in the
    # pointer fields; a G_CICON's ob_spec is the byte offset of its CICONBLK,
    # relocated like every other pointer ob_spec. An old-format file emits the
    # same header with both counts 0.
    # version 13 = LP64, 14 = ILP32: the same 84-byte NRSC header, but the
    # file BEGINS with a native RSHDR (see the RSHDR section comment above):
    # 18 LE u16 in the classic field order, offsets from the file's first
    # byte, padded to 40 bytes; the NRSC header follows at offset 40 and all
    # of its offsets count from the file's first byte too (one reference
    # point). The loaded image is then the file with the RSHDR at its head,
    # which is what EmuTOS publishes as ap_rscmem and what TeraDesk's own
    # xrsrc_gaddr parses (TASK/A41 part 2). rsh_vrsn is the classic file's
    # (0 for a plain file, 4 = NEW_FORMAT_RSC for one with colour icons, as
    # EmuTOS leaves it in memory); an RSXHDR source (vrsn & 3 == 3, long
    # fields) is refused up front, because the classic header is read as
    # 18 u16 here.
    if ilp32:
        NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 24, 28, 36, 16
        OBJ_FMT = "<hhhHHHIhhhh"          # 4-byte ob_spec @ 12
        TED_FMT = "<IIIhhhhhhhh"          # 4-byte te_ptext/te_ptmplt/te_pvalid
        BB_FMT = "<Ihhhhh2x"              # 4-byte bi_pdata, pad to 16
        IB_FMT = "<IIIhhhhhhhhhhh2x"      # 4-byte ib_pmask/ib_pdata/ib_ptext, 36
        NAT_CIB, NAT_CIC = 40, 24
        CIB_FMT = "<IIIhhhhhhhhhhh2xI"    # ICONBLK(36) + 4-byte mainlist @ 36
        CIC_FMT = "<h2xIIIII"             # num_planes, pad, 5 x 4-byte ptrs
        PTR, PTR_FMT, version = 4, "<I", 14
    else:
        NAT_OBJ, NAT_TED, NAT_IB, NAT_BB = 32, 40, 48, 24
        OBJ_FMT = "<hhhHHH4xQhhhh"        # 8-byte ob_spec @ 16
        TED_FMT = "<QQQhhhhhhhh"
        BB_FMT = "<Qhhhhh6x"
        IB_FMT = "<QQQhhhhhhhhhhh2x"
        NAT_CIB, NAT_CIC = 56, 48
        CIB_FMT = "<QQQhhhhhhhhhhh2xQ"    # ICONBLK(48) + 8-byte mainlist @ 48
        CIC_FMT = "<h6xQQQQQ"             # num_planes, pad, 5 x 8-byte ptrs
        PTR, PTR_FMT, version = 8, "<Q", 13

    d = open(path, "rb").read()
    H = struct.unpack(">18H", d[:36])
    (vrsn, p_obj, p_ted, p_ib, p_bb, p_frstr, p_str, p_imd, p_frimg,
     p_tri, nobs, ntree, nted, nib, nbb, nstr, nimg, rssize) = H
    if vrsn & RSC_VERSION_MASK == RSC_VERSION_MASK:
        sys.exit(f"{path}: rsh_vrsn={vrsn:#x} is an extended (RSXHDR, long-field) "
                 f"header; this converter reads the classic 18 x u16 RSHDR only")
    # Colour icons: the CICONBLK table of a new-format file (empty otherwise).
    cicon_blocks = read_cicon_table(d, vrsn, rssize)
    ncib = len(cicon_blocks)
    ncic = sum(len(b[4]) for b in cicon_blocks)

    # Native section base offsets (counts are fixed up front). The v5/v6 header
    # is 68 bytes: the v3/v4 fields (through off_frimg/nimg) plus the TEDINFO/
    # ICONBLK/BITBLK array bases + counts (off_ted/nted, off_ib/nib, off_bb/nbb);
    # v11/v12 append off_cib/ncib and off_cic/ncic (84 bytes); v13/v14 put the
    # 40-byte RSHDR preamble in front, so the NRSC header sits at NRSC_OFF and
    # the first section at HDR. The CICONBLK and CICON arrays follow the
    # ICONBLKs (both record sizes are multiples of PTR, so the alignment of
    # everything behind them is unchanged). A free-string then a free-image
    # pointer table sits between them and the string/image data (each table =
    # count * PTR bytes).
    NRSC_OFF = RSHDR_PAD
    HDR = NRSC_OFF + 84
    off_tri = HDR
    # The OBJECT array (and every 8-byte-pointer section behind it) must start
    # naturally aligned: the kernel dereferences it as `repr(C)` `Object`
    # records (`relocate_blob_core`, `ObjTree`), and so does every app. The
    # 84-byte NRSC header (at 40) leaves `off_tri` at 4 mod 8, so an LP64
    # tree index of 8-byte slots would land the objects at 4 mod 8 too; pad
    # the index up to the record alignment. `nat_ted`/`nat_bb`/`nat_ib`/`PTR`
    # are all multiples of PTR, so aligning the OBJECT array aligns every
    # later section with it.
    n_tri = ntree * PTR
    tri_pad = -(off_tri + n_tri) % PTR
    off_obj = off_tri + n_tri + tri_pad
    off_ted = off_obj + nobs * NAT_OBJ
    off_bb = off_ted + nted * NAT_TED
    off_ib = off_bb + nbb * NAT_BB
    off_cib = off_ib + nib * NAT_IB
    off_cic = off_cib + ncib * NAT_CIB
    off_frstr = off_cic + ncic * NAT_CIC
    off_frimg = off_frstr + nstr * PTR
    off_data = off_frimg + nimg * PTR

    data = bytearray()
    # Namespaced so a reservation can never be satisfied from a shorter cached
    # copy: ("s", coff, size) = read-only string, ("t", coff, size) = a
    # TEDINFO te_ptext buffer, ("b", coff, length) = raster bytes, ("f", coff,
    # size) = a fixed-size text buffer (a CICONBLK's 12-byte label).
    interned = {}
    rasters = []          # native offsets of every raster placed by intern_bytes
    placed = []           # (native offset, reserved bytes) of every datum

    def intern_fixed(coff, size):
        """Intern a fixed-size text buffer (the CICONBLK mono label, 12 bytes).

        EmuTOS reads it as a C string (`ib_ptext`, gemrslib.c:244-245) that
        the file lays out as exactly `size` bytes; the bytes are copied
        verbatim. Should the buffer lack a NUL, one is appended (13 bytes)
        rather than handing the renderer an unterminated string.
        """
        key = ("f", coff, size)
        if key in interned:
            return interned[key]
        raw = d[coff:coff + size]
        if b"\x00" not in raw:
            print(f"warning: label buffer at {coff} has no NUL in {size} bytes; "
                  f"terminated", file=sys.stderr)
            raw += b"\x00"
        nat = off_data + len(data)
        data.extend(raw)
        interned[key] = nat
        placed.append((nat, len(raw)))
        return nat

    def intern_str(coff, reserve=0, kind="s"):
        """Intern a classic NUL-terminated string, padded out to `reserve`.

        `reserve` is the size the *format* declares behind the pointer (see
        `ted_reserve`); the trailing bytes are NUL, so a consumer bounded by
        the declared length writes inside the blob instead of past the string.

        `kind` separates the writable te_ptext buffers ("t") from read-only
        strings ("s"): a text buffer is only ever shared with another te_ptext
        that the source file already aliased, never with a label or template
        that happens to sit at the same classic offset. Two TEDINFOs aliasing
        one te_ptext with *different* te_txtlen get one buffer each — the
        source file could not have satisfied both from one gap anyway.
        """
        if coff == 0:
            return 0
        end = d.index(b"\x00", coff)
        size = max(end + 1 - coff, reserve)
        key = (kind, coff, size)
        if key in interned:
            return interned[key]
        nat = off_data + len(data)
        data.extend(d[coff:end + 1])
        data.extend(b"\x00" * (size - (end + 1 - coff)))
        interned[key] = nat
        placed.append((nat, size))
        return nat

    def intern_bytes(coff, length):
        if coff == 0 or length <= 0:
            return 0
        key = ("b", coff, length)
        if key in interned:
            return interned[key]
        # WORD-align the raster before it is placed (see RASTER_ALIGN): the
        # strings interned into this same section have arbitrary lengths, so
        # without this pad an odd-length label pushes the next raster onto an
        # odd offset. The TEDINFO reservations above only change how many
        # bytes a string contributes, so the pad still establishes it.
        data.extend(b"\x00" * (-(off_data + len(data)) % RASTER_ALIGN))
        nat = off_data + len(data)
        # Raster bytes only (intern_bytes is called for BITBLK bi_pdata and
        # ICONBLK ib_pmask/ib_pdata exclusively; strings go through intern_str).
        # Byte-swap each 16-bit word big-endian classic -> little-endian native.
        data.extend(swap16_raster(d[coff:coff + length]))
        interned[key] = nat
        rasters.append(nat)
        placed.append((nat, length))
        return nat

    # Native struct offset of classic struct #k in a fixed array.
    def nat_of(coff, base, csize, nbase, nsize):
        return nbase + ((coff - base) // csize) * nsize

    # TEDINFOs (remap text/template/valid string pointers).
    teds = bytearray()
    ted_texts = []        # (native offset, reserved bytes, te_txtlen) per TEDINFO
    for i in range(nted):
        o = p_ted + i * CLS_TED
        (ptext, ptmpl, pvalid, font, j1, just, color, j2, thick, txtlen,
         tmplen) = struct.unpack(">IIIhhhhhhhh", d[o:o + CLS_TED])
        # See `ted_reserve`: te_txtlen is a buffer capacity (reserved in full,
        # private), te_tmplen a read-only string length, te_pvalid has none.
        r_text = ted_reserve(d, ptext, txtlen)
        nat_text = intern_str(ptext, r_text, kind="t")
        ted_texts.append((nat_text, r_text, txtlen))
        teds.extend(struct.pack(
            TED_FMT, nat_text, intern_str(ptmpl, ted_reserve(d, ptmpl, tmplen)),
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

    # CICONBLKs + their CICON chains (colour icons, new-format file only).
    # One native CICONBLK per table entry, in table order; the CICON records
    # of all blocks form one array, each block's chain linked by offset
    # (`mainlist` -> first variant, `next_res` -> next, 0 = end), exactly the
    # linked shape `fixup_colour_icons` leaves in memory (gemrslib.c:185-217)
    # -- with offsets where EmuTOS already has pointers. The mono rasters are
    # interned like an ICONBLK's; the colour rasters are the STANDARD-form
    # planes of the file (one plane after another, `transform_cicon`
    # `fd_stand = TRUE`, gemrslib.c:351), byte-swapped like every raster. The
    # device-form conversion is the loader's (`best_match` + transform at
    # rsrc_load), so the file stays device-independent (A40's lesson).
    cibs = bytearray()
    cics = bytearray()
    cic_index = 0
    for words, pdata, pmask, ptext, variants in cicon_blocks:
        wicon, hicon = words[5], words[6]
        mono_words = (wicon // 16) * hicon
        mainlist = off_cic + cic_index * NAT_CIC if variants else 0
        cibs.extend(struct.pack(
            CIB_FMT, intern_bytes(pmask, mono_words * 2),
            intern_bytes(pdata, mono_words * 2),
            intern_fixed(ptext, CLS_CIB_TEXT), *words, mainlist))
        for k, (planes, col_data, col_mask, sel_data, sel_mask) in enumerate(variants):
            nxt = off_cic + (cic_index + 1) * NAT_CIC if k + 1 < len(variants) else 0
            cics.extend(struct.pack(
                CIC_FMT, planes,
                intern_bytes(col_data, mono_words * planes * 2),
                intern_bytes(col_mask, mono_words * 2),
                intern_bytes(sel_data, mono_words * planes * 2),
                intern_bytes(sel_mask, mono_words * 2),
                nxt))
            cic_index += 1
    assert cic_index == ncic and len(cibs) == ncib * NAT_CIB and len(cics) == ncic * NAT_CIC

    # OBJECTs (remap ob_spec by type; coordinates stay packed char-grid
    # words -- the kernel runs rs_obfix on them at load, see above).
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
        elif base_type == CICON_TYPE:
            # Classic ob_spec = 0-based INDEX into the CICONBLK pointer table
            # (`obj->ob_spec = (LONG)ciconblkptr[obj->ob_spec]`, gemrslib.c:
            # 623-626), not a byte offset like every other type: turned into
            # the byte offset of the native record here, so the loader
            # relocates it with the generic `ob_spec += base` rule.
            if spec >= ncib:
                sys.exit(f"object {i}: G_CICON index {spec} but the file has "
                         f"{ncib} CICONBLK(s)")
            nspec = off_cib + spec * NAT_CIB
        else:
            nspec = 0  # G_PROGDEF: unsupported, drawn empty
        objs.extend(struct.pack(
            OBJ_FMT, nxt, head, tail, typ, flags, state, nspec, x, y, w, h))

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

    # Tree index → PTR-wide byte offsets of each tree's root OBJECT, exactly
    # like the frstr/frimg tables above: the loader relocates them in place, so
    # after `rsrc_load` slot i holds the address of tree i's root. This is the
    # array EmuTOS hands the application through AESGLOBAL.ap_ptree
    # (`fix_trindex`, aes/gemrslib.c:591-603) and reads back in
    # `get_addr(R_TREE, i)` (gemrslib.c:124).
    tri = bytearray()
    for t in range(ntree):
        toff = struct.unpack(">I", d[p_tri + t * 4:p_tri + t * 4 + 4])[0]
        tri.extend(struct.pack(PTR_FMT,
                               off_obj + ((toff - p_obj) // CLS_OBJ) * NAT_OBJ))
    tri.extend(b"\x00" * tri_pad)

    total = off_data + len(data)
    # The kernel refuses a blob larger than its buffer (`header_is_sound`
    # against RSC_CAP), and the RSHDR preamble's u16 fields cannot name a
    # byte at or past 64 KiB; say so here, loudly, instead of writing such a
    # file (see the RSHDR section comment for the bound).
    if total > RSC_CAP - RSHDR_PAD:
        sys.exit(f"{path}: native resource is {total} bytes, over the "
                 f"{RSC_CAP - RSHDR_PAD}-byte limit of the RSHDR-headed image "
                 f"(RSC_CAP {RSC_CAP} less the {RSHDR_PAD}-byte preamble, "
                 f"tos-api/src/aes/resource.rs) -- it would be refused by "
                 f"rsrc_load")
    # The RSHDR preamble (classic field order, offsets from the file's first
    # byte): rsh_string/rsh_imdata name the shared string+raster data section
    # (the kernel only bounds them; EmuTOS reads neither), rsh_rssize is the
    # whole image. The kernel checks every other field against the NRSC
    # header (`rshdr_is_consistent`), so the two are written from the same
    # numbers here.
    rshdr = pack_rshdr(
        (vrsn, off_obj, off_ted, off_ib, off_bb, off_frstr, off_data, off_data,
         off_frimg, off_tri, nobs, ntree, nted, nib, nbb, nstr, nimg, total),
        path)
    # v13/v14 NRSC header at NRSC_OFF: magic + 20 u32 (84 bytes) -- the
    # v7..v10 68-byte shape plus the two CICON arrays. Field order MUST match
    # the kernel `NrscHeader` struct (tos-api/src/aes/resource.rs): the v3/v4
    # fields, then off_ted/nted, off_ib/nib, off_bb/nbb (the TEDINFO/ICONBLK/
    # BITBLK bases), then off_cib/ncib, off_cic/ncic (the CICONBLK/CICON
    # bases).
    header = struct.pack(
        "<4s20I", b"NRSC", version, ntree, nobs, off_tri, off_obj, total,
        off_frstr, nstr, off_frimg, nimg,
        off_ted, nted, off_ib, nib, off_bb, nbb,
        off_cib, ncib, off_cic, ncic)
    assert len(rshdr) == NRSC_OFF and len(rshdr) + len(header) == HDR
    blob = (rshdr + header + tri + objs + teds + bbs + ibs + bytes(cibs)
            + bytes(cics) + bytes(frstr) + bytes(frimg) + data)
    assert len(blob) == total, (len(blob), total)
    odd = [r for r in rasters if r % RASTER_ALIGN]
    assert not odd, f"raster(s) at odd offsets {odd} (see RASTER_ALIGN)"
    # Postcondition for the reservations (see `ted_reserve`): every TEDINFO
    # text buffer holds at least its declared te_txtlen bytes, lies inside the
    # blob, and nothing else was interned into it.
    placed.sort()
    for (a, sa), (b, _sb) in zip(placed, placed[1:]):
        assert a + sa <= b, (
            f"data placements overlap: {sa} bytes at {a} run into {b}")
    for i, (nat, res, txtlen) in enumerate(ted_texts):
        if nat == 0:
            continue
        assert res >= txtlen, (
            f"tedinfo[{i}]: reserved {res} B behind te_ptext, te_txtlen is "
            f"{txtlen} (see ted_reserve)")
        assert nat + res <= total, (
            f"tedinfo[{i}]: te_ptext buffer [{nat},{nat + res}) runs past the "
            f"{total}-byte blob")
    open(out, "wb").write(blob)
    print(f"converted {path} -> {out}: {ntree} trees, {nobs} objects, "
          f"{nted} tedinfo, {nbb} bitblk, {nib} iconblk, {ncib} ciconblk, "
          f"{ncic} cicon, {nstr} frstr, {nimg} frimg ({total} bytes, "
          f"RSHDR vrsn {vrsn}) "
          f"[{'ILP32/24B-OBJECT' if ilp32 else 'LP64/32B-OBJECT'}, NRSC v{version}]")


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
    if vrsn & NEW_FORMAT_RSC:
        # An application's own rsc_load.c parser has no CICON path; the
        # kernel-loadable `convert` (NRSC v11/v12) does.
        sys.exit("new-format (CICONBLK) .rsc not supported by --rshdr")

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
    # Namespaced so a reservation can never be satisfied from a shorter cached
    # copy: ("s", coff, size) = read-only string, ("t", coff, size) = a
    # TEDINFO te_ptext buffer, ("b", coff, length) = raster bytes.
    interned = {}
    rasters = []          # native offsets of every raster placed by intern_bytes
    placed = []           # (native offset, reserved bytes) of every datum

    def intern_str(coff, reserve=0, kind="s"):
        """Intern a classic NUL-terminated string, padded out to `reserve`.

        `reserve` is the size the *format* declares behind the pointer (see
        `ted_reserve`); the trailing bytes are NUL, so a consumer bounded by
        the declared length writes inside the blob instead of past the string.

        `kind` separates the writable te_ptext buffers ("t") from read-only
        strings ("s"): a text buffer is only ever shared with another te_ptext
        that the source file already aliased, never with a label or template
        that happens to sit at the same classic offset. Two TEDINFOs aliasing
        one te_ptext with *different* te_txtlen get one buffer each — the
        source file could not have satisfied both from one gap anyway.
        """
        if coff == 0:
            return 0
        end = d.index(b"\x00", coff)
        size = max(end + 1 - coff, reserve)
        key = (kind, coff, size)
        if key in interned:
            return interned[key]
        nat = off_data + len(data)
        data.extend(d[coff:end + 1])
        data.extend(b"\x00" * (size - (end + 1 - coff)))
        interned[key] = nat
        placed.append((nat, size))
        return nat

    def intern_bytes(coff, length):
        if coff == 0 or length <= 0:
            return 0
        key = ("b", coff, length)
        if key in interned:
            return interned[key]
        # WORD-align the raster before it is placed (see RASTER_ALIGN): the
        # strings interned into this same section have arbitrary lengths, so
        # without this pad an odd-length label pushes the next raster onto an
        # odd offset. The TEDINFO reservations above only change how many
        # bytes a string contributes, so the pad still establishes it.
        data.extend(b"\x00" * (-(off_data + len(data)) % RASTER_ALIGN))
        nat = off_data + len(data)
        # Raster bytes only (intern_bytes is called for BITBLK bi_pdata and
        # ICONBLK ib_pmask/ib_pdata exclusively; strings go through intern_str).
        # Byte-swap each 16-bit word big-endian classic -> little-endian native.
        data.extend(swap16_raster(d[coff:coff + length]))
        interned[key] = nat
        rasters.append(nat)
        placed.append((nat, length))
        return nat

    def nat_of(coff, base, csize, nbase, nsize):
        return nbase + ((coff - base) // csize) * nsize

    teds = bytearray()
    ted_texts = []        # (native offset, reserved bytes, te_txtlen) per TEDINFO
    for i in range(nted):
        o = p_ted + i * CLS_TED
        (ptext, ptmpl, pvalid, font, j1, just, color, j2, thick, txtlen,
         tmplen) = struct.unpack(">IIIhhhhhhhh", d[o:o + CLS_TED])
        # See `ted_reserve`: te_txtlen is a buffer capacity (reserved in full,
        # private), te_tmplen a read-only string length, te_pvalid has none.
        r_text = ted_reserve(d, ptext, txtlen)
        nat_text = intern_str(ptext, r_text, kind="t")
        ted_texts.append((nat_text, r_text, txtlen))
        teds.extend(struct.pack(
            TED_FMT, nat_text, intern_str(ptmpl, ted_reserve(d, ptmpl, tmplen)),
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
    # Every RSHDR field is u16 — the whole native file must stay below 64K
    # (`pack_rshdr` refuses otherwise). rsh_vrsn = 0: no XRSC, no CICON
    # extension (refused above).
    assert HDR == RSHDR_PAD
    header = pack_rshdr(
        (0, off_obj, off_ted, off_ib, off_bb, off_frstr, off_data, off_data,
         off_frimg, off_tri, nobs, ntree, nted, nib, nbb, nstr, nimg, total),
        path)
    blob = header + tri + objs + teds + bbs + ibs + bytes(frstr) + bytes(frimg) + data
    assert len(blob) == total, (len(blob), total)
    odd = [r for r in rasters if r % RASTER_ALIGN]
    assert not odd, f"raster(s) at odd offsets {odd} (see RASTER_ALIGN)"
    # Postcondition for the reservations (see `ted_reserve`): every TEDINFO
    # text buffer holds at least its declared te_txtlen bytes, lies inside the
    # blob, and nothing else was interned into it.
    placed.sort()
    for (a, sa), (b, _sb) in zip(placed, placed[1:]):
        assert a + sa <= b, (
            f"data placements overlap: {sa} bytes at {a} run into {b}")
    for i, (nat, res, txtlen) in enumerate(ted_texts):
        if nat == 0:
            continue
        assert res >= txtlen, (
            f"tedinfo[{i}]: reserved {res} B behind te_ptext, te_txtlen is "
            f"{txtlen} (see ted_reserve)")
        assert nat + res <= total, (
            f"tedinfo[{i}]: te_ptext buffer [{nat},{nat + res}) runs past the "
            f"{total}-byte blob")
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
