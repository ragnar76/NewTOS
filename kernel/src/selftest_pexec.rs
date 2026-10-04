//! Phase 06 D3 — `Pexec(PE_LOADGO)` end-to-end self-test.
//!
//! First test that drives the **full process-launch pipeline**:
//! a parent EL0 process calls `Pexec(PE_LOADGO, "C", …)`, the kernel
//! loads child `C`, transfers control to it, the child runs
//! `Cconws("Hello\n")` + `Pterm(42)`, the kernel restores the
//! parent's frame with the child's exit code in `x0`, and the
//! parent terminates with the same code.
//!
//! Exit criterion **EC1**: a trivial NewTOS-native ELF executes via
//! `Pexec`, prints via `Cconws`, returns through `Pterm(0)`. (D3
//! uses `rc=42` instead of `0` so the parent's exit code is
//! observably distinct from "did nothing".)
//!
//! ## Pipeline
//!
//! 1. Register both ELFs in [`tos_api::bdos::process::registry`].
//! 2. Allocate a slot for the parent, [`load_to_slot`], populate
//!    `RUN`, drop to EL0 via `enter_el0_with_resume_capture`. The
//!    parent's resume context (this driver's continuation) lands in
//!    `slot.parent_frame` so the *parent's* eventual `Pterm`
//!    returns control here.
//! 3. Parent runs:
//!    ```text
//!    Pexec(0, "C", 0, 0)   ; → x0 = child rc when child Pterms
//!    Pterm(rc-from-x0)     ; parent terminates with same code
//!    ```
//! 4. Pexec handler loads child `C` into a fresh slot, snapshots
//!    parent's frame into child slot's `parent_frame`, switches
//!    TTBR0, eret to child.
//! 5. Child runs `Cconws("Hello from D3 child\n")` + `Pterm(42)`.
//! 6. Pterm restores parent's frame with `gpr[0] = 42`, switches
//!    TTBR0 back to parent's L0, eret to parent EL0.
//! 7. Parent's `Pexec` SVC returns with `x0 = 42`. Parent's
//!    `Pterm(42)` then runs — kernel-launched, so kernel-resume
//!    path lands back in this driver.
//! 8. Driver verifies pool baseline restored + ST-RAM is back to
//!    its pre-test snapshot.
//!
//! ## Slot allocation
//!
//! D3-Pexec is the first test that uses [`allocate_slot`] for both
//! parent and child. Earlier tests (EC4 / D3-loader / D6) hardcoded
//! their slot index. After D6's `Pterm` released slot 2, the next
//! `allocate_slot` returns `0` (lowest `Free`). EC4 / D3-loader
//! still hold slots 0 / 1 in `Free` state (they never claimed via
//! the CAS path), so the parent will land in slot 0 and the child
//! in slot 1. Both slots get fully reset by the loader's
//! `init_slot_tables` and the page-zeroing pass, so the leftover
//! state from the earlier tests is irrelevant.

use core::sync::atomic::Ordering;

use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
use tos_api::bdos::process::loader::load_to_slot;
use tos_api::bdos::process::{
    allocate_slot, registry, set_current, slot, State, PROCESS_TABLE,
};

use crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
use crate::kprintln;

// ─── Parent ELF ─────────────────────────────────────────────────────
//
// File layout (514 bytes total):
//
//   0x000..0x040 : ELF64 Ehdr           (64 bytes)
//   0x040..0x078 : 1× ELF64 Phdr        (PT_LOAD, 56 bytes)
//   0x078..0x100 : padding              (136 bytes)
//   0x100..0x11C : code                 (28 bytes, 7 instructions)
//   0x11C..0x200 : padding              (228 bytes)
//   0x200..0x202 : program name "C\0"   (2 bytes)
//
// Code:
//   0x100  D2 80 00 00   movz x0, #0          ; mode = PE_LOADGO
//   0x104  10 00 07 E1   adr  x1, .+0xFC      ; path = &"C\0" (PC-relative)
//   0x108  D2 80 00 02   movz x2, #0          ; tail
//   0x10C  D2 80 00 03   movz x3, #0          ; env
//   0x110  D2 80 09 68   movz x8, #0x4B       ; Pexec
//   0x114  D4 00 00 21   svc  #1              ; → x0 = child rc
//   0x118  D2 80 09 88   movz x8, #0x4C       ; Pterm
//   0x11C  D4 00 00 21   svc  #1              ; (note: filesz stops here, so this
//                                             ;  byte is past the 28-byte code if we
//                                             ;  count from 0x100)
//
// Wait — we need 8 instructions = 32 bytes. Adjust filesz to 0x202 - 0x100 = 0x102 = 258.

