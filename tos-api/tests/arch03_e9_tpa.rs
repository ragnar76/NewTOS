//! ARCH03 E9 — the TPA is the **largest free block**, as EmuTOS `alloc_tpa`
//! gives it (lead decision 2026-10-05, superseding "fixed 4 MiB").
//!
//! Sources the expectations come from (EmuTOS `references/emutos`, pin
//! `1a7484ab`):
//! * `bdos/proc.c:460-502` `alloc_tpa`: `ffit(-1L, &pmd)` / `ffit(-1L,
//!   &pmdalt)` measure each pool (`:466,476`); TT-RAM only with
//!   `PF_TTRAMLOAD` (`:475`); a pool qualifies when its largest block holds
//!   `needed` (`:467,477`); TT-RAM wins unless ST-RAM is larger and TT-RAM
//!   cannot hold `needed` + `((flags >> 28) + 1) * TPASIZE_QUANTUM`
//!   (`:34,481-484`); the **whole** block is taken (`ffit(size, …)`,
//!   `:486,493`) and its size is `*avail` → `p_hitpa = p + max` (`:374`);
//! * `bdos/proc.c:220,291`: `needed` = `sizeof(PD)` for `Pexec` 5/7, image +
//!   `sizeof(PD)` for a load;
//! * `bdos/umem.c` `xsetblk`: what the program gives back is an ordinary free
//!   block again, so the next program's TPA is the remainder.
//!
//! The pools are bookkeeping only (never dereferenced on the host; `kalias`
//! is the identity here and `alloc_tpa` only computes it).

use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::mem::{init, traps};
use tos_api::bdos::process::arena::{self, TPASIZE_QUANTUM, TPA_ALIGN};
use tos_types::error::E_OK;
use tos_types::{Long, MX_STRAM, MX_TTRAM, PF_FASTLOAD, PF_STANDARD, PF_TTRAMLOAD, PF_TTRAMMEM};

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
        unsafe { init::init(ST_START, ST_LEN, TT_START, TT_LEN) }.expect("the test pools");
    });
    g
}

fn largest(kind: PoolKind) -> u64 {
    pool_stats(kind).largest_free as u64
}

// ── the pool choice (pure) ──────────────────────────────────────────────

#[test]
fn without_pf_ttramload_the_tpa_is_st_ram_whatever_tt_ram_has() {
    let k = 128 * 1024u64;
    assert_eq!(arena::choose_pool(0, k, 10 << 20, 900 << 20), Some(MX_STRAM));
    assert_eq!(arena::choose_pool(PF_FASTLOAD | PF_TTRAMMEM, k, 10 << 20, 900 << 20), Some(MX_STRAM));
    assert_eq!(arena::choose_pool(0, 11 << 20, 10 << 20, 900 << 20), None, "ST too small: ENSMEM");
}

#[test]
fn with_pf_ttramload_tt_ram_wins_unless_st_is_larger_and_tt_misses_the_quantum() {
    let n = 0x10_0000u64; // needed: 1 MiB
    // TT larger or equal: TT.
    assert_eq!(arena::choose_pool(PF_STANDARD, n, 10 << 20, 900 << 20), Some(MX_TTRAM));
    assert_eq!(arena::choose_pool(PF_TTRAMLOAD, n, 4 << 20, 4 << 20), Some(MX_TTRAM));
    // ST larger, TT holds needed + 1 quantum (bits 31-28 = 0): still TT.
    assert_eq!(arena::choose_pool(PF_TTRAMLOAD, n, 8 << 20, n + TPASIZE_QUANTUM), Some(MX_TTRAM));
    // ...one byte less: ST ("force allocation in ST RAM", proc.c:483).
    assert_eq!(arena::choose_pool(PF_TTRAMLOAD, n, 8 << 20, n + TPASIZE_QUANTUM - 1), Some(MX_STRAM));
    // Bits 31-28 = 0xF: 16 quanta = 2 MiB on top of needed.
    let f = PF_TTRAMLOAD | 0xF000_0000;
    assert_eq!(arena::choose_pool(f, n, 8 << 20, n + 16 * TPASIZE_QUANTUM), Some(MX_TTRAM));
    assert_eq!(arena::choose_pool(f, n, 8 << 20, n + 16 * TPASIZE_QUANTUM - 1), Some(MX_STRAM));
    // Only TT holds it: TT, whatever the quantum.
    assert_eq!(arena::choose_pool(f, 9 << 20, 8 << 20, 9 << 20), Some(MX_TTRAM));
    // Neither: ENSMEM.
    assert_eq!(arena::choose_pool(PF_TTRAMLOAD, 9 << 20, 8 << 20, 1 << 20), None);
    // No TT-RAM at all (alt = 0): ST.
    assert_eq!(arena::choose_pool(PF_STANDARD, n, 8 << 20, 0), Some(MX_STRAM));
}

// ── the block (bookkeeping pools) ───────────────────────────────────────

