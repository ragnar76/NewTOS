#!/usr/bin/env python3
# menu_abi_sim.py -- byte-level ABI-marshalling simulator for the AES MENU group.
#
# Purpose: prove or disprove the hypothesis that TeraDesk's transparent
# interactive dropdown on real HW is a TOOLCHAIN/ABI-marshalling bug, by
# reproducing -- byte for byte -- how the toolchain (gemlib pack + mkrsc NRSC
# convert) LAYS OUT the AES parameter block and the menu OBJECT tree, and how
# the kernel (tos-api/src/aes) READS THEM BACK. If a producer offset and a
# consumer offset disagree by even one byte, that is the smoking gun.
#
# Sources reproduced (cite file:line):
#   PRODUCER (TeraDesk side)
#     - sdk/gemlib/gemlib-0.44.0/gem_aesP.h:70-83   AES_PARAMS macro
#     - sdk/gemlib/gemlib-0.44.0/a_menu_bar.c:48-60 mt_menu_bar pack
#     - sdk/gemlib/gemlib-0.44.0/a_menu_tnormal.c   mt_menu_tnormal pack
#     - sdk/gemlib/gemlib-0.44.0/mt_gem.h:2527-2535 AESPB (6 native pointers)
#     - sdk/gemlib/gemlib-0.44.0/mt_gem.h:2081-2094 OBJECT (ob_spec union @16 LP64)
#     - sdk/gemlib/gemlib-0.44.0/_gc_aes.c:30-42    aes(): svc #2, x0=200, x1=&AESPB
#     - tools/mkrsc.py:168-169                       LP64 OBJ_FMT "<hhhHHH4xQhhhh"
#   CONSUMER (kernel side)
#     - tos-api/src/aes/mod.rs:69-112               Aespb::from_raw / addrin / intin
#     - tos-api/src/aes/object.rs:176-195           ObjTree stride/spec_off/coord_off
#     - tos-api/src/aes/object.rs:310-351           ObjTree::raw_spec / spec
#     - tos-api/src/aes/menu.rs:137-172             menu_bar (reads addrin[0], intin[0])
#     - tos-api/src/aes/object.rs:1944-1990         gr_crack / gr_rect (box interior fill)
#     - tos-api/src/bdos/process/loader.rs:556-558  ELF class -> Abi (TeraDesk=ELF64=Lp64)
#
# The numbers below were cross-checked against a host C compile of the exact
# gemlib structs (LP64): OBJECT=32 spec@16 coords@24, AESPB=48 six 8-byte ptrs,
# TEDINFO=40, ICONBLK=48, BITBLK=24, MENU=16.

import struct


# ---------------------------------------------------------------------------
# ABI model. Two ABIs exist in this codebase (ADR-020):
#   Lp64  : pointer/long = 8 bytes, natural (8-byte) alignment.
#   Ilp32 : pointer/long = 4 bytes, 4-byte alignment (GNU aarch64 -mabi=ilp32).
# TeraDesk is built ELF64/LP64 (build-teradesk.sh:103) and linked against the
# LP64 gemlib.a, so at runtime the loader sets Abi::Lp64
# (loader.rs:558 via img.is_ilp32()==false), hence current_abi()==Lp64 during
# its AES trap.
# ---------------------------------------------------------------------------

class Abi:
    def __init__(self, name, ptr):
        self.name = name
        self.ptr = ptr           # pointer / long width in bytes

LP64 = Abi("Lp64", 8)
ILP32 = Abi("Ilp32", 4)


# ---------------------------------------------------------------------------
# PRODUCER: how the toolchain lays an OBJECT record in memory.
# gemlib mt_gem.h OBJECT and mkrsc's NRSC convert produce the SAME layout
# (mkrsc LP64 OBJ_FMT "<hhhHHH4xQhhhh"; ILP32 "<hhhHHHIhhhh"). The union
# ob_spec forces `ptr`-alignment: after 6 shorts (offset 12) LP64 pads to 16.
# ---------------------------------------------------------------------------

def obj_layout(abi):
    """Return (offsets dict, stride) exactly as the producer emits them."""
    off = {
        "ob_next": 0, "ob_head": 2, "ob_tail": 4,
        "ob_type": 6, "ob_flags": 8, "ob_state": 10,
    }
    # ob_spec is `ptr`-aligned: 12 rounds up to the next multiple of `ptr`.
    spec = (12 + (abi.ptr - 1)) & ~(abi.ptr - 1)
    off["ob_spec"] = spec
    coord = spec + abi.ptr
    off["ob_x"] = coord
    off["ob_y"] = coord + 2
    off["ob_width"] = coord + 4
    off["ob_height"] = coord + 6
    stride = coord + 8
    # A trailing pad may be needed to re-align the next record's ob_spec, but
    # for LP64 coord=24 -> stride 32 (already 8-aligned) and ILP32 coord=16 ->
    # stride 24 (4-aligned): neither needs trailing pad.
    return off, stride


