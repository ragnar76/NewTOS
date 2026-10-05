//! ARCH03 stage 1 — **one shared user address space, VA = PA, page 0 per
//! process** (`docs/analysis/arch03-implementation-plan.md` §3.1/§3.2,
//! invariants I1/I3; decisions E1, E3, E9, E18).
//!
//! What is checked here, on the host, against the real data structures:
//!
//! * the page tables (`bdos::process::mmu`): a pool address translates to
//!   itself in every process root (I1), 2-MiB blocks where a whole aligned
//!   region is pool, pages at the edges; page 0 translates to the slot's own
//!   page, different per slot (I3); a later publication reaches every root,
//!   existing and future; page 0 cannot be published; a valid entry is never
//!   rewritten with other attributes;
//! * the BDOS side: the TPA block starts on a page (`mxalloc_aligned`) and is
//!   an ordinary block (`Mshrink`/`Mfree` by its address); `Malloc`/`Mxalloc`
//!   hand out `m_start` itself and `Malloc(-1)` is the largest free block —
//!   EmuTOS `bdos/umem.c`, no per-process window any more;
//! * `usermem::resolve_kva`: allocated pool memory is live, freed pool memory
//!   is not, page 0 is the slot's own.
//!
//! The tables are kernel statics; on the host their "physical" addresses are
//! the same wrapping arithmetic the target uses, and `mmu::translate` walks
//! them back. The pool is pure bookkeeping (its MDs live in a static arena),
//! so its addresses are never dereferenced. One binary, one pool, one lock.

use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::mem::traps;
use tos_api::bdos::process::mmu::{self, MapError, PageFlags};
use tos_api::bdos::process::{self, usermem, PROCESS_TABLE};
use tos_api::trap::TrapArgs;
use tos_types::error::{EIMBA, E_OK};
use tos_types::{BasePage, Long, MX_STRAM, MX_TTRAM, PF_TTRAMMEM};

const ST_START: u64 = 0x0040_0000; // the Pi layout: 4 MiB OS reserve below
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024 + 0x3000; // ends off a 2-MiB boundary
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;
const MIB2: u64 = 2 * 1024 * 1024;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any allocation.
        unsafe { tos_api::bdos::mem::init::init(ST_START, ST_LEN, TT_START, TT_LEN) }
            .expect("the test pools");
        // SAFETY: single-threaded under the lock; boot-time mapping.
        unsafe { mmu::map_pools() }.expect("the pools map");
    });
    g
}

/// The physical address the tables give a kernel static (`va_to_pa`), as a
/// leaf descriptor's 48-bit output-address field holds it (on the host the
/// wrapping arithmetic carries a bit 48 that the field does not keep).
fn pa_of(kva: *mut u8) -> u64 {
    (kva as u64).wrapping_sub(VA_OFFSET) & ((1u64 << 48) - 1)
}

fn is_block(desc: u64) -> bool {
    desc & 0b10 == 0
}

// ── page tables ─────────────────────────────────────────────────────────

#[test]
fn a_pool_address_is_its_own_address_in_every_process_root() {
    let _g = lock();
    for slot in [0usize, 5, 15] {
        // SAFETY: under the lock; no slot root is installed on the host.
        unsafe { mmu::init_slot_tables(slot) }.unwrap();
        for va in [ST_START, ST_START + 0x1234, ST_START + ST_LEN - 1, TT_START + 0x55_5000, TT_START + TT_LEN - 1] {
            let (pa, desc) = mmu::translate(slot, va).unwrap_or_else(|| panic!("0x{va:x} unmapped in slot {slot}"));
            assert_eq!(pa, va, "VA = PA (I1)");
            // The pool is user RW, EL0-executable, EL1-never (plan §3.2).
            let want = mmu::page_bits(PageFlags::UserRwExec);
            assert_eq!(desc & want & !0b10, want & !0b10, "pool attributes at 0x{va:x}");
        }
        // The pools only: the gap between them, the OS reserve below and
        // the space above stay unmapped.
        for va in [0x1000, 0x3F_F000, ST_START + ST_LEN, TT_START - 1, TT_START + TT_LEN + 0x1000, 0x7FFF_F000] {
            assert_eq!(mmu::translate(slot, va), None, "0x{va:x} must be unmapped");
        }
    }
}

