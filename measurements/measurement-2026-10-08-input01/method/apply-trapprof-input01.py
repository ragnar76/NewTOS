#!/usr/bin/env python3
"""apply-trapprof-input01.py <scratch-tree> -- INPUT01 measurement only, scratch
copy only (never the shared tree). Wraps the synchronous-exception entry
`rust_sync_handler` (kernel/src/arch/aarch64/exceptions.rs) in a timer: every
exception that kept the CPU in the handler -- i.e. with IRQs masked, so with
no input pump possible -- for >= 5 ms prints one serial line

  [i01] trap <us> us from el<0|1> ec 0x.. svc 0x.. fn 0x.. x0 0x.. hz <hz_200 at exit>

(`fn` = x8, the GEMDOS function number for svc #1; `svc` = the immediate =
trap class). This is the "where does the time go while the pointer is frozen"
half of the evidence; the other half is the injector's step lag.
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
p = t / "kernel/src/arch/aarch64/exceptions.rs"
s = p.read_text()
a = '#[unsafe(no_mangle)]\npub extern "C" fn rust_sync_handler(frame: &mut ExceptionFrame) {\n'
assert s.count(a) == 1, "rust_sync_handler anchor"
s = s.replace(a, 'fn rust_sync_handler_i01_inner(frame: &mut ExceptionFrame) {\n')
s += '''
// I01-TEMP BEGIN -- long-trap profiler (INPUT01 measurement, scratch only)
#[unsafe(no_mangle)]
pub extern "C" fn rust_sync_handler(frame: &mut ExceptionFrame) {
    let (t0, frq): (u64, u64);
    unsafe {
        core::arch::asm!("mrs {0}, cntpct_el0", "mrs {1}, cntfrq_el0", out(reg) t0, out(reg) frq,
                         options(nomem, nostack, preserves_flags));
    }
    let el = if frame.spsr & 0xF == 0 { 0 } else { 1 };
    let ec = frame.esr >> 26;
    let imm = frame.esr & 0xFFFF;
    let fnum = frame.gpr[8];
    let x0 = frame.gpr[0];
    rust_sync_handler_i01_inner(frame);
    let t1: u64;
    unsafe {
        core::arch::asm!("mrs {0}, cntpct_el0", out(reg) t1, options(nomem, nostack, preserves_flags));
    }
    let us = if frq == 0 { 0 } else { (t1.wrapping_sub(t0) as u128 * 1_000_000 / frq as u128) as u64 };
    if us >= 5000 {
        crate::kprintln!("[i01] trap {} us from el{} ec {:#x} svc {:#x} fn {:#x} x0 {:#x} hz {}",
                         us, el, ec, imm, fnum, x0, tos_api::bios::tick::hz_200());
    }
}
// I01-TEMP END
'''
p.write_text(s)
print("instrumented", p)
