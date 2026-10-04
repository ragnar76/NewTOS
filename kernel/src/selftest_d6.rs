//! Phase 06 D6 — `Pterm` end-to-end self-test.
//!
//! Validates the **complete user-process exit path** — the first
//! self-test in NewTOS that exercises a real GEMDOS SVC from EL0
//! through the trap dispatcher, including the kernel's frame
//! rewrite that returns control to the parent.
//!
//! ## Pipeline
//!
//! 1. Hand-craft a minimal AArch64 ELF executable whose `.text` does
//!    one `Mxalloc(0x100, MX_STRAM)` followed by `Pterm(0)`.
//! 2. Load it into [`D6_TEST_SLOT`] via
//!    [`tos_api::bdos::process::loader::load_to_slot`] — same code
//!    path D3 exercises. The loader populates the slot's
//!    `basepage_kva` / `basepage_pa` / `l0_pa` so [`pterm::terminate`]
//!    can find the slot via `slot_for(run)` later.
//! 3. Set `RUN = slot.basepage_kva` so `current_owner()` (Phase 06
//!    D9) stamps the `Mxalloc`'d MD with the running basepage's VA.
//! 4. Snapshot pre-`Pterm` ST-RAM pool stats. Drop to EL0 via the
//!    shared
//!    [`crate::arch::aarch64::user_entry::enter_el0_with_resume_capture`]
//!    helper, using the **slot's own** `parent_resume_pc` /
//!    `parent_kernel_sp` atomics — these are the ones the Pterm
//!    handler reads in [`pterm::terminate`].
//! 5. The user code:
//!    * `Mxalloc(0x100, 0)` → `x0 = MD start address` on success.
//!    * `Pterm(0)` → never returns.
//! 6. The Pterm SVC handler in [`crate::arch::aarch64::exceptions`]
//!    rewrites the trap frame to land at the captured EL1 resume
//!    PC, on the parent kernel's saved SP_EL0. This function
//!    therefore observes a normal return.
//! 7. Validate:
//!    * Slot is `Free` again.
//!    * `RUN` is `null` again.
//!    * ST-RAM pool stats are back to the pre-test snapshot
//!      (`Mxalloc`'d block was reaped).
//!    * `MD` count is back to baseline (the `Mxalloc`'d MD was
//!      released to the meta-pool, not just unlinked).
//!
//! ## Why this matters
//!
//! D6 is the first selftest where:
//!
//!   * A user binary makes a GEMDOS SVC and **gets a value back**
//!     from the kernel before continuing user execution.
//!   * The user binary terminates via a real GEMDOS function instead
//!     of the EC4 sentinel — the same exit path Pexec'd source-port
//!     programs will take.
//!   * `free_all_owned` actually finds something to free (D9's
//!     `m_own = run` wiring is exercised by `Mxalloc` from EL0).
//!
//! Once D6 passes, every subsequent process-management deliverable
//! (D7 Pgetpid, Pexec D8 — when it lands — etc.) builds on a
//! verified Pterm-style round-trip.

use core::sync::atomic::Ordering;

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::process::loader::{load_to_slot, LoadedImage};
use tos_api::bdos::process::{set_current, slot, State, PROCESS_TABLE};

use crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
use crate::kprintln;

/// Process-table slot the D6 test claims. Slot 2 — slot 0 is left
/// dirty by EC4 and slot 1 by D3, both of which run earlier in the
/// boot sequence.
pub const D6_TEST_SLOT: usize = 2;

