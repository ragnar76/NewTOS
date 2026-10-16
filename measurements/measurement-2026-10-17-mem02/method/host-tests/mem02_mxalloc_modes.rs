//! MEM02 — `Mxalloc` honours all four pool modes for an EL0 caller, through
//! the GEMDOS trap entry (`tos_api::gemdos::memory::mxalloc`), and returns the
//! pool block itself (ARCH03 stage 1: VA = PA).
//!
//! EmuTOS `xmxalloc`, `references/emutos/bdos/umem.c:234-330`:
//! `mode &= MX_MODEMASK` (:241); `-1` = largest block of the selected pool,
//! the larger of both for the `PREF*` modes (:247-276); `MX_STRAM` -> `pmd`
//! only (:293-295), `MX_TTRAM` -> `pmdalt` only (:297-299), `MX_PREFSTRAM` ->
//! `pmd` then `pmdalt` (:301-307), `MX_PREFTTRAM` -> `pmdalt` then `pmd`
//! (:308-314). Draft of the MEM02 phase-1 report; would have been red on
//! c806a645 (the ADR-021 window: every EL0 block at a window VA from 4 MiB,
//! below the top of ST-RAM, whichever pool it came from).

use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::process::{self, PROCESS_TABLE};
use tos_api::trap::TrapArgs;
use tos_types::error::E_OK;
use tos_types::{BasePage, Long, F_PROT_G, MX_PREFSTRAM, MX_PREFTTRAM, MX_STRAM, MX_TTRAM, PF_TTRAMMEM};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any allocation.
        unsafe { tos_api::bdos::mem::init::init(ST_START, ST_LEN, TT_START, TT_LEN) }.expect("the test pools");
    });
    g
}

struct Proc {
    slot: usize,
    bp: *mut BasePage,
}

impl Proc {
    fn new(flags: u32) -> Proc {
        let slot = process::allocate_slot().expect("a slot");
        let mut b = Box::new([0u8; 256]);
        b[0x28..0x2C].copy_from_slice(&flags.to_le_bytes()); // p_flags
        let bp = Box::into_raw(b) as *mut BasePage;
        PROCESS_TABLE[slot].basepage_kva.store(bp, std::sync::atomic::Ordering::Release);
        process::set_current(process::pd_of(slot));
        Proc { slot, bp }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        process::set_current(std::ptr::null_mut());
        process::free_slot(self.slot);
        // SAFETY: the slot no longer names the box.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

fn mxalloc(amount: i64, mode: i16) -> Long {
    let args = TrapArgs {
        regs: [amount as u64, mode as u16 as u64, 0, 0, 0, 0, 0, 0],
        func: 0x44,
        svc_imm: 1,
        space: tos_api::uaccess::Space::Kernel,
    };
    tos_api::gemdos::memory::mxalloc(&args)
}

fn mfree(a: Long) -> Long {
    let args = TrapArgs { regs: [a as u64, 0, 0, 0, 0, 0, 0, 0], func: 0x49, svc_imm: 1, space: tos_api::uaccess::Space::Kernel };
    tos_api::gemdos::memory::mfree(&args)
}

fn in_st(a: Long) -> bool {
    (a as u64) >= ST_START && (a as u64) < ST_START + ST_LEN
}

fn in_tt(a: Long) -> bool {
    (a as u64) >= TT_START && (a as u64) < TT_START + TT_LEN
}

#[test]
fn the_size_queries_report_the_selected_pools() {
    let _g = lock();
    let _p = Proc::new(PF_TTRAMMEM);
    let st = pool_stats(PoolKind::Stram).largest_free;
    let tt = pool_stats(PoolKind::Ttram).largest_free;
    assert_eq!(mxalloc(-1, MX_STRAM), st);
    assert_eq!(mxalloc(-1, MX_TTRAM), tt);
    assert_eq!(mxalloc(-1, MX_PREFSTRAM), st.max(tt), "umem.c:257-273: the larger of both");
    assert_eq!(mxalloc(-1, MX_PREFTTRAM), st.max(tt));
    assert_eq!(mxalloc(-1, 4), st, "umem.c:241: mode masked to MX_STRAM");
}

#[test]
fn each_mode_takes_its_pool_and_returns_the_block_itself() {
    let _g = lock();
    let _p = Proc::new(PF_TTRAMMEM);
    let s = mxalloc(0x1_0000, MX_STRAM);
    let t = mxalloc(0x1_0000, MX_TTRAM);
    let ps = mxalloc(0x1_0000, MX_PREFSTRAM);
    let pt = mxalloc(0x1_0000, MX_PREFTTRAM);
    let ptg = mxalloc(0x1_0000, MX_PREFTTRAM | F_PROT_G);
    assert!(in_st(s), "MX_STRAM s = 0x{s:x}");
    assert!(in_tt(t), "MX_TTRAM t = 0x{t:x}");
    assert!(in_st(ps), "MX_PREFSTRAM ps = 0x{ps:x}");
    assert!(in_tt(pt), "MX_PREFTTRAM pt = 0x{pt:x}");
    assert!(in_tt(ptg), "MX_PREFTTRAM|F_PROT_G = 0x{ptg:x}: protection bits do not pick the pool");
    for a in [s, t, ps, pt, ptg] {
        assert_eq!(mfree(a), E_OK);
    }
}

#[test]
fn a_forcing_mode_refuses_what_its_pool_cannot_give_and_a_preference_falls_back() {
    let _g = lock();
    let _p = Proc::new(PF_TTRAMMEM);
    let st = pool_stats(PoolKind::Stram).largest_free;
    let tt = pool_stats(PoolKind::Ttram).largest_free;
    assert_eq!(mxalloc(st as i64 + 0x1_0000, MX_STRAM), 0, "umem.c:293-295: ST only");
    assert_eq!(mxalloc(tt as i64 + 0x1_0000, MX_TTRAM), 0, "umem.c:297-299: TT only");
    let f = mxalloc(st as i64 + 0x1_0000, MX_PREFSTRAM);
    assert!(in_tt(f), "umem.c:301-307: PREFST falls back to TT, f = 0x{f:x}");
    assert_eq!(mfree(f), E_OK);
    assert_eq!(pool_stats(PoolKind::Stram).largest_free, st);
    assert_eq!(pool_stats(PoolKind::Ttram).largest_free, tt);
}
