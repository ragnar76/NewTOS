#!/usr/bin/env python3
"""DESK46-TEMP probe 2 (scratch copy only) -- two observations, no changed
behaviour:

1) `unhandled` dumps x0..x29, LR and SP_EL0 before halting.  Without it the
   "[EXC] ... FAR=0" line says that something read address 0 but not with
   which register file.

2) `finish_terminate` reports, for every child that terminates, which restore
   arm it takes (`from_el0` / `full`) and, for three witness registers
   (x19, x29, LR), the value the *child* left in the trap frame next to the
   value the *parent* had stored at its `Pexec`.  If those differ and the
   restore arm is neither, the parent resumes on the child's register file --
   which is a statement about the code, made by the machine rather than by
   reading it.

Applied to the scratch copy after desk46_patch.py.  Never to the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b, n=1):
    s = open(path).read()
    assert s.count(a) == n, (path, s.count(a), a[:70])
    open(path, 'w').write(s.replace(a, b, n))
    print("patched", path)


p = scr + '/kernel/src/arch/aarch64/exceptions.rs'

# 1) full register dump before the halt
rep(p,
    "    if let Some(addr) = far {\n        crate::kprintln!(\"[EXC] FAR=0x{:016x}\", addr);\n    }\n    crate::arch::aarch64::halt();",
    "    if let Some(addr) = far {\n        crate::kprintln!(\"[EXC] FAR=0x{:016x}\", addr);\n    }\n"
    "    // DESK46-TEMP: the whole register file, four per line.\n"
    "    for i in (0..28).step_by(4) {\n"
    "        crate::kprintln!(\n"
    "            \"[EXC] x{:<2}=0x{:016x} x{:<2}=0x{:016x} x{:<2}=0x{:016x} x{:<2}=0x{:016x}\",\n"
    "            i, frame.gpr[i], i + 1, frame.gpr[i + 1],\n"
    "            i + 2, frame.gpr[i + 2], i + 3, frame.gpr[i + 3],\n"
    "        );\n"
    "    }\n"
    "    crate::kprintln!(\n"
"        \"[EXC] x28=0x{:016x} x29=0x{:016x} lr=0x{:016x} sp_el0=0x{:016x}\",\n"
"        frame.gpr[28], frame.gpr[29], frame.lr, frame.sp_el0,\n"
"    );\n"
    "    crate::arch::aarch64::halt();")

# 2) which restore arm `finish_terminate` takes, and the three witnesses
rep(p,
    "    let pexec_caller = tos_api::bdos::process::PROCESS_TABLE\n"
    "        [resume.parent_frame_slot]\n"
    "        .parent_frame_pexec\n"
    "        .swap(false, Ordering::AcqRel);\n",
    "    let pexec_caller = tos_api::bdos::process::PROCESS_TABLE\n"
    "        [resume.parent_frame_slot]\n"
    "        .parent_frame_pexec\n"
    "        .swap(false, Ordering::AcqRel);\n"
    "    // DESK46-TEMP\n"
    "    crate::kprintln!(\n"
    "        \"[d46] finish_terminate: slot={} pexec_caller={} full={} pf.spsr=0x{:x} rc={}\",\n"
    "        resume.parent_frame_slot, i32::from(pexec_caller), i32::from(full), pf_spsr, rc,\n"
    "    );\n"
    "    crate::kprintln!(\n"
    "        \"[d46]   child frame : x19=0x{:016x} x29=0x{:016x} lr=0x{:016x} elr=0x{:016x}\",\n"
    "        frame.gpr[19], frame.gpr[29], frame.lr, frame.elr,\n"
    "    );\n"
    "    crate::kprintln!(\n"
    "        \"[d46]   parent saved: x19=0x{:016x} x29=0x{:016x} lr=0x{:016x} elr=0x{:016x}\",\n"
    "        pf.gpr(19).load(Ordering::Acquire), pf.gpr(29).load(Ordering::Acquire),\n"
    "        pf.lr().load(Ordering::Acquire), pf.elr().load(Ordering::Acquire),\n"
    "    );\n")
