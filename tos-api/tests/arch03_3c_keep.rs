//! ARCH03 stage 3c — `Mxalloc(…, F_KEEP | mode)`: FreeMiNT's "don't
//! auto-free this memory" bit (`sys/mint/mem.h:104-105`).
//!
//! FreeMiNT semantics, checked against the real pool and ownership table:
//!
//! * the bit sets `M_KEEP` on the region (`sys/dosmem.c:131-135`); it is not
//!   part of the protection mode (`:237-242`);
//! * when the terminating process holds the **last** link, the region is not
//!   freed but attached to the root process (`sys/proc_help.c:242-258`) — in
//!   NewTOS the kernel owner; its protection mode stays;
//! * a block with a further owner simply loses the dying owner (links > 1);
//! * once the system holds it, **any** process may `Mfree` it by address
//!   (`sys/dosmem.c:365-398`); before that only an owner may (E15);
//! * an owner's own `Mfree` frees it like any block.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::{free_all_owned, region, traps};
use tos_api::bdos::process::mmu;
use tos_api::bdos::process::{self, memprot, PROCESS_TABLE};
use tos_api::trap::TrapArgs;
use tos_api::uaccess::Space;
use tos_types::error::{EIMBA, E_OK};
use tos_types::{BasePage, Long, ProtMode, F_KEEP, MX_TTRAM};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024;
const ARENA_TABLES: usize = 512;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

#[repr(C, align(4096))]
struct Page([u8; 4096]);

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        let arena: Vec<Page> = (0..ARENA_TABLES).map(|_| Page([0; 4096])).collect();
        let arena = Box::leak(arena.into_boxed_slice());
        // SAFETY: once per binary, before the pools exist.
        unsafe {
            memprot::configure(true, false);
            mmu::set_table_arena(arena.as_mut_ptr() as u64, ARENA_TABLES);
            tos_api::bdos::mem::init::init(ST_START, ST_LEN, TT_START, TT_LEN).expect("pools");
            mmu::map_pools().expect("the pools map");
        }
    });
    g
}

struct Proc {
    slot: usize,
    bp: *mut BasePage,
}

impl Proc {
    fn new() -> Proc {
        let slot = process::allocate_slot().expect("a slot");
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        let p = &PROCESS_TABLE[slot];
        p.basepage_kva.store(bp, Ordering::Release);
        // SAFETY: under the lock; the slot is not an installed root.
        unsafe { mmu::init_slot_tables(slot) }.unwrap();
        p.l0_pa.store(mmu::l0_pa_for(slot), Ordering::Release);
        p.set_state(process::State::Running);
        // SAFETY: under the lock.
        unsafe { memprot::on_root_built(slot) };
        Proc { slot, bp }
    }

    fn pd(&self) -> *mut core::ffi::c_void {
        process::pd_of(self.slot).cast()
    }

    fn alloc(&self, n: Long, mode: i16) -> u64 {
        process::set_current(process::pd_of(self.slot));
        // SAFETY: under the lock.
        let a = unsafe { traps::mxalloc(n, mode) };
        process::set_current(std::ptr::null_mut());
        assert_ne!(a, 0, "allocation");
        a as u32 as u64
    }

    /// GEMDOS `Mfree` as this process (the owner rule of E15 applies).
    fn mfree(&self, addr: u64) -> Long {
        process::set_current(process::pd_of(self.slot));
        let args = TrapArgs { regs: [addr, 0, 0, 0, 0, 0, 0, 0], func: 0x49, svc_imm: 1, space: Space::User };
        let rc = tos_api::gemdos::memory::mfree(&args);
        process::set_current(std::ptr::null_mut());
        rc
    }

