"""virtreg-instrumentation.py <scratch-tree> <bin-dir> -- scratch-only: on
board-qemu-virt (no SD card) put the libc probes into the RAM program registry
and run them one after another from the boot path at the AUTO folder's
position (same recipe as inj-0c-apply-instrumentation.py lib.rs part). Never
applied to the shared tree."""
import sys, pathlib
t = pathlib.Path(sys.argv[1]); progs = pathlib.Path(sys.argv[2]).resolve()
import os
names = os.environ.get("VREG_NAMES", "STDOUTDB.NAP,STDERRP.NAP,FS11A.NAP,APITEST.NCP").split(",")
lib = t / "kernel/src/lib.rs"
s = lib.read_text()
old = "    launch::run_auto_folder();\n"
assert s.count(old) == 1
blk = "\n    // VREG-TEMP BEGIN\n    #[cfg(feature = \"board-qemu-virt\")]\n    {\n"
for n in names:
    v = n.split(".")[0]
    blk += f'        static {v}: &[u8] = include_bytes!("{progs}/{n}");\n'
    blk += f'        tos_api::bdos::process::registry::register(b"C:\\\\{n}", {v});\n'
for n in names:
    blk += f'        kprintln!("[vreg] launch C:\\\\{n}");\n'
    blk += f'        let rc = launch::run_program_with_tail_rc(b"C:\\\\{n}", &[]);\n'
    blk += f'        kprintln!("[vreg] back after {n}, rc={{}}", rc);\n'
blk += "    }\n    // VREG-TEMP END\n"
lib.write_text(s.replace(old, old + blk))
print("instrumented", t)
