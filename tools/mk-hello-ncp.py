#!/usr/bin/env python3
# Generate HELLO.NCP — a minimal native NewTOS AArch64 ELF **console** program
# (ADR-018 `.NCP`). It prints a banner via GEMDOS Cconws (0x09) and terminates
# via Pterm (0x4C, rc=0). The byte layout mirrors the hand-crafted child ELF in
# `kernel/src/selftest_pexec.rs` (every AArch64 instruction encoding is reused
# from there, already proven through the Pexec round-trip), so it satisfies the
# NewTOS ELF loader (`tos-api/src/bdos/process/{elf,loader}.rs`):
#
#   * ELFCLASS64, ELFDATA2LSB, ET_DYN (a PIE -- the loader refuses ET_EXEC
#     since ARCH03 stage 0d), EM_AARCH64 (0xB7), e_version=1, e_flags=0
#   * position-independent code (the banner is addressed PC-relative), no
#     dynamic section: there is nothing to relocate
#   * one PT_LOAD segment, e_phentsize=56
#   * segment + entry at VA 0x1000 (B1 / Option 1: the first page of the TPA
#     is reserved for the BasePage @0 + classic sysvars/OSHEADER/cookie-jar,
#     so native segments start at >= 0x1000, matching sdk/newtos.ld)
#   * whole image fits within the TPA
#
# Usage: tools/mk-hello-ncp.py [out-path]   (default: /tmp/HELLO.NCP)

import struct
import sys

CODE_OFF = 0x1000         # entry VA / file offset of the code (B1 base)
STR_OFF = 0x1100          # VA / file offset of the banner string
BANNER = b"Hello from a NewTOS .NCP program!\r\n\x00"

# Code at 0x1000 (7 instructions, 28 bytes). Little-endian AArch64.
#   adr  x0, .+0x100  ; x0 = &banner (STR_OFF)   (00 08 00 10)
#   movz x8, #0x09    ; Cconws                   (28 01 80 D2)
#   svc  #1                                       (21 00 00 D4)
#   movz x0, #0       ; rc = 0                    (00 00 80 D2)
#   movz x8, #0x4C    ; Pterm                     (88 09 80 D2)
#   svc  #1                                        (21 00 00 D4)
#   b    .            ; safety net                (00 00 00 14)
CODE = bytes([
    0x00, 0x08, 0x00, 0x10,
    0x28, 0x01, 0x80, 0xD2,
    0x21, 0x00, 0x00, 0xD4,
    0x00, 0x00, 0x80, 0xD2,
    0x88, 0x09, 0x80, 0xD2,
    0x21, 0x00, 0x00, 0xD4,
    0x00, 0x00, 0x00, 0x14,
])

FILE_END = STR_OFF + len(BANNER)   # 0x224 for a 36-byte banner
SEG_SIZE = FILE_END - CODE_OFF     # p_filesz = p_memsz

# ELF64 header (64 bytes).
ehdr = struct.pack(
    "<4sBBBBB7xHHIQQQIHHHHHH",
    b"\x7fELF",   # e_ident magic
    2,            # EI_CLASS  = ELFCLASS64
    1,            # EI_DATA   = ELFDATA2LSB
    1,            # EI_VERSION
    0,            # EI_OSABI
    0,            # EI_ABIVERSION
    3,            # e_type    = ET_DYN
    0xB7,         # e_machine = EM_AARCH64
    1,            # e_version
    CODE_OFF,     # e_entry   = 0x1000
    0x40,         # e_phoff   = 64
    0,            # e_shoff
    0,            # e_flags
    64,           # e_ehsize
    56,           # e_phentsize
    1,            # e_phnum
    64,           # e_shentsize
    0,            # e_shnum
    0,            # e_shstrndx
)
assert len(ehdr) == 64, len(ehdr)

# One PT_LOAD program header (56 bytes): p_flags = PF_R | PF_X = 5.
phdr = struct.pack(
    "<IIQQQQQQ",
    1,            # p_type   = PT_LOAD
    5,            # p_flags  = R+X
    CODE_OFF,     # p_offset = 0x1000
    CODE_OFF,     # p_vaddr  = 0x1000
    CODE_OFF,     # p_paddr  = 0x1000
    SEG_SIZE,     # p_filesz
    SEG_SIZE,     # p_memsz
    0x1000,       # p_align
)
assert len(phdr) == 56, len(phdr)

img = bytearray(FILE_END)
img[0:64] = ehdr
img[0x40:0x40 + 56] = phdr
img[CODE_OFF:CODE_OFF + len(CODE)] = CODE
img[STR_OFF:STR_OFF + len(BANNER)] = BANNER

# Self-check the fields the NewTOS ELF parser validates, so a bad edit fails
# here rather than as a silent load error on-device.
assert img[:4] == b"\x7fELF"
assert img[4] == 2 and img[5] == 1 and img[6] == 1          # class/data/version
assert struct.unpack_from("<H", img, 16)[0] == 3            # ET_DYN
assert struct.unpack_from("<H", img, 18)[0] == 0xB7         # EM_AARCH64
assert struct.unpack_from("<Q", img, 24)[0] == CODE_OFF     # e_entry
assert struct.unpack_from("<H", img, 54)[0] == 56           # e_phentsize

out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/HELLO.NCP"
with open(out, "wb") as f:
    f.write(img)
print(f"wrote {out}: {len(img)} bytes, entry=0x{CODE_OFF:x}, "
      f"filesz=0x{SEG_SIZE:x}, banner={len(BANNER)} bytes")
