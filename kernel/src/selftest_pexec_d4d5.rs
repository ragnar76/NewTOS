//! Phase 06 D4 + D5 — `Pexec(PE_LOAD)` then `Pexec(PE_GO)` round-trip.
//!
//! Exercises the classic shell idiom (load a program, then launch it)
//! on the per-process-TTBR0 model with the **opaque PD handle** of
//! ADR-010 option (b): `PE_LOAD` returns the child's `basepage_pa`
//! (slot-unique, `LONG`-representable), the parent passes it back in
//! `PE_GO`'s `tail`, and the kernel resolves it to the loaded slot and
//! launches it.
//!
//! ## Pipeline
//!
//! 1. Register the child ELF as `"C"` (shared with the D3 test).
//! 2. Load a parent ELF that runs:
//!    ```text
//!    h = Pexec(PE_LOAD, "C", 0, 0)   ; load only → opaque handle
//!    rc = Pexec(PE_GO,  0,  h, 0)    ; launch h → child's exit code
//!    Pterm(rc)
//!    ```
//! 3. Drop to EL0 in the parent. The child prints "Hello", `Pterm`s,
//!    control returns to the parent's `PE_GO` with `x0 = rc`, the
//!    parent `Pterm`s, and the kernel-launch path lands back here.
//! 4. Assert the BDOS pool baseline is restored and `RUN` is cleared —
//!    i.e. the whole load → go → child → Pterm → parent → Pterm chain
//!    completed and freed every slot.

use core::sync::atomic::Ordering;

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::process::loader::load_to_slot;
use tos_api::bdos::process::{allocate_slot, registry, set_current, slot, State, PROCESS_TABLE};

use crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
use crate::kprintln;
use crate::selftest_pexec::CHILD_ELF;

// ─── Parent ELF ─────────────────────────────────────────────────────
//
// File layout (514 bytes total):
//
//   0x000..0x040 : ELF64 Ehdr           (64 bytes)
//   0x040..0x078 : 1× ELF64 Phdr        (PT_LOAD, 56 bytes)
//   0x078..0x100 : padding              (136 bytes)
//   0x100..0x130 : code                 (48 bytes, 12 instructions)
//   0x130..0x200 : padding              (208 bytes)
//   0x200..0x202 : program name "C\0"   (2 bytes)
//
// Code (function ID in x8, args x0..x3, `svc #1`):
//   0x100  movz x0, #3          ; mode = PE_LOAD
//   0x104  movz x1, #0x200      ; path = &"C\0"
//   0x108  movz x2, #0          ; tail
//   0x10C  movz x3, #0          ; env
//   0x110  movz x8, #0x4B       ; Pexec
//   0x114  svc  #1              ; → x0 = opaque handle
//   0x118  mov  x2, x0          ; tail = handle
//   0x11C  movz x0, #4          ; mode = PE_GO
//   0x120  movz x8, #0x4B       ; Pexec
//   0x124  svc  #1              ; → x0 = child rc
//   0x128  movz x8, #0x4C       ; Pterm
//   0x12C  svc  #1              ; rc = x0
#[rustfmt::skip]
static PARENT_ELF: [u8; 514] = [
    // ─── Ehdr (64 bytes) ─────────────────────────────────────────
    0x7F, b'E', b'L', b'F',
    2, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
    0x02, 0x00,             // e_type ET_EXEC
    0xB7, 0x00,             // e_machine EM_AARCH64
    0x01, 0x00, 0x00, 0x00, // e_version
    // e_entry = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_phoff = 0x40
    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_shoff = 0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, // e_flags
    0x40, 0x00,             // e_ehsize 64
    0x38, 0x00,             // e_phentsize 56
    0x01, 0x00,             // e_phnum 1
    0x40, 0x00,             // e_shentsize 64
    0x00, 0x00,             // e_shnum
    0x00, 0x00,             // e_shstrndx

    // ─── Phdr (56 bytes) ─────────────────────────────────────────
    0x01, 0x00, 0x00, 0x00, // p_type PT_LOAD
    0x05, 0x00, 0x00, 0x00, // p_flags PF_R | PF_X
    // p_offset = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_vaddr  = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_paddr  = 0x100
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_filesz = 0x102 = 258 (covers code 0x100..0x130 + name 0x200..0x202)
    0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_memsz  = 0x102
    0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_align  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // ─── Padding 0x78..0x100 (136 bytes) ─────────────────────────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,

    // ─── Code at 0x100 (48 bytes, 12 instructions) ───────────────
    0x60, 0x00, 0x80, 0xD2, // movz x0, #3           (mode = PE_LOAD)
    0x01, 0x40, 0x80, 0xD2, // movz x1, #0x200       (path = "C\0")
    0x02, 0x00, 0x80, 0xD2, // movz x2, #0           (tail)
    0x03, 0x00, 0x80, 0xD2, // movz x3, #0           (env)
    0x68, 0x09, 0x80, 0xD2, // movz x8, #0x4B        (Pexec)
    0x21, 0x00, 0x00, 0xD4, // svc  #1               → x0 = handle
    0xE2, 0x03, 0x00, 0xAA, // mov  x2, x0           (tail = handle)
    0x80, 0x00, 0x80, 0xD2, // movz x0, #4           (mode = PE_GO)
    0x68, 0x09, 0x80, 0xD2, // movz x8, #0x4B        (Pexec)
    0x21, 0x00, 0x00, 0xD4, // svc  #1               → x0 = child rc
    0x88, 0x09, 0x80, 0xD2, // movz x8, #0x4C        (Pterm)
    0x21, 0x00, 0x00, 0xD4, // svc  #1               (rc = x0)

    // ─── Padding 0x130..0x200 (208 bytes = 6.5 × 32) ─────────────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,

    // ─── Program name "C\0" at 0x200 (2 bytes) ───────────────────
    b'C', 0,
];

