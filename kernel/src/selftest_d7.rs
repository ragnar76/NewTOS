//! Phase 06 D7 — `Ptermres` (terminate-and-stay-resident) self-test.
//!
//! Mirrors the D6 `Pterm` test but the user binary exits via
//! `Ptermres(blkln, rc)` (GEMDOS 0x31) instead of `Pterm`. The
//! distinguishing behaviour: the process's `Mxalloc`-ed pool block is
//! **reserved** (kept resident, orphaned) rather than freed — the
//! classic TSR trick (EmuTOS `xtermres` / `reserve_blocks`,
//! `bdos/proc.c:624`).
//!
//! ## Pipeline
//!
//! 1. Hand-craft an ELF whose `.text` does `Mxalloc(0x100, MX_STRAM)`
//!    then `Ptermres(0x100, 0)`.
//! 2. Load + launch it (kernel-launch path, same as D6).
//! 3. After the round-trip, assert the **TSR signature** against the
//!    pre-test ST-RAM snapshot:
//!      * `free_bytes` is **lower** — the block stayed resident
//!        (a `Pterm` would have returned it to the free list).
//!      * `allocated_blocks` is back to baseline — the owning MD was
//!        unlinked from `mp_mal` (not leaked on the allocated list).
//!      * the slot is `Free` again and `RUN` is null — the process
//!        terminated and control returned to the kernel driver.
//!
//! The reserved bytes are intentionally lost from the pool for the
//! remainder of the boot (a real TSR holds memory until reboot); no
//! later self-test asserts the ST-RAM baseline, so this is benign.

use core::sync::atomic::Ordering;

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::process::loader::load_to_slot;
use tos_api::bdos::process::{set_current, slot, State, PROCESS_TABLE};

use crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
use crate::kprintln;

/// Process-table slot the D7 test claims. Slot 3 — slots 0/1/2 are
/// used by EC4 / D3 / D6 earlier in the boot (all freed by then, but
/// a dedicated slot keeps the test self-contained).
pub const D7_TEST_SLOT: usize = 3;

// ─── Hand-crafted minimal AArch64 ELF64 (288 bytes) ─────────────────
//
//   0x000..0x040 : Ehdr             (64)
//   0x040..0x078 : 1× Phdr PT_LOAD  (56)
//   0x078..0x100 : padding          (136)
//   0x100..0x120 : code             (32 bytes, 8 instructions)
//
// Code:
//   0x100  movz x0, #0x100   ; Mxalloc size = 256
//   0x104  movz x1, #0       ; mode = MX_STRAM
//   0x108  movz x8, #0x44    ; Mxalloc
//   0x10C  svc  #1           ; → x0 = MD start
//   0x110  movz x0, #0x100   ; blkln (keep-resident size; no-op here)
//   0x114  movz x1, #0       ; rc = 0
//   0x118  movz x8, #0x31    ; Ptermres
//   0x11C  svc  #1           ; never returns
/// Hand-crafted minimal AArch64 ELF64: `Mxalloc(256, MX_STRAM)` then
/// `Ptermres(256, 0)`. Loaded by the D7 self-test via `load_to_slot`.
#[rustfmt::skip]
pub static D7_TEST_ELF: [u8; 288] = [
    // ─── Ehdr (64 bytes) ─────────────────────────────────────────
    0x7F, b'E', b'L', b'F',
    2, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
    0x02, 0x00,              // e_type ET_EXEC
    0xB7, 0x00,              // e_machine EM_AARCH64
    0x01, 0x00, 0x00, 0x00,  // e_version
    // e_entry = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_phoff = 0x40
    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_shoff = 0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,  // e_flags
    0x40, 0x00,              // e_ehsize 64
    0x38, 0x00,              // e_phentsize 56
    0x01, 0x00,              // e_phnum 1
    0x40, 0x00,              // e_shentsize
    0x00, 0x00,              // e_shnum
    0x00, 0x00,              // e_shstrndx

    // ─── Phdr (56 bytes) ─────────────────────────────────────────
    0x01, 0x00, 0x00, 0x00,  // p_type PT_LOAD
    0x05, 0x00, 0x00, 0x00,  // p_flags PF_R | PF_X
    // p_offset = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_vaddr  = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_paddr  = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_filesz = 0x20 = 32
    0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_memsz  = 0x20
    0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_align  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // ─── Padding 0x78..0x100 (136 bytes) ─────────────────────────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,

    // ─── Code at 0x100 (32 bytes, 8 instructions) ────────────────
    0x00, 0x20, 0x80, 0xD2,  // movz x0, #0x100   (Mxalloc size = 256)
    0x01, 0x00, 0x80, 0xD2,  // movz x1, #0       (mode = MX_STRAM)
    0x88, 0x08, 0x80, 0xD2,  // movz x8, #0x44    (Mxalloc)
    0x21, 0x00, 0x00, 0xD4,  // svc  #1           → x0 = MD start
    0x00, 0x20, 0x80, 0xD2,  // movz x0, #0x100   (blkln — no-op)
    0x01, 0x00, 0x80, 0xD2,  // movz x1, #0       (rc = 0)
    0x28, 0x06, 0x80, 0xD2,  // movz x8, #0x31    (Ptermres)
    0x21, 0x00, 0x00, 0xD4,  // svc  #1           (never returns)
];

