//! ARCH03 stage 4 — the user tables with ASIDs **on** (own binary: the
//! switch is a global set once at boot).
//!
//! * every user leaf carries `nG = 1` (ARM ARM `RQGKGF`, D8-7854) — page 0,
//!   the pool blocks/pages, the published kernel pages; table descriptors
//!   stay without upper attribute bits (V0a invariant 1);
//! * a root install hands the slot an ASID (on the host the `msr` itself is
//!   compiled out; the bookkeeping runs), keeps it across re-installs, and a
//!   rebuilt root (`init_slot_tables`, a new tenant) gets a fresh one;
//! * with four numbers per generation (`asid=tiny`) the installs roll the
//!   generation over and every slot is re-assigned afterwards.

use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::process::mmu::{self, PageFlags};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 16 * 1024 * 1024 + 0x3000;
const NG: u64 = 1 << 11;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any table exists ("boot").
        unsafe { mmu::configure_asids(4, 0x7000) };
        // SAFETY: as above.
        unsafe { tos_api::bdos::mem::init::init(ST_START, ST_LEN, TT_START, TT_LEN) }.expect("the test pools");
        // SAFETY: as above.
        unsafe { mmu::map_pools() }.expect("the pools map");
    });
    g
}

#[test]
fn every_user_leaf_is_not_global() {
    let _g = lock();
    assert!(mmu::asids_enabled());
    for f in [PageFlags::UserRwExec, PageFlags::UserRoNx, PageFlags::UserRwNxUncached] {
        assert_eq!(mmu::page_bits(f) & NG, NG, "{f:?}");
    }
    // SAFETY: under the lock; nothing is installed on the host.
    unsafe { mmu::init_slot_tables(2) }.unwrap();
    // Page 0 and pool addresses (2-MiB block and 4-KiB page edges alike).
    for va in [0x0u64, 0xFF8, ST_START, ST_START + 0x1234, TT_START + TT_LEN - 1] {
        let (_, desc) = mmu::translate(2, va).unwrap_or_else(|| panic!("0x{va:x} unmapped"));
        assert_eq!(desc & NG, NG, "0x{va:x}: leaf 0x{desc:x} must be non-global");
    }
}

#[test]
fn installs_assign_keep_retire_and_roll_over() {
    let _g = lock();
    let base = mmu::asid_stats();
    assert_eq!(base.3, 4, "limit as configured");
    for slot in [5usize, 6, 7, 8] {
        // SAFETY: under the lock; host build (no system register access).
        unsafe { mmu::init_slot_tables(slot) }.unwrap();
    }
    let install = |slot: usize| {
        // SAFETY: under the lock; host build: bookkeeping only.
        unsafe { mmu::install_ttbr0(mmu::l0_pa_for(slot)) }
    };
    install(5);
    install(6);
    install(7);
    let s = mmu::asid_stats();
    assert_eq!(s.2 - base.2, 3, "three fresh ASIDs");
    install(5);
    install(6);
    assert_eq!(mmu::asid_stats().2, s.2, "re-installs keep their ASID");
    // A fourth root in this generation: none left -> rollover.
    install(8);
    let r = mmu::asid_stats();
    assert!(r.1 > s.1, "the generation rolled over");
    assert_eq!(r.0, s.0 + (r.1 - s.1), "one generation per rollover");
    // The old slots are stale: installing them hands out new numbers.
    install(5);
    assert_eq!(mmu::asid_stats().2, r.2 + 1);
    // A rebuilt root (new tenant) retires its ASID: the next install of the
    // same slot is a fresh assignment even within the generation.
    let before = mmu::asid_stats().2;
    // SAFETY: under the lock.
    unsafe { mmu::init_slot_tables(5) }.unwrap();
    install(5);
    assert_eq!(mmu::asid_stats().2, before + 1);
    // The empty (non-slot) root needs no ASID.
    let before = mmu::asid_stats();
    // SAFETY: host build.
    unsafe { mmu::install_ttbr0(0x7000) };
    assert_eq!(mmu::asid_stats(), before);
}