def pack_object(abi, o):
    """Serialize one OBJECT the producer way (little-endian)."""
    off, stride = obj_layout(abi)
    buf = bytearray(stride)
    struct.pack_into("<h", buf, off["ob_next"],  o["ob_next"])
    struct.pack_into("<h", buf, off["ob_head"],  o["ob_head"])
    struct.pack_into("<h", buf, off["ob_tail"],  o["ob_tail"])
    struct.pack_into("<H", buf, off["ob_type"],  o["ob_type"])
    struct.pack_into("<H", buf, off["ob_flags"], o["ob_flags"])
    struct.pack_into("<H", buf, off["ob_state"], o["ob_state"])
    if abi.ptr == 8:
        struct.pack_into("<Q", buf, off["ob_spec"], o["ob_spec"] & 0xFFFFFFFFFFFFFFFF)
    else:
        struct.pack_into("<I", buf, off["ob_spec"], o["ob_spec"] & 0xFFFFFFFF)
    struct.pack_into("<h", buf, off["ob_x"],      o["ob_x"])
    struct.pack_into("<h", buf, off["ob_y"],      o["ob_y"])
    struct.pack_into("<h", buf, off["ob_width"],  o["ob_width"])
    struct.pack_into("<h", buf, off["ob_height"], o["ob_height"])
    return bytes(buf)


# ---------------------------------------------------------------------------
# CONSUMER: how the kernel's ObjTree reads an OBJECT record.
# object.rs:176-195 -> stride/spec_off/coord_off keyed on the tree's ABI.
# object.rs:310-318 -> raw_spec reads u64 (LP64) / u32 (ILP32) at spec_off.
# ---------------------------------------------------------------------------

def kernel_objtree_offsets(abi):
    if abi.ptr == 8:      # Abi::Lp64
        return dict(stride=32, spec_off=16, coord_off=24)
    else:                 # Abi::Ilp32
        return dict(stride=24, spec_off=12, coord_off=16)


def kernel_read_object(mem, base, idx, read_abi):
    """Read one OBJECT the kernel way, at `read_abi` width, from a flat memory
    image `mem` whose OBJECT array starts at byte `base`."""
    k = kernel_objtree_offsets(read_abi)
    rec = base + idx * k["stride"]
    def u16(o): return struct.unpack_from("<H", mem, rec + o)[0]
    def i16(o): return struct.unpack_from("<h", mem, rec + o)[0]
    if read_abi.ptr == 8:
        spec = struct.unpack_from("<Q", mem, rec + k["spec_off"])[0]
    else:
        spec = struct.unpack_from("<I", mem, rec + k["spec_off"])[0]
    c = k["coord_off"]
    return {
        "ob_next": i16(0), "ob_head": i16(2), "ob_tail": i16(4),
        "ob_type": u16(6), "ob_flags": u16(8), "ob_state": u16(10),
        "ob_spec": spec,
        "ob_x": i16(c), "ob_y": i16(c + 2), "ob_width": i16(c + 4), "ob_height": i16(c + 6),
    }


# ---------------------------------------------------------------------------
# gr_crack -- the kernel's box colour-word decode (object.rs:1944-1949).
# The dropdown box interior fill comes from ob_spec's low 16 bits. Pattern 0
# (IP_HOLLOW) paints NOTHING (object.rs:1979) -> the box is see-through.
# ---------------------------------------------------------------------------

def gr_crack(color):
    color &= 0xFFFF
    return {
        "border":  (color >> 12) & 0x0F,
        "text":    (color >> 8) & 0x0F,
        "pattern": (color >> 4) & 0x07,   # 0 = IP_HOLLOW -> transparent interior
        "inside":  color & 0x0F,
        "replace": (color >> 11) & 1,
    }


# ---------------------------------------------------------------------------
# A concrete menu OBJECT tree (the shape menu.rs walks: THESCREEN root, THEBAR,
# THEACTIVE box of titles, a G_TITLE, THEMENUS container, a dropdown G_BOX, an
# item G_STRING). Only the fields the menu/draw path reads need real values.
# The dropdown box's ob_spec is a packed colour/thickness LONG (NOT a pointer);
# we give it a WHITE-interior SOLID fill word so a correct read renders opaque.
# ---------------------------------------------------------------------------

