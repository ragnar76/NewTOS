#!/usr/bin/env python3
"""mdv-instrumentation.py <scratch-tree> <bin-dir> <launch-name> [extra-name...]

MEM05 acceptance, applied to a SCRATCH copy only (make-scratch.sh), never to the shared tree. Like the
0c instrumentation (inj-0c-apply-instrumentation.py): on board-qemu-virt (no SD card) the named
programs from <bin-dir> (e.g. MDSTEP.NAP, MDPAR.NAP + MDKID.NAP) join the RAM program registry as
C:\\<NAME> and <launch-name> is launched once from the boot path right after the AUTO folder.
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1]); progs = pathlib.Path(sys.argv[2]).resolve()
launch_name = sys.argv[3]; extra = sys.argv[4:]
lib = t / "kernel/src/lib.rs"; s = lib.read_text()
old = "    launch::run_auto_folder();\n"
assert s.count(old) == 1, "lib.rs auto-folder anchor"
names = [launch_name] + extra
decl = "".join(f'        static P{i}: &[u8] = include_bytes!("{progs}/{n}");\n' for i, n in enumerate(names))
reg = "".join(f'        tos_api::bdos::process::registry::register(b"C:\\\\{n}", P{i});\n' for i, n in enumerate(names))
s = s.replace(old, old + f"""
    // MDV-TEMP BEGIN -- MEM05 probes from the RAM registry (no disk on virt)
    #[cfg(feature = "board-qemu-virt")]
    {{
{decl}{reg}        kprintln!("[mdv] launch C:\\\\{launch_name}");
        let rc = launch::run_program_with_tail_rc(b"C:\\\\{launch_name}", &[]);
        kprintln!("[mdv] back in the boot path after {launch_name}, launch rc={{}}", rc);
    }}
    // MDV-TEMP END
""")
lib.write_text(s)
print("mdv instrumentation applied to", t)
