#!/usr/bin/env python3
"""Assemble auto_test.s and wrap it in a NewTOS-loadable ELF (.PRG).

The NewTOS program loader maps a single 4-KiB user page: the first 256 bytes
hold the BasePage, code/entry start at vaddr 0x100. So we emit one ET_EXEC
ELF64 with a single PT_LOAD (RWX) at vaddr 0x100, p_offset 0x100, entry 0x100
— the exact shape the loader expects (matching the selftest child ELF).

Toolchain: aarch64-elf-as / -ld / -objcopy (Homebrew binutils).
"""
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
SRC = HERE / "auto_test.s"
OBJ = HERE / "auto_test.o"
ELF = HERE / "auto_test.elf"
BIN = HERE / "auto_test.bin"
PRG = HERE / "AUTOTEST.PRG"

VADDR = 0x100  # code/entry vaddr (after the 256-byte BasePage)


def run(*cmd):
    print("  " + " ".join(cmd))
    subprocess.run(cmd, check=True)


def main():
    # Assemble + link at 0x100 so PC-relative `adr` resolves for vaddr 0x100,
    # then strip to a raw image of the loaded bytes.
    run("aarch64-elf-as", "-o", str(OBJ), str(SRC))
    run("aarch64-elf-ld", f"-Ttext=0x{VADDR:x}", "-e", "_start",
        "-o", str(ELF), str(OBJ))
    run("aarch64-elf-objcopy", "-O", "binary", str(ELF), str(BIN))

    code = BIN.read_bytes()
    if len(code) > 0x1000 - VADDR:
        sys.exit(f"image too big: {len(code)} bytes > one 4-KiB page")

    # ── ELF64 header (64 bytes) ──
    e_ident = b"\x7fELF" + bytes([2, 1, 1, 0]) + b"\x00" * 8
    ehdr = e_ident + struct.pack(
        "<HHIQQQIHHHHHH",
        2,        # e_type    = ET_EXEC
        0xB7,     # e_machine = EM_AARCH64
        1,        # e_version
        VADDR,    # e_entry
        64,       # e_phoff   (phdr right after the ehdr)
        0,        # e_shoff
        0,        # e_flags
        64,       # e_ehsize
        56,       # e_phentsize
        1,        # e_phnum
        0, 0, 0,  # e_shentsize, e_shnum, e_shstrndx
    )
    # ── one PT_LOAD program header (56 bytes), RWX ──
    phdr = struct.pack(
        "<IIQQQQQQ",
        1,            # p_type  = PT_LOAD
        7,            # p_flags = R|W|X
        VADDR,        # p_offset (code lives at file offset 0x100)
        VADDR,        # p_vaddr
        VADDR,        # p_paddr
        len(code),    # p_filesz
        len(code),    # p_memsz
        0x1000,       # p_align
    )
    out = bytearray(ehdr + phdr)
    out += b"\x00" * (VADDR - len(out))  # pad to file offset 0x100
    out += code
    PRG.write_bytes(out)
    print(f"wrote {PRG.name}: {len(out)} bytes (code {len(code)} @ vaddr 0x{VADDR:x})")

    for tmp in (OBJ, ELF, BIN):
        tmp.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