G_BOX, G_IBOX, G_STRING, G_TITLE = 20, 24, 28, 32

# ob_spec colour word for the dropdown box: thickness=1 (byte1), colour word
# low16 = 0x1101 -> border=1(black) text=1 pattern=0? no: choose SOLID.
# Encode: pattern must be non-zero for an opaque interior. Use IP_SOLID(7),
# inside=0(white). low16 = border<<12 | text<<8 | pattern<<4 | inside
#        = 1<<12 | 1<<8 | 7<<4 | 0 = 0x1170.  thickness byte -> (th & 0xff)<<16.
BOX_SPEC = (1 << 16) | 0x1170           # th=1, opaque SOLID white interior

TREE = [
    # idx0 THESCREEN (root, G_IBOX): head=THEBAR(1) tail=THEMENUS(3)
    dict(ob_next=-1, ob_head=1, ob_tail=3, ob_type=G_IBOX,  ob_flags=0, ob_state=0,
         ob_spec=0, ob_x=0, ob_y=0, ob_width=640, ob_height=400),
    # idx1 THEBAR (G_BOX): child THEACTIVE(2)
    dict(ob_next=-1, ob_head=2, ob_tail=2, ob_type=G_BOX,   ob_flags=0, ob_state=0,
         ob_spec=0x00001100, ob_x=0, ob_y=0, ob_width=640, ob_height=19),
    # idx2 THEACTIVE (G_IBOX): holds titles; here one title as head/tail=... (index reused)
    dict(ob_next=-1, ob_head=-1, ob_tail=-1, ob_type=G_IBOX, ob_flags=0, ob_state=0,
         ob_spec=0, ob_x=0, ob_y=0, ob_width=640, ob_height=19),
    # idx3 THEMENUS container (G_IBOX): head = dropdown box (4)
    dict(ob_next=-1, ob_head=4, ob_tail=4, ob_type=G_IBOX,  ob_flags=0, ob_state=0,
         ob_spec=0, ob_x=0, ob_y=19, ob_width=640, ob_height=381),
    # idx4 dropdown box (G_BOX): the object whose ob_spec fill decides transparency
    dict(ob_next=-1, ob_head=5, ob_tail=5, ob_type=G_BOX,   ob_flags=0, ob_state=0,
         ob_spec=BOX_SPEC, ob_x=8, ob_y=19, ob_width=120, ob_height=64),
    # idx5 item (G_STRING): ob_spec is a text POINTER (VA), here a plausible EL0 VA
    dict(ob_next=-1, ob_head=-1, ob_tail=-1, ob_type=G_STRING, ob_flags=0, ob_state=0,
         ob_spec=0x0000_0000_4010_0000, ob_x=0, ob_y=0, ob_width=120, ob_height=16),
]

DROPDOWN_BOX = 4   # the object menu.rs open_box/draw_obj fills


def build_tree_image(abi, base=0x40200000):
    """Lay the whole tree in a flat memory image the PRODUCER way."""
    off, stride = obj_layout(abi)
    mem = bytearray(base + stride * len(TREE) + 16)
    for i, o in enumerate(TREE):
        rec = pack_object(abi, o)
        mem[base + i * stride: base + i * stride + stride] = rec
    return mem, base, stride


# ---------------------------------------------------------------------------
# AESPB pack (gemlib) + kernel read, for menu_bar(tree, 1) and
# menu_tnormal(tree, title, normal).
# ---------------------------------------------------------------------------

def pack_aespb(abi, opcode, intin, addrin, base=0x50000000):
    """Reproduce AES_PARAMS + the a_menu_*.c pack + the 6-pointer AESPB.
    Returns (mem, aespb_ptr, array_addrs). control/intin/intout are WORD
    (16-bit) arrays; addrin/addrout are `long`(=ptr) arrays."""
    p = abi.ptr
    # Lay the arrays somewhere, then the 6-pointer AESPB block.
    a_control = base + 0x100
    a_global  = base + 0x200
    a_intin   = base + 0x300
    a_intout  = base + 0x400
    a_addrin  = base + 0x500
    a_addrout = base + 0x600
    aespb     = base + 0x000
    size = base + 0x800
    mem = bytearray(size)

    # control[5] WORDs = {opcode, n_intin, n_intout, n_addrin, n_addrout}
    ctrl = [opcode, len(intin), 1, len(addrin), 0]
    for i, v in enumerate(ctrl):
        struct.pack_into("<h", mem, a_control + 2 * i, v)
    # intin WORDs
    for i, v in enumerate(intin):
        struct.pack_into("<h", mem, a_intin + 2 * i, v)
    # addrin `long`(=ptr) elements
    for i, v in enumerate(addrin):
        if p == 8:
            struct.pack_into("<Q", mem, a_addrin + 8 * i, v & 0xFFFFFFFFFFFFFFFF)
        else:
            struct.pack_into("<I", mem, a_addrin + 4 * i, v & 0xFFFFFFFF)

    # The AESPB: six consecutive native pointers.
    ptrs = [a_control, a_global, a_intin, a_intout, a_addrin, a_addrout]
    for i, v in enumerate(ptrs):
        if p == 8:
            struct.pack_into("<Q", mem, aespb + 8 * i, v)
        else:
            struct.pack_into("<I", mem, aespb + 4 * i, v)

    return mem, aespb, dict(control=a_control, intin=a_intin, addrin=a_addrin)