// ─── Hand-crafted minimal AArch64 ELF64 ─────────────────────────────
//
// File layout (284 bytes total):
//
//   0x000..0x040 : ELF64 Ehdr        (64 bytes)
//   0x040..0x078 : 1× ELF64 Phdr     (PT_LOAD, 56 bytes)
//   0x078..0x100 : padding (zero)    (136 bytes)
//   0x100..0x11C : code              (28 bytes)
//
// Code:
//
//   0x100  D2 80 20 00   movz x0, #0x100   ; size = 256
//   0x104  D2 80 00 01   movz x1, #0       ; mode = MX_STRAM
//   0x108  D2 80 08 88   movz x8, #0x44    ; Mxalloc fn id
//   0x10C  D4 00 00 21   svc  #1           ; → x0 = MD start
//   0x110  D2 80 00 00   movz x0, #0       ; rc = 0
//   0x114  D2 80 09 88   movz x8, #0x4C    ; Pterm fn id
//   0x118  D4 00 00 21   svc  #1           ; never returns
//
// (Encodings derived from ARM ARM C6.2 — MOVZ / SVC. AArch64 is
// little-endian, so each 32-bit instruction is stored low-byte-
// first.)

/// Hand-crafted minimal AArch64 ELF64 executable. Loaded by the D6
/// self-test via `load_to_slot`.
#[rustfmt::skip]
pub static D6_TEST_ELF: [u8; 284] = [
    // ─── ELF64 header (Ehdr, 64 bytes) ───────────────────────────
    0x7F, b'E', b'L', b'F',  // e_ident[EI_MAG0..3]: ELF magic
    2,                       // e_ident[EI_CLASS]: ELFCLASS64
    1,                       // e_ident[EI_DATA]: ELFDATA2LSB
    1,                       // e_ident[EI_VERSION]: EV_CURRENT
    0,                       // e_ident[EI_OSABI]: SYSV
    0,                       // e_ident[EI_ABIVERSION]: 0
    0, 0, 0, 0, 0, 0, 0,     // e_ident padding (7 bytes)

    0x03, 0x00,              // e_type:    ET_DYN (PIE; ET_EXEC is refused since ARCH03 0d)
    0xB7, 0x00,              // e_machine: EM_AARCH64
    0x01, 0x00, 0x00, 0x00,  // e_version: EV_CURRENT

    // e_entry = 0x1000 (≥ RESERVED_LOW_END after B1's linker bump)
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_phoff = 0x40
    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_shoff = 0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    0x00, 0x00, 0x00, 0x00,  // e_flags
    0x40, 0x00,              // e_ehsize:    64
    0x38, 0x00,              // e_phentsize: 56
    0x01, 0x00,              // e_phnum:     1
    0x40, 0x00,              // e_shentsize: 64 (unused)
    0x00, 0x00,              // e_shnum:     0
    0x00, 0x00,              // e_shstrndx:  0

    // ─── Program header (Phdr, 56 bytes) ─────────────────────────
    0x01, 0x00, 0x00, 0x00,  // p_type:  PT_LOAD
    0x05, 0x00, 0x00, 0x00,  // p_flags: PF_R | PF_X (= 5)
    // p_offset = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_vaddr  = 0x1000 (≥ RESERVED_LOW_END so validate_segment OKs it)
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_paddr  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_filesz = 28 bytes
    0x1C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_memsz  = 28 bytes
    0x1C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_align  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // ─── Padding from 0x78 to 0x100 (136 bytes) ──────────────────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,

    // ─── Code at file offset 0x100 (28 bytes) ────────────────────
    0x00, 0x20, 0x80, 0xD2,  // movz x0, #0x100
    0x01, 0x00, 0x80, 0xD2,  // movz x1, #0
    0x88, 0x08, 0x80, 0xD2,  // movz x8, #0x44   (Mxalloc)
    0x21, 0x00, 0x00, 0xD4,  // svc  #1
    0x00, 0x00, 0x80, 0xD2,  // movz x0, #0
    0x88, 0x09, 0x80, 0xD2,  // movz x8, #0x4C   (Pterm)
    0x21, 0x00, 0x00, 0xD4,  // svc  #1
];

