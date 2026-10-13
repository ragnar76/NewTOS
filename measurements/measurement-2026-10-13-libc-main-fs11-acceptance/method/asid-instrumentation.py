#!/usr/bin/env python3
"""asid-instrumentation.py <scratch-tree> <bin-dir> [--virt-launch]
                          [--neg-notlbi] [--neg-noinval]

ARCH03 stage 4, scratch copies only -- NEVER applied to the shared tree.

Always (both boards): the SWITCH-COST counters ("SWCOST-TEMP"). In
kernel/src/arch/aarch64/exceptions.rs every call of the two TTBR0
primitives on the production switch paths (handle_pexec, apply_switch,
restore_kernel_park, finish_terminate) is bracketed with CNTVCT_EL0 reads:
restore_boot_ttbr0 (ASIDs off: empty root + TLBI VMALLE1; on: empty root,
no TLBI) and install_ttbr0 (off: plain msr; on: ASID-tagged msr, plus the
rare rollover flush). Every 64th install prints
  [swcost] installs=N restores=R restore_ticks=.. install_ticks=..
           cntfrq=.. asid gen=G rollovers=K assigned=A limit=L
(cumulative; ticks of the virtual counter). The UART lines the paths print
between the two primitives are NOT inside the brackets.

--virt-launch (board-qemu-virt has no SD card): C:\\ASIDKID.NAP and the
driver variant for the boot argument (memprot=off: ASIDSTRO, else ASIDSTR)
join the RAM program registry and C:\\ASIDSTR.NAP is launched once from the
boot path at the AUTO folder's position (before the AES: part D skipped).
The crash prompt of part C is answered over the serial line (drive.py
--answer).

NEGATIVE CONTROLS (deliberately broken kernels, to show the probes see the
defect they are meant to see):
  --neg-notlbi   restore_boot_ttbr0 WITHOUT its `tlbi vmalle1` (ASIDs off
                 path): with ASIDs off every root runs as ASID 0, so nothing
                 drops the previous root's page-0 translation (0b
                 `sw-notlbi`). Expected: ASIDSTR parts A/B/D FAIL
                 (memprot=off, where no protection flush masks it).
  --neg-noinval  mmu::owe_inval does nothing (ASIDs on): no per-VA
                 invalidation after a narrowing. Expected: MPROT3B case 'f'
                 (a process writes a page it just freed -- the same ASID)
                 goes through = FAIL.
"""
import re, sys, pathlib
t = pathlib.Path(sys.argv[1])
progs = pathlib.Path(sys.argv[2]).resolve()
virt = '--virt-launch' in sys.argv

