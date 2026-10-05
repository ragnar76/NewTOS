//! `Getmpb` (BIOS 0x00) — report the free ST-RAM region as a Memory
//! Partition Block.
//!
//! Spec: `docs/api/bios/00-getmpb.md`. EmuTOS fills a single static
//! memory descriptor `themd` describing `[membot, memtop)` and points the
//! caller's MPB free list at it (`bios/biosmem.c:134`): `mp_mfl = &themd`,
//! `mp_mal = NULL`, `mp_rover` left untouched; `themd` gets `m_link = NULL`,
//! `m_start = membot`, `m_length = memtop - membot`, `m_own = NULL`.
//!
//! NewTOS seeds its allocator directly from the AArch64 memory map, so
//! `Getmpb` is a **read-only** report of the ST-RAM pool extent (from
//! [`crate::bdos::mem::init::stram_range`]). Unlike EmuTOS it does **not**
//! rewrite the live BDOS free list — the "second call destroys your
//! allocator state" quirk is explicitly rejected (spec "NewTOS
//! implementation notes"), so a second call just re-reports the same
//! extent and is harmless.

use core::cell::UnsafeCell;

use tos_types::{Long, ProtMode, MD, MPB};

use crate::bdos::mem::init::stram_range;
use crate::bdos::process::{self, usermem};
use crate::trap::TrapArgs;

/// The single memory descriptor `Getmpb` reports, in a page of its own.
///
/// `Getmpb` publishes it **read-only into the calling process's address
/// space** (ADR-023) so the caller can walk the free list it was handed, and
/// the smallest thing an AArch64 translation table can grant is a whole
/// 4-KiB page. Padding the descriptor to exactly one page — and aligning it
/// to one — guarantees the page an EL0 program can read holds the MD and
/// nothing else of the kernel's.
#[repr(C, align(4096))]
struct ThemdPage {
    /// The descriptor itself, mirroring EmuTOS' `themd`
    /// (`bios/biosmem.c:144`).
    md: MD,
    /// Fill to a whole page so no other kernel datum shares it.
    _pad: [u8; 4096 - core::mem::size_of::<MD>()],
}

const _: () = assert!(
    core::mem::size_of::<ThemdPage>() == 4096,
    "the reported MD must own exactly one page — see `getmpb`'s EL0 publish",
);

/// The single static memory descriptor `Getmpb` reports. Re-filled on each
/// call from the current ST-RAM extent; the caller's MPB free list points
/// here.
struct ThemdCell(UnsafeCell<ThemdPage>);

// SAFETY: `THEMD` is only touched by the BIOS dispatcher while filling a
// caller's MPB. BIOS traps run on the boot/trap CPU under the same
// single-owner discipline as the BDOS `MpbCell`; the pointer fields make
// the auto `Sync` absent, so we assert it explicitly.
unsafe impl Sync for ThemdCell {}

static THEMD: ThemdCell = ThemdCell(UnsafeCell::new(ThemdPage {
    md: MD {
        m_link: core::ptr::null_mut(),
        m_start: 0,
        m_length: 0,
        m_own: core::ptr::null_mut(),
    },
    _pad: [0; 4096 - core::mem::size_of::<MD>()],
}));

/// `Getmpb` (0x00) — fill the caller-supplied MPB with a one-descriptor
/// free list covering the ST-RAM pool. `getmpb` is `void` in EmuTOS; the
/// result register is undefined, so we return `0` and callers read the
/// populated `*mpb`.
///
/// The free-list head the caller receives has to be a pointer the *caller*
/// can walk (spec `docs/api/bios/00-getmpb.md` §Outputs). For an in-kernel
/// (EL1) caller that is the descriptor's kernel VA; for an **EL0** caller
/// the kernel's high-half pages are unreachable, so the descriptor is
/// published read-only (FreeMiNT `PROT_PR`,
/// `references/freemint/sys/mint/mem.h:133`) into the shared user address
/// space at its own address (VA = PA, ARCH03 stage 1) and `mp_mfl` carries
/// that **user address** (ADR-023). If the publication cannot be made,
/// `mp_mfl` is left NULL — an empty free list, which the contract already
/// defines — rather than a pointer the caller would fault on.
pub fn getmpb(args: &TrapArgs) -> Long {
    let mpb = args.arg_u64(0) as *mut MPB;
    if mpb.is_null() {
        return 0;
    }
    let (start, end) = stram_range();
    let page = THEMD.0.get();
    // SAFETY: `page` is our stable static and the single boot/trap CPU is
    // the only accessor; taking the address of the `md` field is in-bounds.
    let md = unsafe { &raw mut (*page).md };

    // SAFETY: `md` is our stable static; `mpb` is the caller's writable
    // MPB (a kernel pointer at EL1, a user VA at EL0 — the same
    // direct-pointer contract as `Rwabs`). Single boot/trap CPU, so no
    // concurrent access to `THEMD`.
    unsafe {
        (*md).m_link = core::ptr::null_mut();
        (*md).m_start = start;
        (*md).m_length = end - start;
        (*md).m_own = core::ptr::null_mut();
    }

    // The free-list head, expressed in the caller's address space.
    let head = match process::slot_for(process::current()) {
        // EL1 caller: the kernel VA is readable.
        None => md,
        // SAFETY: BDOS lock held (single-CPU SVC context); `THEMD` is a
        // `'static` that owns its page, so publishing it read-only exposes
        // nothing else and it outlives every process holding the pointer.
        Some(slot) => match unsafe {
            usermem::expose(slot, md.cast(), core::mem::size_of::<MD>(), ProtMode::Pr)
        } {
            Some(user_va) => user_va as *mut MD,
            None => core::ptr::null_mut(),
        },
    };

    // SAFETY: see above — `mpb` is the caller's writable MPB.
    unsafe {
        // Write only mp_mfl / mp_mal; mp_rover is preserved, exactly as
        // EmuTOS leaves the caller's value (spec §Outputs).
        (*mpb).mp_mfl = head;
        (*mpb).mp_mal = core::ptr::null_mut();
    }
    0
}