/// Hand-crafted parent ELF. Calls Pexec on "C" then Pterm with x0 = child rc.
#[rustfmt::skip]
pub static PARENT_ELF: [u8; 514] = [
    // ─── Ehdr (64 bytes) ─────────────────────────────────────────
    0x7F, b'E', b'L', b'F',
    2, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
    0x03, 0x00,             // e_type ET_DYN (PIE; ET_EXEC is refused since ARCH03 0d)
    0xB7, 0x00,             // e_machine EM_AARCH64
    0x01, 0x00, 0x00, 0x00, // e_version
    // e_entry = 0x1000 (VA of the code; file offset 0x100)
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
    // p_vaddr  = 0x1000 (>= RESERVED_LOW_END; was 0x100 and failed with
    //                    SegmentVaOutOfRange since B1 moved the image base)
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_paddr  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_filesz = 0x102 = 258 (covers code 0x100..0x120 + name 0x200..0x202)
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

    // ─── Code at 0x100 (32 bytes, 8 instructions) ────────────────
    0x00, 0x00, 0x80, 0xD2, // movz x0, #0           (mode = PE_LOADGO)
    0xE1, 0x07, 0x00, 0x10, // adr  x1, .+0xFC       (path = "C\0", PC-relative)
    0x02, 0x00, 0x80, 0xD2, // movz x2, #0           (tail)
    0x03, 0x00, 0x80, 0xD2, // movz x3, #0           (env)
    0x68, 0x09, 0x80, 0xD2, // movz x8, #0x4B        (Pexec)
    0x21, 0x00, 0x00, 0xD4, // svc  #1               → x0 = child rc
    0x88, 0x09, 0x80, 0xD2, // movz x8, #0x4C        (Pterm)
    0x21, 0x00, 0x00, 0xD4, // svc  #1               (parent: rc = x0 from Pexec)

    // ─── Padding 0x120..0x200 (224 bytes = 7 × 32) ───────────────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,

    // ─── Program name "C\0" at 0x200 (2 bytes) ───────────────────
    b'C', 0,
];

// ─── Child ELF ──────────────────────────────────────────────────────
//
// File layout (518 bytes total):
//
//   0x000..0x040 : Ehdr             (64)
//   0x040..0x078 : Phdr             (56)
//   0x078..0x100 : padding          (136)
//   0x100..0x11C : code             (28 bytes, 7 instructions)
//   0x11C..0x200 : padding          (228)
//   0x200..0x206 : "Hello\n\0"      (7 bytes)