/// Run the Pexec D4 (`PE_LOAD`) + D5 (`PE_GO`) round-trip test.
pub fn run_pexec_loadgo_split_test() {
    kprintln!("");
    kprintln!("[d4d5] === Phase 06 D4/D5 Pexec(PE_LOAD)+PE_GO test ===");

    // Register the child (idempotent w.r.t. correctness — the D3 test
    // also registers "C"; lookup returns the first match).
    registry::register(b"C", &CHILD_ELF);

    let pre = pool_stats(PoolKind::Stram);
    kprintln!(
        "[d4d5] pre :  ST-RAM free={} used={} alloc_blocks={}",
        pre.free_bytes, pre.used_bytes, pre.allocated_blocks,
    );

    let parent_slot_idx = match allocate_slot() {
        Some(i) => i,
        None => {
            kprintln!("[d4d5] === FAIL: allocate_slot returned None ===");
            return;
        }
    };
    // SAFETY: pre-test, single-CPU; slot just claimed.
    let parent_loaded = match unsafe { load_to_slot(&PARENT_ELF, parent_slot_idx) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[d4d5] === FAIL: parent loader: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[d4d5] parent loaded slot {}: entry=0x{:x} l0_pa=0x{:x}",
        parent_loaded.slot, parent_loaded.entry_va, parent_loaded.l0_pa,
    );

    let parent_slot_ref = slot(parent_slot_idx).expect("slot in range");
    parent_slot_ref.set_state(State::Running);
    let parent_basepage = parent_slot_ref.basepage_kva.load(Ordering::Acquire);
    set_current(parent_basepage);

    kprintln!("[d4d5] eret → EL0 in parent …");
    // SAFETY: pre-test, single-CPU; the parent's `parent_frame` is the
    // kernel-resume capture, consumed by the parent's final `Pterm`.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            parent_loaded.entry_va,
            parent_loaded.l0_pa,
            USER_STACK_TOP_EXCLUSIVE,
            &PROCESS_TABLE[parent_slot_idx].parent_frame,
        );
    }
    kprintln!("[d4d5] returned from EL0 (parent Pterm'd)");

    let post = pool_stats(PoolKind::Stram);
    let pool_baseline_ok = post.free_bytes == pre.free_bytes
        && post.used_bytes == pre.used_bytes
        && post.allocated_blocks == pre.allocated_blocks;
    let run_cleared = tos_api::bdos::process::current().is_null();

    kprintln!(
        "[d4d5] post:  ST-RAM free={} used={} alloc_blocks={}",
        post.free_bytes, post.used_bytes, post.allocated_blocks,
    );
    kprintln!(
        "[d4d5]   pool baseline restored:                    {}",
        if pool_baseline_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d4d5]   RUN cleared (current() == null):           {}",
        if run_cleared { "OK" } else { "FAIL" },
    );

    if pool_baseline_ok && run_cleared {
        kprintln!("[d4d5] === D4/D5 PEXEC PASS ===");
    } else {
        kprintln!("[d4d5] === D4/D5 PEXEC FAIL ===");
    }
}
