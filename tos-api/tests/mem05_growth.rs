//! MEM05 — the MD supply grows page by page and every page counts against
//! one budget (design `docs/analysis/mem05-design-2026-10-13.md` §2.2, §2.5,
//! tests 1, 3, 4, 8; user decision B in `TASK/MEM05.md`).
//!
//! * When every MD of the pages held is in use, `xmgetmd` takes exactly one
//!   more page; MDs already handed out never move.
//! * A page the source refuses is the old exhaustion path: `Malloc` 0,
//!   `Mshrink` `ERROR`, `Maddalt` `ENSMEM`; `Malloc(-1)` still answers; an
//!   exact fit needs no MD and still succeeds; with pages available again
//!   allocation goes on.
//! * The budget refuses like the source does.
//! * Many interleaved `Malloc`/`Mfree` rounds give every MD back and take no
//!   page beyond the first round's peak.
//!
//! Memory protection is off (16-byte grain); the pools are bookkeeping only.
//! The page source is a capped heap source this binary controls.

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::mdpool::{self, xmfremd_global, xmgetmd_global, MDS_PER_PAGE};
use tos_api::bdos::mem::pages::{self, MdPageSource, MD_PAGE_BUDGET, PAGE_BYTES};
use tos_api::bdos::mem::{init, owner_limit, traps};
use tos_api::bdos::process;
use tos_types::error::{E_OK, ENSMEM, ERROR};
use tos_types::{Long, MD, MX_STRAM};

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

/// A heap page source that gives at most `LEFT` more pages.
struct Capped;
static LEFT: AtomicUsize = AtomicUsize::new(usize::MAX / 2);
static CAPPED: Capped = Capped;

fn layout(n: usize) -> Layout {
    Layout::from_size_align(n * PAGE_BYTES, 16).unwrap()
}

impl MdPageSource for Capped {
    fn alloc_pages(&self, n: usize) -> *mut u8 {
        if LEFT.load(Ordering::Relaxed) < n {
            return std::ptr::null_mut();
        }
        LEFT.fetch_sub(n, Ordering::Relaxed);
        // SAFETY: non-zero size.
        unsafe { alloc_zeroed(layout(n)) }
    }
    unsafe fn free_pages(&self, p: *mut u8, n: usize) {
        LEFT.fetch_add(n, Ordering::Relaxed);
        // SAFETY: from `alloc_pages(n)`.
        unsafe { dealloc(p, layout(n)) };
    }
}

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any page or pool exists.
        unsafe {
            pages::set_source(&CAPPED);
            init::init(0x0040_0000, 10 << 20, 0x0100_0000, 64 << 20).expect("pools");
        }
    });
    LEFT.store(usize::MAX / 2, Ordering::Relaxed);
    pages::set_budget(MD_PAGE_BUDGET);
    process::set_current(std::ptr::null_mut()); // the kernel: no per-process limit
    g
}

fn malloc(n: Long) -> Long {
    // SAFETY: under the lock.
    unsafe { traps::mxalloc(n, MX_STRAM) }
}

fn mfree(a: Long) -> Long {
    // SAFETY: under the lock.
    unsafe { traps::mfree(a) }
}

/// MDs obtainable without a new page.
fn spare() -> usize {
    let s = mdpool::stats();
    s.capacity - s.in_use
}

// ── test 1: growth ───────────────────────────────────────────────────────

#[test]
fn a_page_is_taken_only_when_every_md_is_in_use_and_mds_never_move() {
    let _g = lock();
    let s0 = mdpool::stats();
    let held0 = pages::stats().md;
    let n = spare();
    let mut mds: Vec<*mut MD> = Vec::new();
    for i in 0..n {
        // SAFETY: under the lock.
        let md = unsafe { xmgetmd_global() };
        assert!(!md.is_null());
        // SAFETY: a fresh MD of ours.
        unsafe { (*md).m_start = i as Long + 1 };
        mds.push(md);
    }
    assert_eq!(mdpool::stats().pages, s0.pages, "the MDs held suffice for {n}");
    assert_eq!(spare(), 0);
    // One more: exactly one page more.
    // SAFETY: under the lock.
    let extra = unsafe { xmgetmd_global() };
    assert!(!extra.is_null());
    let s1 = mdpool::stats();
    assert_eq!(s1.pages, s0.pages + 1);
    assert_eq!(s1.capacity, s0.capacity + MDS_PER_PAGE);
    assert_eq!(pages::stats().md, held0 + 1, "booked against the budget as an MD page");
    assert_eq!(MDS_PER_PAGE, 117, "39 blocks of 3 MDs per 4-KiB page");
    mds.push(extra);
    let mut uniq = mds.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), mds.len(), "distinct MDs");
    // Growing moved nothing.
    for (i, &md) in mds[..n].iter().enumerate() {
        // SAFETY: still ours.
        assert_eq!(unsafe { (*md).m_start }, i as Long + 1);
    }
    for md in mds {
        // SAFETY: under the lock; each once.
        unsafe { xmfremd_global(md) };
    }
    assert_eq!(mdpool::stats().in_use, s0.in_use);
}