/// Hand-crafted child ELF. Calls Cconws to print a banner then
/// Pterm with rc = 42.
#[rustfmt::skip]
pub static CHILD_ELF: [u8; 519] = [
    // ─── Ehdr (64 bytes) ─────────────────────────────────────────
    0x7F, b'E', b'L', b'F',
    2, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
    0x03, 0x00, 0xB7, 0x00, 0x01, 0x00, 0x00, 0x00, // e_type ET_DYN (PIE), EM_AARCH64, EV_CURRENT
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_entry = 0x1000
    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_phoff
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_shoff
    0x00, 0x00, 0x00, 0x00,                          // e_flags
    0x40, 0x00, 0x38, 0x00, 0x01, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,

    // ─── Phdr (56 bytes) ─────────────────────────────────────────
    0x01, 0x00, 0x00, 0x00, // p_type PT_LOAD
    0x05, 0x00, 0x00, 0x00, // p_flags PF_R | PF_X
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_offset = 0x100
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_vaddr  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_paddr
    // p_filesz = 0x107 = 263 (covers code + 7-byte string at 0x200..0x207)
    0x07, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x07, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_memsz
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_align

    // ─── Padding 0x78..0x100 (136 bytes) ─────────────────────────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,

    // ─── Code at 0x100 (28 bytes, 7 instructions) ────────────────
    //   0x100  10 00 08 00  adr  x0, .+0x100     ; arg = &"Hello\n\0" (PC-relative)
    //   0x104  D2 80 01 28  movz x8, #0x09       ; Cconws
    //   0x108  D4 00 00 21  svc  #1
    //   0x10C  D2 80 05 40  movz x0, #0x2A       ; rc = 42
    //   0x110  D2 80 09 88  movz x8, #0x4C       ; Pterm
    //   0x114  D4 00 00 21  svc  #1              ; never returns
    //   0x118  14 00 00 00  b    .               ; safety net
    0x00, 0x08, 0x00, 0x10, // adr  x0, .+0x100
    0x28, 0x01, 0x80, 0xD2, // movz x8, #0x09 (Cconws)
    0x21, 0x00, 0x00, 0xD4, // svc  #1
    0x40, 0x05, 0x80, 0xD2, // movz x0, #0x2A (rc = 42)
    0x88, 0x09, 0x80, 0xD2, // movz x8, #0x4C (Pterm)
    0x21, 0x00, 0x00, 0xD4, // svc  #1
    0x00, 0x00, 0x00, 0x14, // b    .

    // ─── Padding 0x11C..0x200 (228 bytes = 7 × 32 + 4) ───────────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,

    // ─── String at 0x200 ─────────────────────────────────────────
    b'H', b'e', b'l', b'l', b'o', b'\n', 0,
];

/// Run the Pexec D3 round-trip test.
pub fn run_pexec_test() {
    kprintln!("");
    kprintln!("[d3] === Phase 06 D3 Pexec test ===");

    // 1. Register the child program.
    if !registry::register(b"C", &CHILD_ELF) {
        kprintln!("[d3] === FAIL: registry full ===");
        return;
    }

    // 2. Snapshot ST-RAM baseline.
    let pre = pool_stats(PoolKind::Stram);
    kprintln!(
        "[d3] pre :  ST-RAM free={} used={} alloc_blocks={}",
        pre.free_bytes, pre.used_bytes, pre.allocated_blocks,
    );

    // 3. Allocate parent slot, load parent ELF.
    let parent_slot_idx = match allocate_slot() {
        Some(i) => i,
        None => {
            kprintln!("[d3] === FAIL: allocate_slot returned None ===");
            return;
        }
    };
    // SAFETY: pre-test, single-CPU; slot just claimed.
    let parent_loaded = match unsafe { load_to_slot(&PARENT_ELF, parent_slot_idx) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[d3] === FAIL: parent loader: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[d3] parent loaded slot {}: entry=0x{:x} l0_pa=0x{:x}",
        parent_loaded.slot, parent_loaded.entry_va, parent_loaded.l0_pa,
    );

    // 4. Mark parent Running, publish RUN.
    let parent_slot_ref = slot(parent_slot_idx).expect("slot in range");
    parent_slot_ref.set_state(State::Running);
    let parent_basepage = parent_slot_ref.basepage_kva.load(Ordering::Acquire);
    set_current(parent_basepage);

    // 5. Drop to EL0 in parent. Parent's `Pterm` (after the Pexec
    //    round-trip) brings us back here via the kernel-launch
    //    path (parent_l0_pa = 0 default → restore_boot_ttbr0).
    kprintln!("[d3] eret → EL0 in parent …");
    // SAFETY: pre-test, single-CPU; the slot's parent_frame is
    // populated by `enter_el0_with_resume_capture` for the kernel-
    // resume path, and Pterm later consumes it via the SPSR.M=EL1t
    // branch of the handler.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            parent_loaded.entry_va,
            parent_loaded.basepage_va,
            parent_loaded.l0_pa,
            USER_STACK_TOP_EXCLUSIVE,
            &PROCESS_TABLE[parent_slot_idx].parent_frame,
        );
    }
    kprintln!("[d3] returned from EL0 (parent Pterm'd)");

    // 6. Validate.
    let post = pool_stats(PoolKind::Stram);
    let pool_baseline_ok = post.free_bytes == pre.free_bytes
        && post.used_bytes == pre.used_bytes
        && post.allocated_blocks == pre.allocated_blocks;
    let run_cleared =
        tos_api::bdos::process::current().is_null();

    kprintln!(
        "[d3] post:  ST-RAM free={} used={} alloc_blocks={}",
        post.free_bytes, post.used_bytes, post.allocated_blocks,
    );
    kprintln!(
        "[d3]   pool baseline restored:                      {}",
        if pool_baseline_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   RUN cleared (current() == null):             {}",
        if run_cleared { "OK" } else { "FAIL" },
    );

    if pool_baseline_ok && run_cleared {
        kprintln!("[d3] === D3 PEXEC PASS ===");
    } else {
        kprintln!("[d3] === D3 PEXEC FAIL ===");
    }
}