    /// `Pterm`'s memory sweep for this process.
    fn terminate(&self) {
        // SAFETY: under the lock.
        unsafe { free_all_owned::free_all_owned(self.pd()) };
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        self.terminate();
        process::free_slot(self.slot);
        // SAFETY: allocated by `Box::new` in `new`.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

fn allocated(addr: u64) -> bool {
    // SAFETY: under the lock (callers hold it).
    unsafe { traps::is_allocated(addr) }
}

fn access(slot: usize, va: u64) -> Option<u64> {
    mmu::translate(slot, va).map(|(_, d)| (d >> 6) & 0b11)
}

const RW: Option<u64> = Some(0b01);

#[test]
fn a_kept_block_outlives_its_owner_and_anybody_may_free_it_then() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let kept = a.alloc(0x1000, MX_TTRAM | F_KEEP);
    let kept_g = a.alloc(0x1000, MX_TTRAM | F_KEEP | 0x20);
    let plain = a.alloc(0x1000, MX_TTRAM);
    // SAFETY: under the lock.
    unsafe {
        assert!(region::is_kept(kept as Long) && region::is_kept(kept_g as Long));
        assert!(!region::is_kept(plain as Long));
        // F_KEEP is not part of the mode: the PRGFLAGS default / PROT_G.
        assert_eq!(memprot::mode_of(kept), ProtMode::P);
        assert_eq!(memprot::mode_of(kept_g), ProtMode::G);
    }
    // While its owner lives, only an owner frees it (E15).
    assert_eq!(b.mfree(kept), EIMBA);
    assert!(allocated(kept));

    // The owner terminates: the plain block goes, the kept ones stay —
    // the system holds them, their modes unchanged.
    a.terminate();
    assert!(!allocated(plain), "an ordinary block is freed with its owner");
    assert!(allocated(kept) && allocated(kept_g), "F_KEEP blocks are not");
    // SAFETY: under the lock.
    unsafe {
        assert_eq!(region::links(kept as Long), 1);
        assert!(region::is_owner(kept as Long, traps::kernel_owner()), "inherited by the system");
        assert_eq!(memprot::mode_of(kept), ProtMode::P);
    }
    // Rights follow the mode: private memory of the system is nobody's,
    // global memory everybody's.
    assert_eq!(access(b.slot, kept), None);
    assert_eq!(access(b.slot, kept_g), RW);

    // Now anybody may free them by address.
    assert_eq!(b.mfree(kept), E_OK);
    assert_eq!(b.mfree(kept_g), E_OK);
    assert!(!allocated(kept) && !allocated(kept_g));
    // SAFETY: under the lock.
    assert!(!unsafe { region::is_kept(kept as Long) }, "the flag goes with the block");
    assert_eq!(b.mfree(kept), EIMBA, "and it is gone");
}

#[test]
fn an_owner_frees_a_kept_block_like_any_other() {
    let _g = lock();
    let a = Proc::new();
    let kept = a.alloc(0x1000, MX_TTRAM | F_KEEP);
    assert_eq!(a.mfree(kept), E_OK);
    assert!(!allocated(kept));
    // A later block at the same address is not kept.
    let again = a.alloc(0x1000, MX_TTRAM);
    assert_eq!(again, kept, "first fit reuses the pages");
    // SAFETY: under the lock.
    assert!(!unsafe { region::is_kept(again as Long) });
    a.terminate();
    assert!(!allocated(again));
}

#[test]
fn a_kept_block_with_a_further_owner_passes_to_it_first() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let kept = a.alloc(0x1000, MX_TTRAM | F_KEEP);
    // SAFETY: under the lock.
    unsafe { region::attach(kept as Long, b.pd()) }.unwrap();
    // `a` dies with another link left: the block is simply b's (links > 1,
    // FreeMiNT only decrements).
    a.terminate();
    // SAFETY: under the lock.
    unsafe {
        assert!(region::is_owner(kept as Long, b.pd()));
        assert!(!region::is_owner(kept as Long, traps::kernel_owner()));
    }
    // `b` dies as the last owner: now the system inherits it.
    b.terminate();
    assert!(allocated(kept));
    // SAFETY: under the lock.
    assert!(unsafe { region::is_owner(kept as Long, traps::kernel_owner()) });
    let c = Proc::new();
    assert_eq!(c.mfree(kept), E_OK);
    assert!(!allocated(kept));
}