ex = t / "kernel/src/arch/aarch64/exceptions.rs"
s = ex.read_text()
old = "crate::arch::aarch64::user_entry::restore_boot_ttbr0();"
n = s.count(old)
assert n == 4, f"exceptions.rs: expected 4 restore_boot_ttbr0 sites, found {n}"
s = s.replace(old, "{ let __t = swcost::now(); crate::arch::aarch64::user_entry::restore_boot_ttbr0(); swcost::restored(__t); } // SWCOST-TEMP")
pat = re.compile(r"tos_api::bdos::process::mmu::install_ttbr0\(([^;]*)\);")
n = len(pat.findall(s))
assert n == 3, f"exceptions.rs: expected 3 install_ttbr0 sites, found {n}"
s = pat.sub(lambda m: "{ let __t = swcost::now(); tos_api::bdos::process::mmu::install_ttbr0(" + m.group(1) + "); swcost::installed(__t); } // SWCOST-TEMP", s)
s += '''
// SWCOST-TEMP BEGIN -- ARCH03-4 switch-cost counters (scratch only)
pub mod swcost {
    use core::sync::atomic::{AtomicU64, Ordering};
    static INSTALLS: AtomicU64 = AtomicU64::new(0);
    static RESTORES: AtomicU64 = AtomicU64::new(0);
    static T_RESTORE: AtomicU64 = AtomicU64::new(0);
    static T_INSTALL: AtomicU64 = AtomicU64::new(0);
    #[inline(always)]
    pub fn now() -> u64 {
        let v: u64;
        unsafe { core::arch::asm!("isb", "mrs {}, cntvct_el0", out(reg) v, options(nostack, preserves_flags)) };
        v
    }
    fn freq() -> u64 {
        let v: u64;
        unsafe { core::arch::asm!("mrs {}, cntfrq_el0", out(reg) v, options(nomem, nostack, preserves_flags)) };
        v
    }
    pub fn restored(t0: u64) {
        T_RESTORE.fetch_add(now().wrapping_sub(t0), Ordering::Relaxed);
        RESTORES.fetch_add(1, Ordering::Relaxed);
    }
    pub fn installed(t0: u64) {
        T_INSTALL.fetch_add(now().wrapping_sub(t0), Ordering::Relaxed);
        let n = INSTALLS.fetch_add(1, Ordering::Relaxed) + 1;
        if n % 64 == 0 {
            let (g, r, a, l) = tos_api::bdos::process::mmu::asid_stats();
            crate::kprintln!(
                "[swcost] installs={} restores={} restore_ticks={} install_ticks={} cntfrq={} asid gen={} rollovers={} assigned={} limit={}",
                n, RESTORES.load(Ordering::Relaxed), T_RESTORE.load(Ordering::Relaxed),
                T_INSTALL.load(Ordering::Relaxed), freq(), g, r, a, l
            );
        }
    }
}
// SWCOST-TEMP END
'''
ex.write_text(s)

if virt:
    lib = t / "kernel/src/lib.rs"
    s = lib.read_text()
    old = """    launch::run_auto_folder();
"""
    assert s.count(old) == 1, "lib.rs auto-folder anchor"
    s = s.replace(old, f"""    launch::run_auto_folder();

    // ASID4-TEMP BEGIN -- the stage-4 ASID probe from the RAM registry (virt)
    #[cfg(feature = "board-qemu-virt")]
    {{
        static ASIDKID: &[u8] = include_bytes!("{progs}/ASIDKID.NAP");
        static ASIDSTR: &[u8] = include_bytes!("{progs}/ASIDSTR.NAP");
        static ASIDSTRO: &[u8] = include_bytes!("{progs}/ASIDSTRO.NAP");
        tos_api::bdos::process::registry::register(b"C:\\\\ASIDKID.NAP", ASIDKID);
        let drv = if tos_api::bdos::process::memprot::enabled() {{ ASIDSTR }} else {{ ASIDSTRO }};
        tos_api::bdos::process::registry::register(b"C:\\\\ASIDSTR.NAP", drv);
        let rc = launch::run_program_with_tail_rc(b"C:\\\\ASIDSTR.NAP", &[]);
        kprintln!("[asid4] back in the boot path after ASIDSTR, launch rc={{}}", rc);
    }}
    // ASID4-TEMP END
""")
    lib.write_text(s)
if '--neg-notlbi' in sys.argv:
    ue = t / "kernel/src/arch/aarch64/user_entry.rs"
    u = ue.read_text()
    old = """            "tlbi vmalle1",
            "dsb ish",
"""
    assert u.count(old) == 1, "user_entry.rs tlbi anchor"
    u = u.replace(old, """            // NEG-NOTLBI: tlbi removed
            "dsb ish",
""")
    ue.write_text(u)
if '--neg-noinval' in sys.argv:
    mm = t / "tos-api/src/bdos/process/mmu.rs"
    u = mm.read_text()
    old = """unsafe fn owe_inval(va: u64, owner: Option<usize>) {
"""
    assert u.count(old) == 1, "mmu.rs owe_inval anchor"
    u = u.replace(old, old + """    if true { let _ = (va, owner); return; } // NEG-NOINVAL
""")
    mm.write_text(u)
print("asid instrumentation applied to", t, "(virt launch)" if virt else "")