/// Run the D7 `Ptermres` self-test.
pub fn run_d7_ptermres_test() {
    kprintln!("");
    kprintln!("[d7] === Phase 06 D7 Ptermres test ===");

    // SAFETY: pre-test, single-CPU; slot dedicated to D7.
    let loaded = match unsafe { load_to_slot(&D7_TEST_ELF, D7_TEST_SLOT) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[d7] === FAIL: loader rejected blob: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[d7] loaded slot {}: entry_va=0x{:016x} l0_pa=0x{:016x}",
        loaded.slot, loaded.entry_va, loaded.l0_pa,
    );

    let proc_slot = slot(D7_TEST_SLOT).expect("slot index in range");
    proc_slot.set_state(State::Running);
    let basepage_kva = proc_slot.basepage_kva.load(Ordering::Acquire);
    set_current(basepage_kva);

    let pre = pool_stats(PoolKind::Stram);
    kprintln!(
        "[d7] pre :  ST-RAM free={} used={} free_blocks={} alloc_blocks={}",
        pre.free_bytes, pre.used_bytes, pre.free_blocks, pre.allocated_blocks,
    );

    kprintln!("[d7] eret → EL0 at entry_va …");
    // SAFETY: pre-test, single-CPU; the slot's parent-resume atomics
    // outlive the round-trip; the Ptermres handler reads them before
    // freeing the slot.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            loaded.entry_va,
            loaded.l0_pa,
            USER_STACK_TOP_EXCLUSIVE,
            &PROCESS_TABLE[loaded.slot].parent_frame,
        );
    }
    kprintln!("[d7] returned from EL0 via Ptermres");

    let post = pool_stats(PoolKind::Stram);
    kprintln!(
        "[d7] post:  ST-RAM free={} used={} free_blocks={} alloc_blocks={}",
        post.free_bytes, post.used_bytes, post.free_blocks, post.allocated_blocks,
    );

    // TSR signature: the Mxalloc'd block stayed resident (free_bytes
    // dropped) but its MD was unlinked from the allocated list.
    let block_resident = post.free_bytes < pre.free_bytes;
    let alloc_list_clean = post.allocated_blocks == pre.allocated_blocks;
    let slot_freed = proc_slot.read_state() == State::Free;
    let run_cleared = tos_api::bdos::process::current().is_null();

    kprintln!(
        "[d7]   block kept resident (free_bytes dropped):     {}",
        if block_resident { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d7]   allocated-list clean (MD reserved):           {}",
        if alloc_list_clean { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d7]   slot freed (state == Free):                   {}",
        if slot_freed { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d7]   RUN cleared (current() == null):              {}",
        if run_cleared { "OK" } else { "FAIL" },
    );

    if block_resident && alloc_list_clean && slot_freed && run_cleared {
        kprintln!("[d7] === D7 PTERMRES PASS ===");
    } else {
        kprintln!("[d7] === D7 PTERMRES FAIL ===");
    }
}
