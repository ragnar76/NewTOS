#!/usr/bin/env python3
"""A85-TEMP (scratch copy only): draw the file selector on C:\\*.* without the
modal loop (`tos_api::aes::fsel::preview`, the A63/A64 showcase) right before
the shell loop, print a serial marker and halt -- the fsel guard dump."""
import sys
p = sys.argv[1] + '/kernel/src/lib.rs'; s = open(p).read()
a = "    shell_loop::sh_main(isauto, isgem);\n"
assert s.count(a) == 1
s = s.replace(a, "    // A85-TEMP (scratch copy only): fsel guard dump.\n"
                 "    tos_api::aes::fsel::preview();\n"
                 "    kprintln!(\"[a85] MARK FSEL_PREVIEW\");\n"
                 "    arch::aarch64::halt();\n"
                 "    #[allow(unreachable_code)]\n" + a, 1)
open(p, 'w').write(s); print("patched", p)
