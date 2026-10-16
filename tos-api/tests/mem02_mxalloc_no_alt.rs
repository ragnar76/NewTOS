//! MEM02 — a machine without alternate RAM (`pmdalt` empty): `MX_TTRAM`
//! fails instead of handing out ST-RAM, `MX_PREFTTRAM` takes ST-RAM.
//! EmuTOS `xmxalloc`, `references/emutos/bdos/umem.c:297-299` (`ffit` on an
//! empty `pmdalt` -> NULL) and `:308-314`. Own binary: the pools are seeded
//! once per process (`init::init`).

use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::trap::TrapArgs;
use tos_types::error::E_OK;
use tos_types::{Long, MX_PREFSTRAM, MX_PREFTTRAM, MX_STRAM, MX_TTRAM};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any allocation.
        unsafe { tos_api::bdos::mem::init::init(ST_START, ST_LEN, 0, 0) }.expect("ST only");
    });
    g
}

fn call(func: u64, a: u64, b: u64) -> Long {
    let args = TrapArgs {
        regs: [a, b, 0, 0, 0, 0, 0, 0],
        func,
        svc_imm: 1,
        space: tos_api::uaccess::Space::Kernel,
    };
    match func {
        0x44 => tos_api::gemdos::memory::mxalloc(&args),
        0x49 => tos_api::gemdos::memory::mfree(&args),
        _ => unreachable!(),
    }
}

#[test]
fn without_alternate_ram_mx_ttram_fails_and_the_preference_takes_st() {
    let _g = lock();
    let st = pool_stats(PoolKind::Stram).largest_free;
    assert_eq!(call(0x44, u64::MAX, MX_TTRAM as u64), 0);
    assert_eq!(call(0x44, u64::MAX, MX_PREFTTRAM as u64), st);
    assert_eq!(
        call(0x44, 0x1000, MX_TTRAM as u64),
        0,
        "forced TT with no TT pool: refused, not ST"
    );
    let p = call(0x44, 0x1000, MX_PREFTTRAM as u64);
    assert!(
        (p as u64) >= ST_START && (p as u64) < ST_START + ST_LEN,
        "p = 0x{p:x}"
    );
    let q = call(0x44, 0x1000, MX_PREFSTRAM as u64);
    assert!((q as u64) >= ST_START && (q as u64) < ST_START + ST_LEN);
    let r = call(0x44, 0x1000, MX_STRAM as u64);
    assert!((r as u64) >= ST_START && (r as u64) < ST_START + ST_LEN);
    for a in [p, q, r] {
        assert_eq!(call(0x49, a as u64, 0), E_OK);
    }
}
