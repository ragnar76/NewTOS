//! Post-E9 point 3 — what `free_accs` (kernel `accs::free_accs`) relies on:
//! dropping an accessory's PD as owner gives back **all** of its memory —
//! the TPA its loader shrank (EmuTOS `pgmld`), its environment and the
//! blocks it allocated itself (its `Malloc`'ed start-up stack) — while a
//! block another process also holds stays allocated for that owner.
//!
//! EmuTOS (`references/emutos`, pin `1a7484ab`): `free_accs`
//! (`aes/geminit.c:296-303`) `dos_free`s each accessory's TPA, and the end
//! of the `run_accs_and_desktop` process frees everything else it owned
//! (`ixterm` → `free_all_owned(r, &pmd)` / `&pmdalt`, `bdos/proc.c:133-139`;
//! `Pexec(PE_LOAD)` makes `run` the owner of TPA and environment,
//! `:305-307`). NewTOS reaches the same end state with
//! `free_all_owned(accessory PD)` (the accessory owns its TPA, environment
//! and `Malloc`s itself).
//!
//! The pools are bookkeeping only (never dereferenced on the host).

use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::free_all_owned::free_all_owned;
use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::mem::{init, region, traps};
use tos_api::bdos::process::arena;
use tos_types::error::E_OK;
use tos_types::{Long, MX_TTRAM, PF_STANDARD};

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

/// An accessory as `load_one_acc` leaves it and as it leaves itself after
/// its first run: environment, TPA shrunk to the image, a 64-KiB stack it
/// `Malloc`ed (mintlib `_acc_main`). Returns (PD, env, stack).
///
/// # Safety
/// Under the test lock (the BDOS lock's stand-in).
unsafe fn an_accessory(image_len: Long) -> (Long, Long, Long) {
    // SAFETY: under the lock.
    unsafe {
        let env = traps::mxalloc(0x100, MX_TTRAM);
        assert_ne!(env, 0);
        let tpa = arena::alloc_tpa(PF_STANDARD, image_len as u64).expect("a TPA");
        let pd = tpa.base as Long;
        let me = pd as u64 as *mut core::ffi::c_void;
        assert!(traps::set_owner(pd, me));
        assert!(traps::set_owner(env, me));
        // `pgmld`'s Mshrink (util/gemdos.c:40-44).
        assert_eq!(traps::mshrink(0, pd, image_len), E_OK);
        // The accessory's own Malloc (its stack).
        let stack = traps::mxalloc(0x1_0000, MX_TTRAM);
        assert_ne!(stack, 0);
        assert!(traps::set_owner(stack, me));
        (pd, env, stack)
    }
}

#[test]
fn freeing_an_accessory_by_its_pd_returns_tpa_environment_and_its_mallocs() {
    let _g = lock();
    let tt0 = pool_stats(PoolKind::Ttram);
    // SAFETY: under the lock.
    let (pd, env, stack) = unsafe { an_accessory(0x6000) };
    assert!(pool_stats(PoolKind::Ttram).used_bytes > tt0.used_bytes);
    // SAFETY: under the lock.
    let freed = unsafe { free_all_owned(pd as u64 as *mut core::ffi::c_void) };
    assert_eq!(freed, 3, "TPA, environment, stack");
    let tt1 = pool_stats(PoolKind::Ttram);
    assert_eq!(tt1.used_bytes, tt0.used_bytes, "nothing of it left allocated");
    assert_eq!(tt1.largest_free, tt0.largest_free, "coalesced back");
    assert_eq!(tt1.free_blocks, tt0.free_blocks);
    // Every block is gone: freeing again is EIMBA.
    for b in [pd, env, stack] {
        // SAFETY: under the lock.
        assert_ne!(unsafe { traps::mfree(b) }, E_OK);
    }
}

#[test]
fn a_second_session_finds_the_same_memory_as_the_first() {
    // What the ACCFREE QEMU probe measures: load, free, load again -> the
    // accessory lands at the same place and the largest block is unchanged.
    let _g = lock();
    // SAFETY: under the lock.
    let (pd1, _, _) = unsafe { an_accessory(0x4000) };
    let largest1 = pool_stats(PoolKind::Ttram).largest_free;
    // SAFETY: under the lock.
    unsafe { free_all_owned(pd1 as u64 as *mut core::ffi::c_void) };
    // SAFETY: under the lock.
    let (pd2, _, _) = unsafe { an_accessory(0x4000) };
    assert_eq!(pd2, pd1, "same place");
    assert_eq!(pool_stats(PoolKind::Ttram).largest_free, largest1, "no leak between sessions");
    // SAFETY: under the lock.
    unsafe { free_all_owned(pd2 as u64 as *mut core::ffi::c_void) };
}

#[test]
fn a_block_another_process_also_holds_stays_for_that_owner() {
    let _g = lock();
    let tt0 = pool_stats(PoolKind::Ttram);
    let other = 0x0dea_d000u64 as *mut core::ffi::c_void;
    // SAFETY: under the lock.
    unsafe {
        let (pd, _, _) = an_accessory(0x4000);
        let me = pd as u64 as *mut core::ffi::c_void;
        let shared = traps::mxalloc(0x2000, MX_TTRAM);
        assert!(traps::set_owner(shared, me));
        region::attach(shared, other).expect("a second owner (ARCH03 2d link)");
        free_all_owned(me);
        assert!(traps::is_allocated(shared as u64), "still held by the other owner");
        assert!(region::is_owner(shared, other));
        assert!(!region::is_owner(shared, me));
        free_all_owned(other);
        assert!(!traps::is_allocated(shared as u64));
    }
    assert_eq!(pool_stats(PoolKind::Ttram).used_bytes, tt0.used_bytes);
}
