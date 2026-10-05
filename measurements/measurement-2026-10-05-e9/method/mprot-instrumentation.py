#!/usr/bin/env python3
"""mprot-instrumentation.py <scratch-tree> <bin-dir>

Applied AFTER apply-instrumentation.py (0c) and uacc-instrumentation.py to
the same scratch copy (never to the shared tree): on board-qemu-virt (no SD
card) the stage-3b memory-protection probe joins the RAM program registry --
C:\\MPKID.NAP and the driver variant that matches the boot argument
(protection on: MPROT3B, memprot=strict: MPROTS, memprot=off: MPROTO) --
and is launched from the boot path right after UACC3A. The crash prompts are
answered over the serial line by drive.py (--answer).
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
progs = pathlib.Path(sys.argv[2]).resolve()
lib = t / "kernel/src/lib.rs"
s = lib.read_text()
old = '''        kprintln!("[u3a] back in the boot path after UACC3A, launch rc={}", rc);
'''
assert s.count(old) == 1, "lib.rs UACC3A anchor"
s = s.replace(old, old + f'''        // M3B-TEMP: the stage-3b memory-protection probe
        static MPKID: &[u8] = include_bytes!("{progs}/MPKID.NAP");
        static MPROT3B: &[u8] = include_bytes!("{progs}/MPROT3B.NAP");
        static MPROTS: &[u8] = include_bytes!("{progs}/MPROTS.NAP");
        static MPROTO: &[u8] = include_bytes!("{progs}/MPROTO.NAP");
        tos_api::bdos::process::registry::register(b"C:\\\\MPKID.NAP", MPKID);
        let drv = if !tos_api::bdos::process::memprot::enabled() {{ MPROTO }}
            else if tos_api::bdos::process::memprot::strict() {{ MPROTS }} else {{ MPROT3B }};
        tos_api::bdos::process::registry::register(b"C:\\\\MPROT.NAP", drv);
        let rc = launch::run_program_with_tail_rc(b"C:\\\\MPROT.NAP", &[]);
        kprintln!("[m3b] back in the boot path after MPROT, launch rc={{}}", rc);
''')
lib.write_text(s)
print("mprot instrumentation applied to", t)
