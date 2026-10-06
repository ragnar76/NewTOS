#!/usr/bin/env python3
"""e10-instrumentation.py <scratch-tree> <bin-dir>

Applied AFTER the crash instrumentation (0c apply-instrumentation.py,
uacc-instrumentation.py, mprot-instrumentation.py) to the same scratch copy,
never to the shared tree: on board-qemu-virt (no SD card, no #Z autorun) the
stage-3c E10 probe joins the RAM program registry -- C:\\E10KID.NAP and the
driver variant matching the boot argument (on: E10USR, memprot=strict:
E10USRS, memprot=off: E10USRO) -- and is launched ONCE from the AES session
loop right after `go_graphic` (the AES is up, the screen is graphic: the
probe is a GEM program), before the desktop. The crash prompt of the strict
'r' case is answered over the serial line by drive.py (--answer).
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
progs = pathlib.Path(sys.argv[2]).resolve()
lib = t / "kernel/src/lib.rs"
s = lib.read_text()
old = '''        shell_loop::go_graphic();
'''
assert s.count(old) == 1, "lib.rs go_graphic anchor"
s = s.replace(old, old + f'''        // E10-TEMP: the stage-3c E10 probe (a GEM program), once per boot
        #[cfg(feature = "board-qemu-virt")]
        {{
            static E10_ONCE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
            if !E10_ONCE.swap(true, core::sync::atomic::Ordering::Relaxed) {{
                static E10KID: &[u8] = include_bytes!("{progs}/E10KID.NAP");
                static E10USR: &[u8] = include_bytes!("{progs}/E10USR.NAP");
                static E10USRS: &[u8] = include_bytes!("{progs}/E10USRS.NAP");
                static E10USRO: &[u8] = include_bytes!("{progs}/E10USRO.NAP");
                tos_api::bdos::process::registry::register(b"C:\\\\E10KID.NAP", E10KID);
                let drv = if !tos_api::bdos::process::memprot::enabled() {{ E10USRO }}
                    else if tos_api::bdos::process::memprot::strict() {{ E10USRS }} else {{ E10USR }};
                tos_api::bdos::process::registry::register(b"C:\\\\E10USR.NAP", drv);
                let rc = launch::run_program_with_tail_rc(b"C:\\\\E10USR.NAP", &[]);
                kprintln!("[e10] back in the session loop after E10USR, launch rc={{}}", rc);
            }}
        }}
''')
lib.write_text(s)
print("e10 instrumentation applied to", t)