// ── test 3: a refused page ───────────────────────────────────────────────

#[test]
fn a_refused_page_is_the_old_exhaustion_path() {
    let _g = lock();
    let refused0 = pages::stats().refused_source;
    LEFT.store(0, Ordering::Relaxed);
    let big = malloc(4096);
    assert_ne!(big, 0);
    let mut blocks = vec![big];
    loop {
        let a = malloc(16);
        if a == 0 {
            break;
        }
        blocks.push(a);
        assert!(blocks.len() < 100_000, "the supply must run out without pages");
    }
    assert_eq!(spare(), 0, "every MD held is in use");
    assert!(pages::stats().refused_source > refused0, "the source said no");
    // SAFETY: under the lock.
    assert!(unsafe { traps::mxalloc(-1, MX_STRAM) } > 0, "Malloc(-1) still reports the largest free block");
    // SAFETY: under the lock; the tail needs an MD.
    assert_eq!(unsafe { traps::mshrink(0, big, 16) }, ERROR, "Mshrink: no MD for the tail");
    // SAFETY: under the lock; a region next to nothing needs an MD (never touched).
    assert_eq!(unsafe { traps::maddalt(0x0600_0000, 0x0010_0000) }, ENSMEM);
    // An exact fit takes a free block whole: no MD needed.
    let mid = blocks[blocks.len() / 2];
    assert_eq!(mfree(mid), E_OK);
    assert_eq!(malloc(16), mid, "the freed block is reused whole");
    // Pages again: allocation goes on.
    LEFT.store(usize::MAX / 2, Ordering::Relaxed);
    let p0 = mdpool::stats().pages;
    let more = malloc(16);
    assert_ne!(more, 0);
    assert_eq!(mdpool::stats().pages, p0 + 1);
    blocks.push(more);
    for a in blocks {
        assert_eq!(mfree(a), E_OK);
    }
    // SAFETY: under the lock.
    assert!(unsafe { owner_limit::audit() }.is_ok());
}

// ── test 4: the budget ───────────────────────────────────────────────────

#[test]
fn the_budget_refuses_like_the_source() {
    let _g = lock();
    let st = pages::stats();
    pages::set_budget(st.held);
    let mut blocks = Vec::new();
    loop {
        let a = malloc(16);
        if a == 0 {
            break;
        }
        blocks.push(a);
        assert!(blocks.len() < 100_000, "the budget must stop the supply");
    }
    let st1 = pages::stats();
    assert_eq!(st1.held, st.held, "not one page beyond the budget");
    assert!(st1.refused_budget > st.refused_budget);
    assert_eq!(spare(), 0);
    pages::set_budget(MD_PAGE_BUDGET);
    let more = malloc(16);
    assert_ne!(more, 0);
    assert_eq!(pages::stats().held, st.held + 1);
    blocks.push(more);
    for a in blocks {
        assert_eq!(mfree(a), E_OK);
    }
    // The budget is clamped to MD_PAGE_BUDGET.
    pages::set_budget(MD_PAGE_BUDGET * 4);
    assert_eq!(pages::stats().budget, MD_PAGE_BUDGET);
}

// ── test 8: clean-up ─────────────────────────────────────────────────────

#[test]
fn interleaved_rounds_give_every_md_back_and_leak_no_page() {
    let _g = lock();
    let in_use0 = mdpool::stats().in_use;
    let mut peak_pages = None;
    for round in 0..5u32 {
        let mut live = Vec::new();
        for i in 0..900u32 {
            let a = malloc(16 + (i % 5) as Long * 16);
            assert_ne!(a, 0);
            live.push(a);
            if i % 3 == round % 3 {
                // Free an older one: holes, coalescing, MDs back and forth.
                let k = (i as usize * 7) % live.len();
                assert_eq!(mfree(live.swap_remove(k)), E_OK);
            }
        }
        for a in live {
            assert_eq!(mfree(a), E_OK);
        }
        assert_eq!(mdpool::stats().in_use, in_use0, "round {round}: every MD back");
        // SAFETY: under the lock.
        assert!(unsafe { owner_limit::audit() }.is_ok());
        let p = mdpool::stats().pages;
        match peak_pages {
            None => peak_pages = Some(p),
            Some(q) => assert_eq!(p, q, "round {round}: no page beyond the first round's"),
        }
    }
}