/// F1 — verify `Pexec` populates the child's command tail (`p_cmdlin`,
/// EmuTOS `init_pd_fields`) and environment (`p_env`, EmuTOS
/// `alloc_env`). Drives the loader directly (like the other Pexec
/// selftests) so the basepage fields are QEMU-checkable without an EL0
/// parent; the exit-code / launch delivery stays EL0-gated.
pub fn run_pexec_env_test() {
    use tos_api::bdos::mem::traps::mfree;
    use tos_api::bdos::process::free_slot;
    use tos_api::bdos::process::loader::{load_to_slot_with, EnvSpec};
    use tos_types::BasePage;

    // High-half linear-map offset (kernel VA = phys + VA_OFFSET); the
    // env block lives in pool phys, read back here from the kernel.
    const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

    kprintln!("");
    kprintln!("[f1] === Pexec command-tail + environment test ===");

    // Atari command-tail image: a length byte followed by the text (no
    // trailing NUL — the loader appends it). "HELLO" → length 5.
    let tail: [u8; 6] = [5, b'H', b'E', b'L', b'L', b'O'];
    // Environment image: two records + terminating double NUL (20 bytes).
    let env: &[u8] = b"PATH=C:\\\0TERM=vt52\0\0";

    let slot_idx = match allocate_slot() {
        Some(i) => i,
        None => {
            kprintln!("[f1] === FAIL: no slot ===");
            return;
        }
    };

    // SAFETY: pre-test, single-CPU; slot just claimed. `true` = PE_LOADGO
    // → the env block is owned by the child.
    let loaded = match unsafe {
        load_to_slot_with(&CHILD_ELF, slot_idx, &tail, EnvSpec::Explicit(env), true)
    } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[f1] === FAIL: loader {:?} ===", e);
            free_slot(slot_idx);
            return;
        }
    };

    let bp_kva = PROCESS_TABLE[loaded.slot]
        .basepage_kva
        .load(Ordering::Acquire);
    // SAFETY: the loader published a valid basepage kernel VA for the slot.
    let bp: &BasePage = unsafe { &*bp_kva };

    // Command tail: length byte, then the text, then a NUL.
    let len = bp.p_cmdlin[0] as usize;
    let tail_ok = len == 5 && &bp.p_cmdlin[1..6] == b"HELLO" && bp.p_cmdlin[6] == 0;

    // Environment: p_env is non-zero and the pool copy matches verbatim.
    let penv = bp.p_env;
    let env_ok = if penv != 0 {
        let src = (penv as u64 + VA_OFFSET) as *const u8;
        // SAFETY: p_env points at the copied env block in mapped pool mem.
        let copy = unsafe { core::slice::from_raw_parts(src, env.len()) };
        copy == env
    } else {
        false
    };

    kprintln!(
        "[f1]   p_cmdlin[0]={} text=\"{}\" NUL@{}          {}",
        len,
        core::str::from_utf8(&bp.p_cmdlin[1..6]).unwrap_or("?"),
        6,
        if tail_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[f1]   p_env=0x{:x} (!=0 & content match)         {}",
        penv,
        if env_ok { "OK" } else { "FAIL" },
    );

    // Clean up: free the env block, release the slot.
    if penv != 0 {
        // SAFETY: single-CPU pre-test; p_env is a live pool allocation.
        let _ = unsafe { mfree(penv) };
    }
    free_slot(loaded.slot);

    if tail_ok && env_ok {
        kprintln!("[f1] === F1 PEXEC TAIL+ENV PASS ===");
    } else {
        kprintln!("[f1] === F1 PEXEC TAIL+ENV FAIL ===");
    }
}
