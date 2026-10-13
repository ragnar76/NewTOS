#!/usr/bin/env python3
"""vt-instrumentation.py <scratch-tree>

Scratch-only memory trace for the VFORKT diagnosis (never applied to the
shared tree). Prints to the serial console:

  [vt] alloc  start len own=<m_own cookie> prot          (traps::mxalloc, every block)
  [vt] free   start len                                  (traps::mfree, every path:
                                                          user Mfree, Pterm sweep, kernel)
  [vt] mshrk0 / mshrk start oldlen newlen                (traps::mshrink)
  [vt] svc fn=.. slot run x0 x1 x2 -> res elr lr sp stk= (user GEMDOS 0x44/0x48/0x49/0x4A,
                                                          + user-stack words that look like
                                                          return addresses)
  [vt] pexec mode tail env slot run                      (handle_pexec entry)
  [vt] pterm fn rc slot run                              (handle_pterm entry)
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

def patch(rel, old, new, count=1):
    p = t / rel
    s = p.read_text()
    assert s.count(old) == count, f"{rel}: anchor count {s.count(old)} != {count}: {old[:60]!r}"
    s = s.replace(old, new)
    p.write_text(s)

# tos-api: a log hook the kernel fills in.
patch("tos-api/src/lib.rs", "pub use trap::{TrapArgs, TrapClass};\n",
"""pub use trap::{TrapArgs, TrapClass};

/// VT-TEMP scratch log hook (vforkt diagnosis).
#[allow(missing_docs)]
pub static VT_LOG: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
/// VT-TEMP
#[allow(missing_docs)]
pub fn vt_log(a: core::fmt::Arguments) {
    let p = VT_LOG.load(core::sync::atomic::Ordering::Relaxed);
    if p != 0 {
        // SAFETY: only ever stores a `fn(Arguments)`.
        let f: fn(core::fmt::Arguments) = unsafe { core::mem::transmute(p) };
        f(a);
    }
}
""")

TR = "tos-api/src/bdos/mem/traps.rs"
patch(TR, """        let (start, len) = unsafe { ((*m).m_start, (*m).m_length) };
        // A fresh block has no further owners""",
"""        let (start, len) = unsafe { ((*m).m_start, (*m).m_length) };
        crate::vt_log(format_args!("[vt] alloc  {:#010x} len={:#x} own={:#x} prot={:?} mode={}", start, len, owner as usize, prot, mode));
        // A fresh block has no further owners""")
patch(TR, """    let len = unsafe { (*p).m_length };
    let mut supply = unsafe { init::supply() };
    // SAFETY: caller holds BDOS lock; `p` is an MD on `mp_mal`.
    unsafe {
        pool::freeit(p, mpb, &mut supply);""",
"""    let len = unsafe { (*p).m_length };
    crate::vt_log(format_args!("[vt] free   {:#010x} len={:#x} own={:#x} run={:#x}", addr, len, unsafe { (*p).m_own } as usize, current_owner() as usize));
    let mut supply = unsafe { init::supply() };
    // SAFETY: caller holds BDOS lock; `p` is an MD on `mp_mal`.
    unsafe {
        pool::freeit(p, mpb, &mut supply);""")
patch(TR, """        // Equivalent to Mfree(blk).
""", """        // Equivalent to Mfree(blk).
        crate::vt_log(format_args!("[vt] mshrk0 {:#010x} len={:#x}", blk, m_len));
""")
patch(TR, """    // 5. Real shrink — split off the tail.
""", """    // 5. Real shrink — split off the tail.
    crate::vt_log(format_args!("[vt] mshrk  {:#010x} {:#x} -> {:#x}", blk, m_len, len));
""")

EX = "kernel/src/arch/aarch64/exceptions.rs"
patch(EX, """    // Mid-syscall cooperative yield (S2 restartable block, design §4.3 + §8.2
""", """    // VT-TEMP: user memory calls with their callers.
    if matches!(class, TrapClass::Gemdos) && frame.spsr & 0xF == 0
        && matches!(frame.gpr[8] as u16, 0x44 | 0x48 | 0x49 | 0x4A)
    {
        let run = tos_api::bdos::process::current();
        crate::kprint!("[vt] svc fn={:#x} slot={} run={:#x} x0={:#x} x1={:#x} x2={:#x} -> {:#x} elr={:#x} lr={:#x} sp={:#x} stk=",
            frame.gpr[8] as u16, tos_api::bdos::process::sched::running_slot(), run as usize,
            frame.gpr[0], frame.gpr[1], frame.gpr[2], result as u64, frame.elr, frame.lr, frame.sp_el0);
        if !run.is_null() {
            let bp = run as u64;
            let mut n = 0;
            for i in 0..96u64 {
                match tos_api::uaccess::get_user_u64(frame.sp_el0 + 8 * i) {
                    Ok(v) if v > bp + 0x1000 && v < bp + 0x40000 && v & 3 == 0 => {
                        crate::kprint!("+{:#x}:{:#x} ", 8 * i, v - bp);
                        n += 1;
                        if n >= 12 { break; }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
        crate::kprintln!("");
    }

    // Mid-syscall cooperative yield (S2 restartable block, design §4.3 + §8.2
""")
patch(EX, """    let blkln = frame.gpr[0] as u32 as i32;
""", """    let blkln = frame.gpr[0] as u32 as i32;
    crate::kprintln!("[vt] pterm fn={:#x} rc={} slot={} run={:#x} elr={:#x} lr={:#x}", func, rc,
        tos_api::bdos::process::sched::running_slot(), tos_api::bdos::process::current() as usize, frame.elr, frame.lr);
""")
patch(EX, """    let from_el0 = frame.spsr & 0xF == 0;
""", """    let from_el0 = frame.spsr & 0xF == 0;
    tos_api::VT_LOG.store(vt_print as fn(core::fmt::Arguments) as usize, core::sync::atomic::Ordering::Relaxed);
    crate::kprintln!("[vt] pexec mode={} tail={:#x} env={:#x} slot={} run={:#x} elr={:#x} lr={:#x}", mode, tail_arg, env_arg,
        tos_api::bdos::process::sched::running_slot(), tos_api::bdos::process::current() as usize, frame.elr, frame.lr);
""")
patch(EX, """fn handle_pexec(frame: &mut ExceptionFrame) {
""", """fn vt_print(a: core::fmt::Arguments) {
    crate::kprintln!("{}", a);
}

fn handle_pexec(frame: &mut ExceptionFrame) {
""")
print("vt instrumentation applied to", t)