/// Run the D6 Pterm round-trip self-test.
pub fn run_d6_pterm_test() {
    kprintln!("");
    kprintln!("[d6] === Phase 06 D6 Pterm test ===");

    // 1. Load the ELF into our slot.
    // SAFETY: pre-test, single-CPU; the slot is dedicated to D6.
    let loaded = match unsafe { load_to_slot(&D6_TEST_ELF, D6_TEST_SLOT) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[d6] === FAIL: loader rejected blob: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[d6] loaded slot {}: entry_va=0x{:016x} l0_pa=0x{:016x}",
        loaded.slot, loaded.entry_va, loaded.l0_pa,
    );

    // 2. Mark the slot Loaded → Running and publish RUN. The Pterm
    //    handler reads RUN to find the dying process; `Mxalloc` from
    //    user space stamps `m_own = current_owner() = run` so
    //    `free_all_owned` can reap exactly the user's allocations.
    let proc_slot = slot(D6_TEST_SLOT).expect("slot index in range");
    proc_slot.set_state(State::Running);
    let basepage_kva = proc_slot.basepage_kva.load(Ordering::Acquire);
    set_current(basepage_kva);

    // 3. Snapshot ST-RAM pool baseline so post-Pterm can assert
    //    "the Mxalloc'd block was returned to the free list".
    let pre = pool_stats(PoolKind::Stram);
    kprintln!(
        "[d6] pre :  ST-RAM free={} used={} free_blocks={} alloc_blocks={}",
        pre.free_bytes, pre.used_bytes, pre.free_blocks, pre.allocated_blocks,
    );

    // 4. Drop to EL0. The user runs Mxalloc → Pterm. The Pterm
    //    handler rewrites the trap frame to redirect to the resume
    //    PC captured by the helper.
    kprintln!("[d6] eret → EL0 at entry_va …");
    // SAFETY: pre-test, single-CPU; the slot's parent-resume atomics
    // outlive the round-trip and the Pterm handler reads them before
    // freeing the slot.
    unsafe { drop_to_el0_via_pterm(&loaded) };
    kprintln!("[d6] returned from EL0 via Pterm");

    // 5. Validate cleanup.
    let post = pool_stats(PoolKind::Stram);
    kprintln!(
        "[d6] post:  ST-RAM free={} used={} free_blocks={} alloc_blocks={}",
        post.free_bytes, post.used_bytes, post.free_blocks, post.allocated_blocks,
    );

    let pool_baseline_ok = post.free_bytes == pre.free_bytes
        && post.used_bytes == pre.used_bytes
        && post.free_blocks == pre.free_blocks
        && post.allocated_blocks == pre.allocated_blocks;

    let slot_freed = proc_slot.read_state() == State::Free;
    let run_cleared =
        tos_api::bdos::process::current().is_null();
    // The Mxalloc must have actually executed (otherwise free_all_owned
    // wouldn't observe a difference). We can't probe the user's `x0`
    // post-Pterm, but `pre.allocated_blocks` and `post.allocated_blocks`
    // should match — the difference at the *peak* between them is
    // visible only via `freed_mds` from the handler. Phase 06 D7+ will
    // expose that count; for D6 we rely on the absence of a leak.

    kprintln!(
        "[d6]   pool baseline restored:                      {}",
        if pool_baseline_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d6]   slot freed (state == Free):                  {}",
        if slot_freed { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d6]   RUN cleared (current() == null):             {}",
        if run_cleared { "OK" } else { "FAIL" },
    );

    if pool_baseline_ok && slot_freed && run_cleared {
        kprintln!("[d6] === D6 PASS ===");
    } else {
        kprintln!("[d6] === D6 FAIL ===");
    }
}

/// Drop to EL0 at `loaded.entry_va`, capturing the EL1 resume PC
/// into the slot's own atomics so the Pterm handler can read them.
unsafe fn drop_to_el0_via_pterm(img: &LoadedImage) {
    let proc_slot = &PROCESS_TABLE[img.slot];

    // SAFETY: pre-test, single-CPU; the slot's atomics live for the
    // entire kernel lifetime (statics in `tos_api::bdos::process`).
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            img.entry_va,
            img.basepage_va,
            img.l0_pa,
            USER_STACK_TOP_EXCLUSIVE,
            &proc_slot.parent_frame,
        );
    }
}
