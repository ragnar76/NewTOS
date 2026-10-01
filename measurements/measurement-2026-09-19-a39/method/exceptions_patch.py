#!/usr/bin/env python3
import sys
ws = sys.argv[1]

def patch(path, pairs):
    s = open(path).read()
    for a, b in pairs:
        assert s.count(a) == 1, (path, a[:80])
        s = s.replace(a, b, 1)
    open(path, 'w').write(s)
    print("patched", path)

p = ws + '/kernel/src/arch/aarch64/exceptions.rs'
patch(p, [
    ("    let path_bytes = &path_buf[..path_len];\n",
     "    let path_bytes = &path_buf[..path_len];\n"
     "    // H05-TEMP\n"
     "    {\n"
     "        let cur = tos_api::bdos::process::current();\n"
     "        let cs = tos_api::bdos::process::slot_for(cur);\n"
     "        crate::kprintln!(\"[h05] Pexec mode={} from_el0={} caller_slot={:?} caller_bp=0x{:x} path='{}'\",\n"
     "            mode, from_el0, cs, cur as u64, core::str::from_utf8(path_bytes).unwrap_or(\"?\"));\n"
     "    }\n"),
    ("    if !child.launch {\n        frame.gpr[0] = child.handle;\n        return;\n    }\n",
     "    if !child.launch {\n        frame.gpr[0] = child.handle;\n        return;\n    }\n"
     "    // H05-TEMP\n"
     "    crate::kprintln!(\"[h05] Pexec -> child slot={} child_bp_kva=0x{:x} entry=0x{:x} parent_elr=0x{:x} parent_spsr=0x{:x}\",\n"
     "        child.slot, tos_api::bdos::process::pexec::child_basepage_kva(child.slot) as u64, child.entry_va, frame.elr, frame.spsr);\n"),
    ("    let pf = &tos_api::bdos::process::PROCESS_TABLE[resume.parent_frame_slot]\n        .parent_frame;\n    let pf_spsr = pf.spsr().load(Ordering::Acquire);\n    let from_el0 = (pf_spsr & 0xF) == 0;\n",
     "    let pf = &tos_api::bdos::process::PROCESS_TABLE[resume.parent_frame_slot]\n        .parent_frame;\n    let pf_spsr = pf.spsr().load(Ordering::Acquire);\n    let from_el0 = (pf_spsr & 0xF) == 0;\n"
     "    // H05-TEMP\n"
     "    {\n"
     "        let ps = tos_api::bdos::process::PROCESS_TABLE[resume.parent_frame_slot].parent_slot.load(Ordering::Acquire);\n"
     "        let run = tos_api::bdos::process::current();\n"
     "        crate::kprintln!(\"[h05] Pterm dying_slot={} rc={} resume_to_el0={} parent_slot={} parent_elr=0x{:x} RUN_after=0x{:x} run_slot={:?} parent_l0=0x{:x}\",\n"
     "            resume.parent_frame_slot, rc, from_el0, ps as i32, pf.elr().load(Ordering::Acquire), run as u64, tos_api::bdos::process::slot_for(run), resume.parent_l0_pa);\n"
     "    }\n"),
])