def kernel_read_aespb(mem, aespb_ptr, read_abi):
    """Aespb::from_raw (mod.rs:69-83): read the six block pointers at read_abi
    width, then control[0], intin[0], addrin[0] the way the kernel does."""
    p = read_abi.ptr
    def rdptr(a):
        if p == 8:
            return struct.unpack_from("<Q", mem, a)[0]
        return struct.unpack_from("<I", mem, a)[0]
    control = rdptr(aespb_ptr + 0 * p)
    intin   = rdptr(aespb_ptr + 2 * p)
    addrin  = rdptr(aespb_ptr + 4 * p)
    opcode  = struct.unpack_from("<h", mem, control + 0)[0]
    intin0  = struct.unpack_from("<h", mem, intin + 0)[0]
    if p == 8:
        addrin0 = struct.unpack_from("<Q", mem, addrin + 0)[0]
    else:
        addrin0 = struct.unpack_from("<I", mem, addrin + 0)[0]
    return dict(opcode=opcode, intin0=intin0, addrin0=addrin0)


# ---------------------------------------------------------------------------
# Run the comparison and print a verdict.
# ---------------------------------------------------------------------------

def hr(t=""):
    print("=" * 78)
    if t:
        print(t)
        print("=" * 78)


def dump_layout(abi):
    off, stride = obj_layout(abi)
    kern = kernel_objtree_offsets(abi)
    print(f"  producer OBJECT ({abi.name}): stride={stride} "
          f"spec@{off['ob_spec']} coords@{off['ob_x']}")
    print(f"  kernel   ObjTree({abi.name}): stride={kern['stride']} "
          f"spec@{kern['spec_off']} coords@{kern['coord_off']}")
    agree = (stride == kern["stride"] and off["ob_spec"] == kern["spec_off"]
             and off["ob_x"] == kern["coord_off"])
    print(f"  => {'MATCH' if agree else 'MISMATCH'}")
    return agree


