// AArch64 exception vector table.
//
// References:
//   - ARM ARM D1.10.2 "Exception vectors"
//   - ARM ARM D17.2.37 "ESR_EL1, Exception Syndrome Register (EL1)"
//   - AAPCS64 §5.2.2 "Stack alignment"
//
// Sixteen vectors of 128 B each, table aligned to 2048 B (2^11).
// Each vector is a single `b` to a long-form stub which saves the full
// integer register state into a frame on the stack, calls the Rust
// handler with x0 = &mut ExceptionFrame, then restores state and `eret`s.
//
// Frame layout (Phase 05 D6 — added SP_EL0 + pad slot):
//   30*8 (x0..x29) + 8 (lr) + 8 (elr) + 8 (spsr) + 8 (esr)
//   + 8 (sp_el0) + 8 (pad)  = 288 bytes
//
// SP_EL0 is meaningful for traps from EL0 (Phase 06 onward): the user
// program's stack pointer at trap entry. The pad slot keeps the stp/ldp
// pairs aligned and is reserved for a future TPIDR_EL0 / process tag.

.equ FRAME_SIZE, 288

.macro EXC_BODY handler
    sub     sp, sp, #FRAME_SIZE
    stp     x0,  x1,  [sp, #16 * 0]
    stp     x2,  x3,  [sp, #16 * 1]
    stp     x4,  x5,  [sp, #16 * 2]
    stp     x6,  x7,  [sp, #16 * 3]
    stp     x8,  x9,  [sp, #16 * 4]
    stp     x10, x11, [sp, #16 * 5]
    stp     x12, x13, [sp, #16 * 6]
    stp     x14, x15, [sp, #16 * 7]
    stp     x16, x17, [sp, #16 * 8]
    stp     x18, x19, [sp, #16 * 9]
    stp     x20, x21, [sp, #16 * 10]
    stp     x22, x23, [sp, #16 * 11]
    stp     x24, x25, [sp, #16 * 12]
    stp     x26, x27, [sp, #16 * 13]
    stp     x28, x29, [sp, #16 * 14]
    mrs     x9,  elr_el1
    stp     x30, x9,  [sp, #16 * 15]
    mrs     x10, spsr_el1
    mrs     x11, esr_el1
    stp     x10, x11, [sp, #16 * 16]
    mrs     x12, sp_el0
    stp     x12, xzr, [sp, #16 * 17]    // sp_el0 + 8 B reserved/pad

    mov     x0, sp
    bl      \handler

    ldp     x12, x13, [sp, #16 * 17]    // x13 receives the pad — discarded
    msr     sp_el0, x12
    ldp     x10, x11, [sp, #16 * 16]
    msr     spsr_el1, x10
    ldp     x30, x9,  [sp, #16 * 15]
    msr     elr_el1,  x9
    ldp     x28, x29, [sp, #16 * 14]
    ldp     x26, x27, [sp, #16 * 13]
    ldp     x24, x25, [sp, #16 * 12]
    ldp     x22, x23, [sp, #16 * 11]
    ldp     x20, x21, [sp, #16 * 10]
    ldp     x18, x19, [sp, #16 * 9]
    ldp     x16, x17, [sp, #16 * 8]
    ldp     x14, x15, [sp, #16 * 7]
    ldp     x12, x13, [sp, #16 * 6]
    ldp     x10, x11, [sp, #16 * 5]
    ldp     x8,  x9,  [sp, #16 * 4]
    ldp     x6,  x7,  [sp, #16 * 3]
    ldp     x4,  x5,  [sp, #16 * 2]
    ldp     x2,  x3,  [sp, #16 * 1]
    ldp     x0,  x1,  [sp, #16 * 0]
    add     sp, sp, #FRAME_SIZE
    eret
.endm

.section .text.exceptions, "ax"
.global __exception_vectors
.balign 2048
__exception_vectors:
    // Current EL with SP0 — used by Phase 03 D6 onward, where the
    // kernel runs at EL1 with SPSel=0 (SP_EL0 = main kernel stack).
    // Exceptions auto-promote SPSel to 1, so the handlers run on
    // SP_EL1 = emergency exception stack.
    b   __vec_sync                   // 0x000 sync (SVC, data/inst aborts)
    .balign 0x80
    b   __vec_irq                    // 0x080 IRQ
    .balign 0x80
    b   __vec_unhandled              // 0x100 FIQ
    .balign 0x80
    b   __vec_unhandled              // 0x180 SError
    .balign 0x80

    // Current EL with SPx — entered while running on SP_EL1 directly
    // (SPSel=1). Used by Phase 02 and as a safety net if early code
    // takes an exception before the SPSel=0 switch in `kernel_main`.
    b   __vec_sync                   // 0x200 sync (SVC etc.)
    .balign 0x80
    b   __vec_irq                    // 0x280 IRQ
    .balign 0x80
    b   __vec_unhandled              // 0x300 FIQ
    .balign 0x80
    b   __vec_unhandled              // 0x380 SError
    .balign 0x80

    // Lower EL using AArch64 — used once user processes (PHASE-06) exist.
    b   __vec_sync                   // 0x400 sync
    .balign 0x80
    // IRQ taken while an EL0 program runs. Programs are entered with
    // SPSR.DAIF = 0 (`user_entry.rs`), so every interrupt that arrives
    // during a program — the 200 Hz system tick, and the PL011 RX IRQ —
    // lands here; it must go to the same handler as an EL1 IRQ, not to the
    // halting catch-all. (The frame save/restore in EXC_BODY is EL-agnostic:
    // exceptions from EL0 run on SP_EL1 and the eret returns to EL0 with the
    // program's own SP_EL0 and TTBR0 untouched.)
    b   __vec_irq                    // 0x480 IRQ
    .balign 0x80
    b   __vec_unhandled              // 0x500 FIQ
    .balign 0x80
    b   __vec_unhandled              // 0x580 SError
    .balign 0x80

    // Lower EL using AArch32 — NewTOS does not implement AArch32.
    b   __vec_unhandled              // 0x600 sync
    .balign 0x80
    b   __vec_unhandled              // 0x680 IRQ
    .balign 0x80
    b   __vec_unhandled              // 0x700 FIQ
    .balign 0x80
    b   __vec_unhandled              // 0x780 SError
    .balign 0x80
    // end of 2 KiB table

// Long-form stubs — placed outside the table so they may exceed 128 B.
__vec_sync:
    EXC_BODY rust_sync_handler

__vec_irq:
    EXC_BODY rust_irq_handler

__vec_unhandled:
    EXC_BODY rust_unhandled_handler