#[test]
fn the_tpa_is_the_whole_largest_block_and_the_next_one_gets_what_is_given_back() {
    let _g = lock();
    let tt0 = largest(PoolKind::Ttram);
    let used0 = pool_stats(PoolKind::Ttram).used_bytes;
    // SAFETY: under the lock (the BDOS lock's stand-in).
    let a = unsafe { arena::alloc_tpa(PF_STANDARD, 0x4000) }.expect("a TPA");
    assert_eq!(a.base % TPA_ALIGN, 0, "page-aligned");
    assert!(a.base >= TT_START && a.base < TT_START + TT_LEN, "PF_TTRAMLOAD: TT-RAM");
    assert_eq!(a.len, tt0, "p_hitpa - p_lowtpa = the largest TT block (EmuTOS *avail)");
    assert_eq!(largest(PoolKind::Ttram), 0, "nothing left in TT-RAM until it shrinks");
    assert_eq!(pool_stats(PoolKind::Ttram).used_bytes as u64, used0 as u64 + tt0);
    // The program keeps 64 KiB (its crt0's Mshrink) ...
    // SAFETY: under the lock.
    assert_eq!(unsafe { traps::mshrink(0, a.base as Long, 0x1_0000) }, E_OK);
    // ... and the next program's TPA is the rest, right behind it.
    // SAFETY: under the lock.
    let b = unsafe { arena::alloc_tpa(PF_STANDARD, 0x4000) }.expect("a second TPA");
    assert_eq!(b.base, a.base + 0x1_0000);
    assert_eq!(b.len, tt0 - 0x1_0000);
    // SAFETY: under the lock; both are live blocks of ours.
    unsafe {
        arena::free_tpa(b.base as Long);
        arena::free_tpa(a.base as Long);
    }
    assert_eq!(largest(PoolKind::Ttram), tt0, "both back, coalesced");
}

#[test]
fn without_pf_ttramload_the_largest_st_block_page_aligned_slack_returned() {
    let _g = lock();
    let st0 = largest(PoolKind::Stram);
    let free_blocks0 = pool_stats(PoolKind::Stram).free_blocks;
    // Misalign the next free ST byte (the 16-byte grain without protection).
    // SAFETY: under the lock.
    let small = unsafe { traps::mxalloc(48, MX_STRAM) };
    assert_ne!(small, 0);
    // SAFETY: under the lock.
    let t = unsafe { arena::alloc_tpa(PF_FASTLOAD | PF_TTRAMMEM, 0x100) }.expect("an ST TPA");
    assert!(t.base >= ST_START && t.base < ST_START + ST_LEN, "no PF_TTRAMLOAD: ST-RAM");
    assert_eq!(t.base % TPA_ALIGN, 0);
    let slack = t.base - (small as u64 + 48);
    assert!(slack > 0 && slack < TPA_ALIGN, "the TPA starts on the next page");
    assert_eq!(t.len, st0 - 48 - slack, "the rest of the largest block");
    assert_eq!(largest(PoolKind::Stram), slack, "only the slack in front is free");
    // SAFETY: under the lock.
    unsafe {
        arena::free_tpa(t.base as Long);
        assert_eq!(traps::mfree(small), E_OK);
    }
    assert_eq!(largest(PoolKind::Stram), st0);
    assert_eq!(pool_stats(PoolKind::Stram).free_blocks, free_blocks0);
}

#[test]
fn a_program_the_largest_block_cannot_hold_is_ensmem_and_costs_nothing() {
    let _g = lock();
    let st = pool_stats(PoolKind::Stram);
    let tt = pool_stats(PoolKind::Ttram);
    // SAFETY: under the lock.
    assert!(unsafe { arena::alloc_tpa(PF_STANDARD, TT_LEN + 1) }.is_none());
    // SAFETY: under the lock.
    assert!(unsafe { arena::alloc_tpa(0, ST_LEN + 1) }.is_none());
    assert_eq!(pool_stats(PoolKind::Stram).used_bytes, st.used_bytes);
    assert_eq!(pool_stats(PoolKind::Ttram).used_bytes, tt.used_bytes);
    // A PF_TTRAMLOAD program too big for TT-RAM but not for ST-RAM gets
    // ST-RAM (proc.c: st_ram_available, alt not).
    let big = largest(PoolKind::Ttram);
    // SAFETY: under the lock.
    let a = unsafe { arena::alloc_tpa(PF_STANDARD, 0x2000) }.expect("TT TPA");
    assert_eq!(a.len, big);
    // TT-RAM is full now; the next PF_TTRAMLOAD program lands in ST-RAM.
    // SAFETY: under the lock.
    let b = unsafe { arena::alloc_tpa(PF_STANDARD, 0x2000) }.expect("ST TPA");
    assert!(b.base >= ST_START && b.base < ST_START + ST_LEN);
    // SAFETY: under the lock.
    unsafe {
        arena::free_tpa(b.base as Long);
        arena::free_tpa(a.base as Long);
    }
}

#[test]
fn the_largest_free_block_is_the_first_of_maximal_length() {
    let _g = lock();
    // Carve TT-RAM into [A 1 MiB][hole 4 MiB][B 1 MiB][hole 4 MiB][C rest].
    // SAFETY: under the lock.
    unsafe {
        let a = traps::mxalloc(1 << 20, MX_TTRAM);
        let h1 = traps::mxalloc(4 << 20, MX_TTRAM);
        let b = traps::mxalloc(1 << 20, MX_TTRAM);
        let h2 = traps::mxalloc(4 << 20, MX_TTRAM);
        let rest = traps::mxalloc((TT_LEN - (10 << 20)) as Long, MX_TTRAM);
        assert!(a != 0 && h1 != 0 && b != 0 && h2 != 0 && rest != 0);
        assert_eq!(traps::mfree(h1), E_OK);
        assert_eq!(traps::mfree(h2), E_OK);
        assert_eq!(
            traps::largest_free_block(init::pmdalt()),
            Some((h1, 4 << 20)),
            "ffit(ffit(-1)) takes the first block of the largest length"
        );
        let t = arena::alloc_tpa(PF_STANDARD, 0x2000).expect("TPA");
        assert_eq!((t.base, t.len), (h1 as u64, 4 << 20));
        arena::free_tpa(t.base as Long);
        for blk in [a, b, rest] {
            assert_eq!(traps::mfree(blk), E_OK);
        }
    }
    assert_eq!(largest(PoolKind::Ttram), TT_LEN);
}