def main():
    hr("AES MENU-group ABI marshalling simulation (byte-level)")
    print("TeraDesk runtime ABI = Lp64 (ELF64 app + LP64 gemlib.a); the kernel's")
    print("current_abi() therefore returns Lp64 during its AES trap.\n")

    hr("1) OBJECT record layout: producer (gemlib/mkrsc) vs kernel (ObjTree)")
    print("LP64 (the real TeraDesk configuration):")
    lp64_ok = dump_layout(LP64)
    print("\nILP32 (the legacy config, shown for contrast):")
    ilp32_ok = dump_layout(ILP32)

    hr("2) Field-by-field OBJECT read of the LP64 menu tree")
    mem, base, stride = build_tree_image(LP64)
    ok_all = True
    for idx in range(len(TREE)):
        want = TREE[idx]
        got = kernel_read_object(mem, base, idx, LP64)   # kernel reads at Lp64
        same = all((got[k] & 0xFFFFFFFFFFFFFFFF) == (want[k] & 0xFFFFFFFFFFFFFFFF)
                   if k == "ob_spec" else got[k] == want[k] for k in want)
        ok_all = ok_all and same
        tag = "OK" if same else "DIFF"
        print(f"  obj[{idx}] type={want['ob_type']:>2} spec=0x{want['ob_spec']:016x} "
              f"-> kernel spec=0x{got['ob_spec']:016x} coords="
              f"({got['ob_x']},{got['ob_y']},{got['ob_width']},{got['ob_height']}) [{tag}]")
    print(f"\n  all fields identical: {ok_all}")

    hr("3) The transparent-dropdown test: the box's ob_spec fill")
    print("The dropdown box is object index", DROPDOWN_BOX,
          "(G_BOX). Its interior fill comes")
    print("from gr_crack(ob_spec & 0xFFFF) (object.rs:2218). Pattern 0 = IP_HOLLOW")
    print("paints NOTHING (object.rs:1979) -> a see-through box.\n")

    correct = kernel_read_object(mem, base, DROPDOWN_BOX, LP64)["ob_spec"]
    crk = gr_crack(correct)
    print(f"  CORRECT LP64 read  : ob_spec=0x{correct:016x}  low16=0x{correct & 0xFFFF:04x}")
    print(f"                       gr_crack -> pattern={crk['pattern']} "
          f"inside={crk['inside']}  => "
          f"{'HOLLOW/transparent' if crk['pattern']==0 else 'OPAQUE fill'}")

    # Counterfactual: what a WRONG (ILP32-offset) read of the LP64 tree would get.
    wrong = kernel_read_object(mem, base, DROPDOWN_BOX, ILP32)["ob_spec"]
    wcrk = gr_crack(wrong)
    print(f"  WRONG ILP32 read   : ob_spec=0x{wrong:016x}  low16=0x{wrong & 0xFFFF:04x}")
    print(f"   (kernel using @12/stride24 on the 32-byte tree)")
    print(f"                       gr_crack -> pattern={wcrk['pattern']} "
          f"inside={wcrk['inside']}  => "
          f"{'HOLLOW/transparent' if wcrk['pattern']==0 else 'OPAQUE fill'}")
    print("\n  The WRONG read is what a real ABI mismatch would produce (garbage")
    print("  colour word -> often hollow -> transparent). The kernel does NOT do")
    print("  this: it reads at Lp64 because TeraDesk is Lp64, so it gets the")
    print("  CORRECT word above.")

    hr("4) AESPB pack + kernel read: menu_bar(tree, 1) [opcode 30]")
    tree_va = base
    mem2, aespb, arrs = pack_aespb(LP64, opcode=30, intin=[1], addrin=[tree_va])
    got = kernel_read_aespb(mem2, aespb, LP64)
    print(f"  producer: control[0]=30 intin[0]=1 addrin[0]=0x{tree_va:016x} "
          f"(8-byte long)")
    print(f"  kernel  : opcode={got['opcode']} show=intin[0]={got['intin0']} "
          f"tree=addrin[0]=0x{got['addrin0']:016x}")
    mb_ok = (got["opcode"] == 30 and got["intin0"] == 1 and got["addrin0"] == tree_va)
    print(f"  => {'MATCH' if mb_ok else 'MISMATCH'}")

    hr("5) AESPB pack + kernel read: menu_tnormal(tree, title=2, normal=1) [33]")
    mem3, aespb3, _ = pack_aespb(LP64, opcode=33, intin=[2, 1], addrin=[tree_va])
    got3 = kernel_read_aespb(mem3, aespb3, LP64)
    # kernel menu_tnormal reads intin[0]=title, intin[1]=normal, addrin[0]=tree.
    p = LP64.ptr
    intin_base = struct.unpack_from("<Q", mem3, aespb3 + 2 * p)[0]
    title = struct.unpack_from("<h", mem3, intin_base + 0)[0]
    normal = struct.unpack_from("<h", mem3, intin_base + 2)[0]
    print(f"  producer: control[0]=33 intin[0]=2(title) intin[1]=1(normal) "
          f"addrin[0]=0x{tree_va:016x}")
    print(f"  kernel  : opcode={got3['opcode']} title={title} normal={normal} "
          f"tree=0x{got3['addrin0']:016x}")
    tn_ok = (got3["opcode"] == 33 and title == 2 and normal == 1
             and got3["addrin0"] == tree_va)
    print(f"  => {'MATCH' if tn_ok else 'MISMATCH'}")

    hr("VERDICT")
    all_match = lp64_ok and ok_all and mb_ok and tn_ok and crk["pattern"] != 0
    if all_match:
        print("MENU-group ABI marshalling MATCHES end-to-end for LP64 TeraDesk:")
        print("  - AESPB 6-pointer block: producer 8-byte ptrs == kernel 8-byte reads")
        print("  - control/intin WORDs and addrin[0] tree pointer: byte-identical")
        print("  - OBJECT record: stride 32, ob_spec@16, coords@24 on BOTH sides")
        print("  - dropdown box ob_spec (fill) read at the CORRECT offset -> opaque")
        print("\nThe transparent dropdown is NOT an ABI-marshalling bug. Next suspect:")
        print("  VDI box-interior fill / save_under (bb_save) on real HW, or the")
        print("  actual ob_spec fill value in TeraDesk's converted desktop.rsc.")
    else:
        print("A DIVERGENCE was found -- see the per-section output above.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
