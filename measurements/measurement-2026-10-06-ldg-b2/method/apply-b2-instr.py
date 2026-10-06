#!/usr/bin/env python3
"""apply-b2-instr.py <scratch-tree>

Audit instrumentation for finding B2 (LDG module heap), applied ONLY to a
scratch rsync copy of the tree (never the shared tree). It adds serial
prints, no behaviour change:

  [b2] fn=0x48/0x44/0x49/0x4a ...  every Malloc / Mxalloc / Mfree / Mshrink
       with arguments, result, the running PD (`run`), ELR, LR and up to 8
       return addresses from the caller's AArch64 frame-pointer chain (read
       with the fault-safe unprivileged loads of tos_api::uaccess).
  [b2] Pexec mode=..               every Pexec entry (mode, x1, x2).
  [b2] Pterm run=..                every Pterm, with the dying module's
       _mchunk_free_list.next as read from both NLD layouts (POPUP.NLD:
       link VA 0x83d8, EX_LIB.NLD: 0x9278; runtime VA = basepage + link VA),
       and, after the sweep, for every block that PD got from Malloc/Mxalloc
       during its life: is it still allocated?
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
ex = t / "kernel/src/arch/aarch64/exceptions.rs"
s = ex.read_text()

# A) capture args before the result match
a = "    let result: i64 = match class {\n        TrapClass::Gemdos => tos_api::gemdos::dispatch(&args) as i64,\n"
assert s.count(a) == 1, "anchor A"
s = s.replace(a, "    let b2_gem = matches!(class, TrapClass::Gemdos); // B2-TEMP\n    let (b2_fn, b2_a0, b2_a1, b2_a2) = (frame.gpr[8] as u16, frame.gpr[0], frame.gpr[1], frame.gpr[2]); // B2-TEMP\n" + a)

# B) print after the match
b = "        TrapClass::Xbios => tos_api::xbios::dispatch(&args),\n    };\n"
assert s.count(b) == 1, "anchor B"
s = s.replace(b, b + "    if b2_gem && matches!(b2_fn, 0x44 | 0x48 | 0x49 | 0x4A) { b2::mem(frame, b2_fn, b2_a0, b2_a1, b2_a2, result); } // B2-TEMP\n")

# C) Pterm around the terminate call
c = "    let result = unsafe {\n        match func {\n            0x00 => tos_api::bdos::process::pterm::terminate_zero(),\n"
assert s.count(c) == 1, "anchor C"
s = s.replace(c, "    let b2_dying = tos_api::bdos::process::current() as u64; b2::pterm_pre(b2_dying, func); // B2-TEMP\n" + c)
c2 = "            _ => tos_api::bdos::process::pterm::terminate(rc),\n        }\n    };\n"
assert s.count(c2) == 1, "anchor C2"
s = s.replace(c2, c2 + "    b2::pterm_post(b2_dying); // B2-TEMP\n")

# D) Pexec entry
d = "fn handle_pexec(frame: &mut ExceptionFrame) {\n"
assert s.count(d) == 1, "anchor D"
s = s.replace(d, d + "    crate::kprintln!(\"[b2] Pexec mode={:#x} x1={:#x} x2={:#x} run={:#x}\", frame.gpr[0], frame.gpr[1], frame.gpr[2], tos_api::bdos::process::current() as u64); // B2-TEMP\n")

s += r'''
// B2-TEMP BEGIN -- audit instrumentation (measurement-2026-10-06-ldg-b2)
mod b2 {
    use super::ExceptionFrame;
    static mut LOG: [(u64, u64); 64] = [(0, 0); 64];
    static mut N: usize = 0;
    fn chain(fp0: u64) -> [u64; 8] {
        let mut ra = [0u64; 8];
        let mut fp = fp0;
        for r in ra.iter_mut() {
            if fp == 0 || fp & 7 != 0 || fp >= 0x1_0000_0000 { break; }
            let nfp = match tos_api::uaccess::get_user_u64(fp) { Ok(v) => v, Err(_) => break };
            let lr = match tos_api::uaccess::get_user_u64(fp + 8) { Ok(v) => v, Err(_) => break };
            *r = lr;
            fp = nfp;
        }
        ra
    }
    pub fn mem(frame: &ExceptionFrame, f: u16, a0: u64, a1: u64, a2: u64, res: i64) {
        let run = tos_api::bdos::process::current() as u64;
        let from_el0 = frame.spsr & 0xF == 0;
        let ra = if from_el0 { chain(frame.gpr[29]) } else { [0; 8] };
        crate::kprintln!("[b2] fn={:#x} a0={:#x} a1={:#x} a2={:#x} -> {:#x} run={:#x} el0={} elr={:#x} lr={:#x} ra={:x?}",
            f, a0, a1, a2, res, run, from_el0, frame.elr, frame.lr, ra);
        if (f == 0x48 || f == 0x44) && res > 0 && run != 0 {
            // SAFETY: single CPU, trap context.
            unsafe {
                if N < 64 { LOG[N] = (run, res as u64); N += 1; }
            }
        }
    }
    fn rd(va: u64) -> u64 {
        // fault-safe unprivileged load (Pterm comes from EL0)
        tos_api::uaccess::get_user_u64(va).unwrap_or(0xDEAD_DEAD)
    }
    pub fn pterm_pre(run: u64, func: u16) {
        if run == 0 { return; }
        crate::kprintln!("[b2] Pterm func={:#x} run={:#x} free_list.next@POPUP={:#x} @EX_LIB={:#x}",
            func, run, rd(run + 0x83d8), rd(run + 0x9278));
    }
    pub fn pterm_post(run: u64) {
        if run == 0 { return; }
        // SAFETY: single CPU, trap context, BDOS lock implicit.
        unsafe {
            for i in 0..N {
                let (r, a) = LOG[i];
                if r == run {
                    crate::kprintln!("[b2]   after Pterm of {:#x}: block {:#x} still allocated = {}",
                        run, a, tos_api::bdos::mem::traps::is_allocated(a));
                }
            }
        }
    }
}
// B2-TEMP END
'''
ex.write_text(s)
print("b2 instrumentation applied")