#[test]
fn whole_aligned_regions_are_blocks_and_edges_are_pages() {
    let _g = lock();
    // SAFETY: under the lock.
    unsafe { mmu::init_slot_tables(1) }.unwrap();
    let (_, d) = mmu::translate(1, ST_START).unwrap();
    assert!(is_block(d), "ST pool start is 2-MiB aligned and whole: a block");
    let (_, d) = mmu::translate(1, TT_START + TT_LEN - 1).unwrap();
    assert!(!is_block(d), "the TT pool ends 0x3000 into a region: pages there");
    assert_eq!(mmu::translate(1, TT_START + TT_LEN), None, "and not one page more");
}

#[test]
fn page_zero_is_each_processs_own() {
    let _g = lock();
    for slot in [2usize, 3] {
        // SAFETY: under the lock.
        unsafe { mmu::init_slot_tables(slot) }.unwrap();
    }
    let p2 = pa_of(mmu::page0_kva(2).unwrap());
    let p3 = pa_of(mmu::page0_kva(3).unwrap());
    assert_ne!(p2, p3);
    assert_eq!(mmu::translate(2, 0x4F2).map(|t| t.0), Some(p2 + 0x4F2), "$4F2 in slot 2 is slot 2's page");
    assert_eq!(mmu::translate(3, 0x5A0).map(|t| t.0), Some(p3 + 0x5A0), "$5A0 in slot 3 is slot 3's page");
    // Page 0 is not a pool page and cannot be published to everybody.
    // SAFETY: under the lock.
    let e = unsafe { mmu::map_shared(0, 0x1000, PageFlags::UserRoNx, false) };
    assert_eq!(e, Err(MapError::VaOutOfRange));
}

#[test]
fn a_publication_reaches_every_root_present_and_future() {
    let _g = lock();
    // SAFETY: under the lock.
    unsafe { mmu::init_slot_tables(4) }.unwrap();
    // One page in the first 2 MiB (a kernel structure below the pools, the
    // Pi kernel image's region) and one in another GiB (virt's framebuffer).
    let low = 0x0008_7000u64;
    let high = 0x5081_7000u64;
    // SAFETY: under the lock; nothing is dereferenced.
    unsafe {
        mmu::map_shared(low, 0x1000, PageFlags::UserRoNx, false).unwrap();
        mmu::map_shared(high, 0x3000, PageFlags::UserRwNxUncached, false).unwrap();
        mmu::init_slot_tables(9).unwrap(); // a root built afterwards
    }
    for slot in [4usize, 9] {
        assert_eq!(mmu::translate(slot, low + 8).map(|t| t.0), Some(low + 8), "slot {slot}");
        assert_eq!(mmu::translate(slot, high + 0x2004).map(|t| t.0), Some(high + 0x2004), "slot {slot}");
    }
    let (_, d) = mmu::translate(9, low).unwrap();
    assert_eq!(d & 0xFFF0_0000_0000_0FFF, mmu::page_bits(PageFlags::UserRoNx) & 0xFFF0_0000_0000_0FFF);
    // The same request again is fine; another attribute on a valid page is
    // refused (stage 1 never rewrites a valid entry).
    // SAFETY: under the lock.
    unsafe {
        assert_eq!(mmu::map_shared(low, 0x1000, PageFlags::UserRoNx, false), Ok(()));
        assert_eq!(mmu::map_shared(low, 0x1000, PageFlags::UserRwNx, false), Err(MapError::Conflict));
        assert_eq!(
            mmu::map_shared(ST_START, 0x1000, PageFlags::UserRoNx, false),
            Err(MapError::Conflict),
            "a pool block is not re-published with other rights"
        );
    }
}

// ── the BDOS side ───────────────────────────────────────────────────────

#[test]
fn the_tpa_block_starts_on_a_page_and_is_an_ordinary_block() {
    let _g = lock();
    let before = pool_stats(PoolKind::Stram);
    // Misalign the pool's next free byte first.
    // SAFETY: under the lock (the BDOS lock's stand-in).
    let small = unsafe { traps::mxalloc(48, MX_STRAM) };
    assert_ne!(small, 0);
    let tpa = unsafe { traps::mxalloc_aligned(4 * 1024 * 1024, MX_STRAM, 4096) };
    assert_ne!(tpa, 0);
    assert_eq!(tpa % 4096, 0, "page-aligned TPA");
    assert!((tpa as u64) >= ST_START && (tpa as u64) < ST_START + ST_LEN);
    // Exactly the TPA is allocated: the slack went back to the pool.
    let mid = pool_stats(PoolKind::Stram);
    assert_eq!(mid.used_bytes, before.used_bytes + 48 + 4 * 1024 * 1024);
    // The shell idiom works on it, and so does freeing it by its address.
    assert_eq!(unsafe { traps::mshrink(0, tpa, 0x4_0000) }, E_OK);
    assert_eq!(unsafe { traps::mfree(tpa) }, E_OK);
    assert_eq!(unsafe { traps::mfree(small) }, E_OK);
    let after = pool_stats(PoolKind::Stram);
    assert_eq!(after.used_bytes, before.used_bytes);
    assert_eq!(after.free_blocks, before.free_blocks, "the fragments coalesced again");
}

