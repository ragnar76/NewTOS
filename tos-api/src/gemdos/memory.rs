//! GEMDOS memory group (`Maddalt`, `Mxalloc`, `Malloc`, `Mfree`,
//! `Mshrink`).
//!
//! The trap entries in this module decode the AAPCS64 argument
//! registers and forward into [`crate::bdos::mem::traps`]. Argument
//! ordering follows the Atari ABI exactly so source-level ports see
//! the same parameters in the same positions.
//!
//! ## One address space (ARCH03 stage 1)
//!
//! Every user address is its own physical address and both pools are
//! mapped for every process ([`crate::bdos::process::mmu`]), so the pool
//! allocator's answer **is** the answer: `Malloc`/`Mxalloc` return the
//! block's `m_start`, `Mfree`/`Mshrink` take it back, and `Malloc(-1)` is the
//! largest free block of the selected pool — EmuTOS `bdos/umem.c`, for an
//! EL0 program and for a kernel caller alike. (Until stage 1 an EL0 caller
//! got a per-process window address instead, ADR-021; that window and its
//! one-page alignment slack are gone.)
//!
//! What still depends on the caller: `Malloc` picks its pool from the
//! running process's `PF_TTRAMMEM` flag (EmuTOS `xmalloc`), and a freed
//! block is reported to the AES ([`crate::aes::resource::on_mfree`]), which
//! may hold it as a resource blob.

use crate::bdos::mem::traps;
use crate::bdos::process;
use crate::trap::TrapArgs;
use tos_types::error::E_OK;
use tos_types::{Long, MX_PREFTTRAM, MX_STRAM, PF_TTRAMMEM};

/// `Maddalt(start, size)` — GEMDOS 0x14.
///
/// Spec:
/// [`docs/api/gemdos/memory/14-maddalt.md`](../../../../docs/api/gemdos/memory/14-maddalt.md).
///
/// Argument register layout:
/// - `x0` = `start`  (Atari `LONG`, sign-extended to `i64` on entry)
/// - `x1` = `size`   (Atari `LONG`)
///
/// `Maddalt` registers kernel-trusted physical RAM and is never an EL0-app
/// operation, so it always forwards to the raw pool primitive.
pub fn maddalt(args: &TrapArgs) -> Long {
    let start = args.arg_i32(0);
    let size = args.arg_i32(1);
    // SAFETY: NewTOS is single-CPU pre-Phase 06 and SVC handlers run
    // with IRQs masked, so the BDOS layer is exclusively held for
    // the duration of the dispatch. Phase 06 will replace this with
    // an explicit BDOS giant lock.
    unsafe { crate::bdos::mem::traps::maddalt(start, size) }
}

/// `Mxalloc(amount, mode)` — GEMDOS 0x44.
///
/// Spec:
/// [`docs/api/gemdos/memory/44-mxalloc.md`](../../../../docs/api/gemdos/memory/44-mxalloc.md).
///
/// Argument register layout:
/// - `x0` = `amount` (Atari `LONG`)
/// - `x1` = `mode`   (Atari `WORD`)
///
/// The returned pointer is the block's address, valid in every process
/// (VA = PA); `amount == -1` returns the largest free block of the selected
/// pool(s) (EmuTOS `xmxalloc`, `bdos/umem.c`).
pub fn mxalloc(args: &TrapArgs) -> Long {
    let amount = args.arg_i32(0);
    let mode = args.arg_i16(1);
    // SAFETY: NewTOS is single-CPU and SVC handlers run with IRQs masked, so
    // the BDOS layer is exclusively held for the duration of the dispatch.
    unsafe { traps::mxalloc(amount, mode) }
}

/// `Malloc(amount)` — GEMDOS 0x48.
///
/// Spec:
/// [`docs/api/gemdos/memory/48-malloc.md`](../../../../docs/api/gemdos/memory/48-malloc.md).
///
/// Argument register layout:
/// - `x0` = `amount` (Atari `LONG`)
///
/// Policy wrapper around `Mxalloc`: EmuTOS `xmalloc` picks the mode from the
/// running process's `PF_TTRAMMEM` flag (`bdos/umem.c:116`). A kernel caller
/// (no running process) keeps `traps::malloc` (kernel sentinel =
/// `PF_TTRAMMEM`).
pub fn malloc(args: &TrapArgs) -> Long {
    let amount = args.arg_i32(0);
    let run = process::current_kva();
    if run.is_null() {
        // SAFETY: see `mxalloc`.
        return unsafe { traps::malloc(amount) };
    }
    // EmuTOS `xmalloc`: `PF_TTRAMMEM ? MX_PREFTTRAM : MX_STRAM`, read from the
    // running process's own basepage.
    // SAFETY: `run` is the kernel alias of the running process's basepage.
    let flags = unsafe { (*run).p_flags };
    let mode = if flags & PF_TTRAMMEM != 0 {
        MX_PREFTTRAM
    } else {
        MX_STRAM
    };
    // SAFETY: see `mxalloc`.
    unsafe { traps::mxalloc(amount, mode) }
}

/// Tell the AES that a program freed the block at `m` — it may be a
/// resource blob or icon buffer `rsrc_load` registered, which is an
/// ordinary process-owned block in EmuTOS too (`rs_readit` →
/// `dos_alloc_anyram`, `aes/gemrslib.c:775`) and freed by whoever passes
/// its address — TeraDesk's `xrsrc_free` = `Mfree(ap_rscmem)` (A76,
/// `docs/api/gemdos/memory/49-mfree.md` § "AES resource block"). The AES
/// entry must not free that key a second time on `rsrc_free`.
fn note_user_free(m: Long) {
    crate::aes::resource::on_mfree(m);
}

/// `Mfree(addr)` — GEMDOS 0x49.
///
/// Spec:
/// [`docs/api/gemdos/memory/49-mfree.md`](../../../../docs/api/gemdos/memory/49-mfree.md).
///
/// Argument register layout:
/// - `x0` = `addr` (Atari `LONG`, the block start address)
///
/// `EIMBA` when `addr` starts no allocated block (EmuTOS `xmfree`,
/// `bdos/umem.c:135-159`); no ownership check (EmuTOS).
pub fn mfree(args: &TrapArgs) -> Long {
    let addr = args.arg_i32(0);
    // SAFETY: see `mxalloc`.
    let rc = unsafe { traps::mfree(addr) };
    if rc == E_OK {
        note_user_free(addr);
    }
    rc
}

/// `Mshrink(n, blk, len)` — GEMDOS 0x4A.
///
/// Spec:
/// [`docs/api/gemdos/memory/4a-mshrink.md`](../../../../docs/api/gemdos/memory/4a-mshrink.md).
///
/// Argument register layout:
/// - `x0` = `n`   (Atari `WORD`, ignored — historic CP/M field)
/// - `x1` = `blk` (Atari `LONG`, block start address)
/// - `x2` = `len` (Atari `LONG`, new block length in bytes)
///
/// `blk` is a block address as `Malloc` returned it — including a basepage,
/// which is the start of its TPA block (EmuTOS `xsetblk`, `bdos/umem.c:170`).
/// `len == 0` frees the whole block (EmuTOS's `Mshrink(len=0) == Mfree`).
pub fn mshrink(args: &TrapArgs) -> Long {
    let n = args.arg_i16(0);
    let blk = args.arg_i32(1);
    let len = args.arg_i32(2);
    // SAFETY: see `mxalloc`.
    let rc = unsafe { traps::mshrink(n, blk, len) };
    if rc == E_OK && len == 0 {
        note_user_free(blk);
    }
    rc
}
