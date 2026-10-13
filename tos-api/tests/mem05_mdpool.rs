//! MEM05 — the MD meta-allocator (`xmgetmd` / `xmfremd`): O(1) free over
//! the slot index (EmuTOS `bdos/osmem.c:229-243`), design
//! `docs/analysis/mem05-design-2026-10-13.md` §2.3, test 2.
//!
//! The global supply is used directly; this binary seeds no pools, so every
//! MD it holds is one of these tests' (one test at a time, under the lock).

use std::sync::{Mutex, MutexGuard};

use tos_api::bdos::mem::mdpool::{self, xmfremd_global, xmgetmd_global};
use tos_types::MD;

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// A small deterministic permutation of `0..n` (LCG shuffle).
fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    let mut x = seed;
    for i in (1..n).rev() {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (x >> 33) as usize % (i + 1);
        v.swap(i, j);
    }
    v
}

fn get(n: usize) -> Vec<*mut MD> {
    (0..n)
        .map(|_| {
            // SAFETY: under the lock (stands in for the BDOS lock).
            let md = unsafe { xmgetmd_global() };
            assert!(!md.is_null());
            md
        })
        .collect()
}

#[test]
fn freeing_costs_one_step_per_md_in_any_order() {
    let _g = lock();
    let n = 200;
    let before = mdpool::stats();
    let mds = get(n);
    let mut uniq = mds.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), n, "distinct MDs");
    assert_eq!(mdpool::stats().in_use, before.in_use + n);
    let steps0 = mdpool::stats().free_steps;
    for i in shuffled(n, 7) {
        // SAFETY: under the lock; each MD once.
        unsafe { xmfremd_global(mds[i]) };
    }
    let s = mdpool::stats();
    assert_eq!(s.free_steps - steps0, n as u64, "O(1): one step per free, independent of position");
    assert_eq!(s.in_use, before.in_use);
    // All of them can be had again.
    let again = get(n);
    for md in again {
        // SAFETY: under the lock.
        unsafe { xmfremd_global(md) };
    }
    assert_eq!(mdpool::stats().in_use, before.in_use);
}

#[test]
fn a_wild_or_double_free_is_ignored() {
    let _g = lock();
    let before = mdpool::stats();
    let md = get(1)[0];
    // SAFETY: under the lock.
    unsafe {
        xmfremd_global(md);
        // The slot index is -1 now: the second free changes nothing
        // (EmuTOS: "i < 0 || i >= MDS_PER_BLOCK" → ignore).
        xmfremd_global(md);
        xmfremd_global(core::ptr::null_mut());
    }
    assert_eq!(mdpool::stats().in_use, before.in_use);
    // Still sound: the next two allocations are distinct.
    let two = get(2);
    assert_ne!(two[0], two[1]);
    for md in two {
        // SAFETY: under the lock.
        unsafe { xmfremd_global(md) };
    }
}