/// A process the host can "run": a basepage of its own in a claimed slot.
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

fn gemdos(func: u64, a: [u64; 3]) -> Long {
    let args = TrapArgs { regs: [a[0], a[1], a[2], 0, 0, 0, 0, 0], func, svc_imm: 1, space: tos_api::uaccess::Space::Kernel };
    match func {
        0x44 => tos_api::gemdos::memory::mxalloc(&args),
        0x48 => tos_api::gemdos::memory::malloc(&args),
        0x49 => tos_api::gemdos::memory::mfree(&args),
        0x4A => tos_api::gemdos::memory::mshrink(&args),
        _ => unreachable!(),
    }
}

#[test]
fn an_el0_malloc_returns_the_pool_block_itself() {
    let _g = lock();
    let p = Proc::new(PF_TTRAMMEM);
    // `Malloc(-1)`: the largest free block of the pool PF_TTRAMMEM selects
    // (MX_PREFTTRAM: the larger of both) — no window bound any more.
    let largest = gemdos(0x48, [u64::MAX, 0, 0]);
    let tt = pool_stats(PoolKind::Ttram).largest_free;
    let st = pool_stats(PoolKind::Stram).largest_free;
    assert_eq!(largest, tt.max(st));
    // `Malloc(n)` is `m_start`: a TT-RAM pool address, below 2 GiB (I2), and
    // it is an allocated block, live for the AES' reader.
    let a = gemdos(0x48, [1000, 0, 0]);
    assert!((a as u64) >= TT_START && (a as u64) < TT_START + TT_LEN, "a = 0x{a:x}");
    assert_eq!(usermem::resolve_kva(p.slot, a as u64 + 999), Some(a as u64 + 999));
    // `Mxalloc(-1, ST)` and an ST block.
    assert_eq!(gemdos(0x44, [u64::MAX, MX_STRAM as u64, 0]), pool_stats(PoolKind::Stram).largest_free);
    let s = gemdos(0x44, [64, MX_STRAM as u64, 0]);
    assert!((s as u64) >= ST_START && (s as u64) < ST_START + ST_LEN);
    let t = gemdos(0x44, [64, MX_TTRAM as u64, 0]);
    assert!((t as u64) >= TT_START);
    // `Mshrink` and `Mfree` take the same address back.
    assert_eq!(gemdos(0x4A, [0, a as u64, 100]), E_OK);
    assert_eq!(gemdos(0x49, [a as u64, 0, 0]), E_OK);
    assert_eq!(usermem::resolve_kva(p.slot, a as u64), None, "freed pool memory is no live user memory");
    assert_eq!(gemdos(0x49, [a as u64, 0, 0]), EIMBA, "double free");
    assert_eq!(gemdos(0x49, [s as u64, 0, 0]), E_OK);
    assert_eq!(gemdos(0x4A, [0, t as u64, 0]), E_OK, "Mshrink to 0 frees");
    // Page 0 resolves to the process's own page.
    let p0 = mmu::page0_kva(p.slot).unwrap() as u64;
    assert_eq!(usermem::resolve_kva(p.slot, 0x484), Some(p0 + 0x484));
}

#[test]
fn the_running_process_is_its_pd_pointer() {
    let _g = lock();
    let p = Proc::new(0);
    // `RUN` is the PD pointer; the kernel reads the basepage through its
    // alias (identity on the host); `slot_for` finds the slot from it.
    assert_eq!(process::current(), process::pd_of(p.slot));
    assert_eq!(process::current_kva(), p.bp);
    assert_eq!(process::slot_for(process::current()), Some(p.slot));
    assert_eq!(process::current_page0_kva(), mmu::page0_kva(p.slot));
    // New blocks are owned by that PD (`m_own = run`).
    assert_eq!(traps::current_owner(), process::current().cast());
}
